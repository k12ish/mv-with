use core::ops::Range;
use std::ffi::OsString;
use std::io::Read;

use camino::Utf8Path;
use codespan_reporting::diagnostic::{Diagnostic, Label};
use ignore::Walk;
use self_cell::self_cell;

/// This type alias is required by the `self_cell` macro.
type PathVec<'a> = Vec<&'a Utf8Path>;

self_cell!(
    /// Internal Type that allows multiple `Utf8Path`s to share the same buffer
    ///
    /// This type is created by the `self_cell` macro from the crate
    /// with the same name.
    struct SharedPaths {
        owner: String,

        #[covariant]
        dependent: PathVec,
    }
    impl {Debug, Eq, PartialEq}
);

impl SharedPaths {
    /// Find the the range of a substring within the shared buffer
    fn substring_range<T: AsRef<str>>(&self, substring: T) -> Range<usize> {
        let string_ptr: *const u8 = self.borrow_owner().as_ptr();
        let substring_ptr: *const u8 = substring.as_ref().as_ptr();
        let start;

        debug_assert!(
            {
                let string_end = string_ptr.wrapping_add(self.borrow_owner().len());
                let substring_end = substring_ptr.wrapping_add(substring.as_ref().len());

                string_ptr <= substring_ptr
                    && substring_ptr <= substring_end
                    && substring_end <= string_end
            },
            "the substring must be a true substring of the shared buffer"
        );

        // RATIONALE:
        //   Methods like string.find(substring) are ambigous since there may be
        //   multiple occurences of a substring.
        //
        //   Using the underlying pointer is a cheap and easy to reason about,
        //   we simply find the offset between the substring pointer and the
        //   pointer to the parent string
        //
        // SAFETY:
        //   Both pointers are in bounds and a multiple of 8 bits apart.
        //   This operation is equivalent to `ptr_into_vec.offset_from(vec.as_ptr())`,
        //   so it will never "wrap around" the address space.

        unsafe { start = substring_ptr.offset_from(string_ptr) as usize }

        Range {
            start,
            end: start + substring.as_ref().len(),
        }
    }
}

/// Wrapper type around `SharedPath` that manages one list of files
pub struct FileList(SharedPaths);

impl FileList {
    pub fn parse_walker(walker: Walk) -> Result<Self, (String, FLParseError)> {
        let filenames = walker
            .skip(1) // Skip current directory
            .map(|r| r.map(|entry| entry.into_path().into_os_string().into_string()))
            .collect::<Result<Result<Vec<String>, OsString>, _>>()
            .expect("Cannot Read Directory")
            .expect("Non-Utf8 path not supported");
        let buf = filenames.join("\n");
        if buf.trim().is_empty() {
            Err((buf, FLParseError::EmptyDirectory))
        } else {
            Ok(FileList::from_string(buf))
        }
    }

    pub fn parse_reader<T: Read>(mut reader: T) -> Result<Self, (String, FLParseError)> {
        let mut buf = String::new();
        reader
            .read_to_string(&mut buf)
            .expect("Non-Utf8 path not supported");
        buf.truncate(buf.trim_end().len());
        if buf.trim().is_empty() {
            Err((buf, FLParseError::EmptyStdIn))
        } else {
            Ok(FileList::from_string(buf))
        }
    }

    fn from_string(string: String) -> Self {
        FileList(SharedPaths::new(string, |s| {
            s.lines().map(|s| Utf8Path::new(s)).collect()
        }))
    }

    pub fn confirm_files_exist(self) -> Result<Self, (String, FLParseError)> {
        let list = self.0.borrow_dependent();
        let mut labels = Vec::new();
        for file in list {
            if !file.exists() {
                labels.push(
                    Label::primary((), self.0.substring_range(file))
                        .with_message("File does not exist"),
                );
            }
        }
        if labels.is_empty() {
            Ok(self)
        } else {
            Err((self.0.into_owner(), FLParseError::FileDoesNotExist(labels)))
        }
    }

    pub fn sort_by_file_depth(&mut self) {
        self.0.with_dependent_mut(|_, dependent| {
            dependent.sort_by_key(|path| {
                usize::MAX
                    - std::fs::canonicalize(path)
                        .expect("TOCTTOU error: files are expected to exist")
                        .components()
                        .count()
            });
        });
    }

    pub fn as_string(&self) -> String {
        let mut buf = String::new();
        for path in self.0.borrow_dependent() {
            buf.push_str(path.as_ref());
            buf.push('\n');
        }
        buf.pop();
        buf
    }
}

impl AsRef<str> for FileList {
    fn as_ref(&self) -> &str {
        self.0.borrow_owner()
    }
}

/// Wrapper around two `SharedPaths` that manages corresponding lists of files
pub struct RenameRequest {
    origin: SharedPaths,
    target: SharedPaths,
}

impl RenameRequest {
    pub fn new(origin: FileList, target: FileList) -> Result<Self, (String, RRParseError)> {
        use std::cmp::Ordering;

        let FileList(origin) = origin;
        let FileList(target) = target;
        let origin_vec = origin.borrow_dependent();
        let target_vec = target.borrow_dependent();
        let origin_len = origin_vec.len();
        let target_len = target_vec.len();

        match target_len.cmp(&origin_len) {
            Ordering::Equal => {
                if origin_vec
                    .iter()
                    .zip(target_vec.iter())
                    .all(|(a, b)| a == b)
                {
                    Err((target.into_owner(), RRParseError::FileUnchanged))
                } else {
                    Ok(RenameRequest { origin, target })
                }
            }
            Ordering::Less => {
                let end = target.borrow_owner().len();
                Err((target.into_owner(), RRParseError::TooFewLines(end)))
            }
            Ordering::Greater => {
                let start = target.substring_range(target_vec[origin_len - 1]).end + 1;
                let end = target.borrow_owner().len();
                Err((target.into_owner(), RRParseError::TooManyLines(start..end)))
            }
        }
    }

    pub fn print_diffs(&self) {
        use codespan_reporting::term::termcolor::ColorSpec as Spec;
        use codespan_reporting::term::termcolor::{BufferWriter, WriteColor};
        use codespan_reporting::term::termcolor::{Color, ColorChoice};
        use dissimilar::Chunk::*;
        use std::io::Write;
        use unicode_segmentation::UnicodeSegmentation;

        let origin = self.origin.borrow_dependent();
        let target = self.target.borrow_dependent();

        let wtr = BufferWriter::stdout(ColorChoice::Always);
        let mut buf = wtr.buffer();

        macro_rules! write_buf {
            ($color:expr, $($arg:tt)*) => {
                buf.set_color(&$color).unwrap();
                write!(&mut buf, $($arg)*).unwrap();
            };
        }

        for (before, after) in origin.iter().zip(target) {
            let chunk_vec = dissimilar::diff(before.as_ref(), after.as_ref());

            // The padding is calculated manually because ANSI escape codes interfere with
            // formatting strings, we cannot use format!({:>65}) to align the message
            let padding = {
                let diff_len: isize = chunk_vec
                    .iter()
                    .map(|chunk| {
                        let (Equal(s) | Delete(s) | Insert(s)) = chunk;
                        s.graphemes(true).count() as isize
                    })
                    .sum();
                " ".repeat(std::cmp::max(65 - diff_len, 0) as usize)
            };

            write_buf!(&Spec::new(), "  ");

            if let [Equal(diff) | Delete(diff)] = chunk_vec[..] {
                write_buf!(Spec::new().set_dimmed(true), "{}", diff);
                write_buf!(Spec::new(), "{}", padding);
                write_buf!(Spec::new().set_dimmed(true).set_italic(true), "(ignore)");
                writeln!(&mut buf).unwrap();
                continue;
            }

            for chunk in chunk_vec {
                match chunk {
                    Equal(diff) => {
                        write_buf!(Spec::new(), "{}", diff);
                    }
                    Insert(diff) => {
                        write_buf!(Spec::new().set_fg(Some(Color::Green)), "{}", diff);
                    }
                    Delete(diff) => {
                        // HACK: termcolor does not yet have strikethrough capability
                        write_buf!(Spec::new().set_fg(Some(Color::Red)), "\x1B[9m{}", diff);
                    }
                }
            }
            write_buf!(Spec::new(), "{}", padding);
            write_buf!(Spec::new().set_italic(true), "(rename)");
            writeln!(&mut buf).unwrap();
        }

        buf.set_color(&Spec::new()).unwrap();
        writeln!(&mut buf).unwrap();
        wtr.print(&buf).unwrap();
    }

    pub fn rename(self) -> Result<(), CannotRenameFile> {
        let origin = self.origin.borrow_dependent();
        let target = self.target.borrow_dependent();
        for (before, after) in origin.iter().zip(target) {
            if let Err(e) = std::fs::rename(before, after) {
                return Err(CannotRenameFile(
                    (before.to_string(), after.to_string()),
                    format!("{}", e),
                ));
            }
        }
        Ok(())
    }
}

/// Error that is triggered when you misspell an argument
/// ```bash
/// $ mv-with ivm
/// ```
pub struct MisspelledBashCommand<'a>(pub &'a str);
impl<'a> MisspelledBashCommand<'a> {
    pub fn report(self) -> Diagnostic<()> {
        Diagnostic::error()
            .with_message(format!("cannot execute `{}`", self.0))
            .with_notes(vec![String::from("Did you misspell this command?")])
    }
}

/// Error that is triggered when mv-with cannot rename a file
pub struct CannotRenameFile(pub (String, String), pub String);
impl CannotRenameFile {
    pub fn report(self) -> Diagnostic<()> {
        let (before, after) = self.0;
        Diagnostic::error()
            .with_message(format!("cannot rename `{}` to `{}`", before, after))
            .with_notes(vec![format!("Underlying OS error: {}", self.1)])
    }
}

#[derive(Debug)]
/// Errors that can be triggered when creating a `FileList` struct
pub enum FLParseError {
    /// Triggered if the Directory is empty
    /// ```bash
    /// $ mkdir foo && cd foo
    /// $ mv-with vim
    /// ```
    EmptyDirectory,
    /// Triggered if the StdIn is empty
    /// ```bash
    /// $ echo | mv-with vim
    /// ```
    EmptyStdIn,
    /// Triggered an input file does not exist.
    /// ```bash
    /// $ echo invalid_filename | mv-with vim
    /// ```
    FileDoesNotExist(Vec<Label<()>>),
}

use FLParseError::*;
impl FLParseError {
    pub fn report(self) -> Diagnostic<()> {
        match self {
            EmptyDirectory => {
                Diagnostic::warning()
                    .with_message("Directory is empty")
                    .with_notes(vec![
                        "By default, mv-with respects filters such as globs, file types and .gitignore files".into(),
                        "Use StdIn for finegrained control, eg. `ls -A | mv-with vim`".into()
                    ])}
            EmptyStdIn => Diagnostic::warning().with_message("StdIn is empty"),
            FileDoesNotExist(labels) => Diagnostic::error().with_message("File does not exist").with_labels(labels)
        }
    }

    pub fn status(&self) -> Option<i32> {
        match self {
            FileDoesNotExist(_) => Some(1),
            EmptyDirectory | EmptyStdIn => Some(0),
        }
    }
}

/// Errors that can be triggered when creating a `RenameRequest` struct
pub enum RRParseError {
    /// Triggered if the user removes lines from the tempfile
    TooFewLines(usize),
    /// Triggered if the user adds extra lines to the tempfile
    TooManyLines(Range<usize>),
    /// Triggered if the file is unchanged
    FileUnchanged,
}

use RRParseError::*;
impl RRParseError {
    pub fn report(self) -> Diagnostic<()> {
        match self {
            FileUnchanged => {
                return Diagnostic::note().with_message("Temporary file was unchanged")
            }
            TooFewLines(end) => Diagnostic::error()
                .with_message("Unexpected EOF")
                .with_labels(vec![
                    Label::primary((), end..end).with_message("Unexpected EOF")
                ]),
            TooManyLines(range) => Diagnostic::error()
                .with_message("Too many lines in temporary file")
                .with_labels(vec![Label::primary((), range).with_message("Expected EOF")]),
        }
        .with_notes(vec![
            "temporary file should have the same number of lines before and after editing".into(),
        ])
    }

    pub fn status(&self) -> Option<i32> {
        match self {
            FileUnchanged => Some(0),
            TooFewLines(_) | TooManyLines(_) => Some(1),
        }
    }
}

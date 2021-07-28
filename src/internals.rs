use core::ops::Range;
use std::ffi::OsString;
use std::io::Read;

use camino::Utf8Path;
use codespan_reporting::diagnostic::Label;
use ignore::Walk;
use self_cell::self_cell;

#[derive(Debug, Eq, PartialEq)]
struct PathVec<'a>(pub Vec<&'a Utf8Path>);

self_cell!(
    struct SharedPaths {
        owner: String,

        #[covariant]
        dependent: PathVec,
    }
    impl {Debug, Eq, PartialEq}
);

impl SharedPaths {
    fn substring_range<T: AsRef<str>>(&self, substring: T) -> Range<usize> {
        let string_ptr: *const u8 = self.borrow_owner().as_ptr();
        let substring_ptr: *const u8 = substring.as_ref().as_ptr();
        let start;

        debug_assert!({
            let string_end = string_ptr.wrapping_add(self.borrow_owner().len());
            let substring_end = substring_ptr.wrapping_add(substring.as_ref().len());

            string_ptr <= substring_ptr
                && substring_ptr <= substring_end
                && substring_end <= string_end
        });

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

pub struct FileList(SharedPaths);

impl FileList {
    pub fn parse_walker(walker: Walk) -> Result<Self, errors::FLParseError> {
        let filenames = walker
            .skip(1) // Skip current directory
            .map(|r| r.map(|entry| entry.into_path().into_os_string().into_string()))
            .collect::<Result<Result<Vec<String>, OsString>, _>>()
            .expect("Cannot Read Directory")
            .expect("Non-Utf8 path not supported");
        let buf = filenames.join("\n");
        if buf.trim().is_empty() {
            Err(errors::FLParseError::EmptyDirectory)
        } else {
            Ok(FileList::from_string(buf))
        }
    }

    pub fn parse_reader<T: Read>(mut reader: T) -> Result<Self, errors::FLParseError> {
        let mut buf = String::new();
        reader
            .read_to_string(&mut buf)
            .expect("Non-Utf8 path not supported");
        buf.truncate(buf.trim_end().len());
        if buf.trim().is_empty() {
            Err(errors::FLParseError::EmptyStdIn)
        } else {
            Ok(FileList::from_string(buf))
        }
    }

    fn from_string(string: String) -> Self {
        assert!(!string.trim().is_empty());
        FileList(SharedPaths::new(string, |s| {
            PathVec(s.lines().map(|s| Utf8Path::new(s)).collect())
        }))
    }

    pub fn as_str(&self) -> &str {
        self.0.borrow_owner()
    }

    pub fn confirm_files_exist(&self) -> Result<(), errors::FileDoesNotExist> {
        let PathVec(list) = self.0.borrow_dependent();
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
            Ok(())
        } else {
            Err(errors::FileDoesNotExist(labels))
        }
    }
}

pub struct RenameRequest {
    origin: SharedPaths,
    target: SharedPaths,
}

impl RenameRequest {
    pub fn new(origin: FileList, target: FileList) -> Result<Self, (String, errors::RRParseError)> {
        let FileList(origin) = origin;
        let FileList(target) = target;

        use errors::RRParseError;
        use std::cmp::Ordering;

        let origin_len = origin.borrow_dependent().0.len();
        let target_len = target.borrow_dependent().0.len();
        match target_len.cmp(&origin_len) {
            Ordering::Equal => Ok(RenameRequest { origin, target }),
            Ordering::Less => {
                let end = target.borrow_owner().len();
                Err((target.into_owner(), RRParseError::TooFewLines(end)))
            }
            Ordering::Greater => {
                let start = {
                    let vec = &target.borrow_dependent().0;
                    target.substring_range(vec[origin_len - 1]).end + 1
                };
                let end = target.borrow_owner().len();
                Err((target.into_owner(), RRParseError::TooManyLines(start..end)))
            }
        }
    }

    pub fn sort(&mut self) {
        // Permutation that sorts the files in `origin` by order of decreasing
        // file depth
        let permutation = {
            let file_depths = self
                .origin
                .borrow_dependent()
                .0
                .iter()
                .map(|path| {
                    std::fs::canonicalize(path)
                        .expect("TOCTTOU error: file origins are expected to exist")
                        .components()
                        .count()
                })
                .collect::<Vec<_>>();
            permutation::sort_by(&file_depths[..], |a, b| b.cmp(a))
        };

        // DRY code here is not worth the boilerplate
        self.origin.with_dependent_mut(|_, dependent| {
            let mut sorted_paths = permutation.apply_slice(&dependent.0[..]);
            dependent.0.truncate(0);
            dependent.0.append(&mut sorted_paths)
        });
        self.target.with_dependent_mut(|_, dependent| {
            let mut sorted_paths = permutation.apply_slice(&dependent.0[..]);
            dependent.0.truncate(0);
            dependent.0.append(&mut sorted_paths)
        });
    }

    pub fn print_diffs(&self) {
        use codespan_reporting::term::termcolor::{BufferWriter, WriteColor};
        use codespan_reporting::term::termcolor::{Color, ColorChoice, ColorSpec};
        use dissimilar::Chunk;
        use std::io::Write;

        let PathVec(origin) = self.origin.borrow_dependent();
        let PathVec(target) = self.target.borrow_dependent();

        let wtr = BufferWriter::stdout(ColorChoice::Always);
        let mut buf = wtr.buffer();

        for (before, after) in origin.iter().zip(target) {
            let chunk_vec = dissimilar::diff(before.as_ref(), after.as_ref());

            // The padding is calculated manually because ANSI escape codes interfere with
            // formatting strings, we cannot use format!({:>65}) to align the message
            let padding = {
                let diff_len: isize = chunk_vec
                    .iter()
                    .map(|s| match s {
                        Chunk::Equal(s) => s.chars().count(),
                        Chunk::Delete(s) => s.chars().count(),
                        Chunk::Insert(s) => s.chars().count(),
                    } as isize)
                    .sum();

                let len = std::cmp::max(65 - diff_len, 0) as usize;
                std::iter::repeat(' ').take(len).collect::<String>()
            };

            buf.set_color(&ColorSpec::new()).unwrap();
            write!(&mut buf, "  ").unwrap();

            match chunk_vec[..] {
                [Chunk::Equal(diff)] => {
                    buf.set_color(&ColorSpec::new().set_dimmed(true)).unwrap();
                    write!(&mut buf, "{}", diff).unwrap();
                    buf.set_color(&ColorSpec::new()).unwrap();
                    write!(&mut buf, "{}", padding).unwrap();
                    buf.set_color(&ColorSpec::new().set_dimmed(true).set_italic(true))
                        .unwrap();
                    write!(&mut buf, "(ignore)").unwrap();
                }
                _ => {
                    for chunk in chunk_vec {
                        match chunk {
                            Chunk::Equal(diff) => {
                                buf.set_color(&ColorSpec::new()).unwrap();
                                write!(&mut buf, "{}", diff).unwrap();
                            }
                            Chunk::Insert(diff) => {
                                buf.set_color(&ColorSpec::new().set_fg(Some(Color::Green)))
                                    .unwrap();
                                write!(&mut buf, "{}", diff).unwrap();
                            }
                            Chunk::Delete(diff) => {
                                buf.set_color(&ColorSpec::new().set_fg(Some(Color::Red)))
                                    .unwrap();
                                // HACK: termcolor does not have strikethrough capability
                                write!(&mut buf, "\x1B[9m{}", diff).unwrap();
                            }
                        }
                    }
                    buf.set_color(&ColorSpec::new()).unwrap();
                    write!(&mut buf, "{}", padding).unwrap();
                    buf.set_color(&ColorSpec::new().set_italic(true)).unwrap();
                    write!(&mut buf, "(rename)").unwrap();
                }
            }
            writeln!(&mut buf, "").unwrap();
        }
        buf.set_color(&ColorSpec::new()).unwrap();
        writeln!(&mut buf, "").unwrap();
        wtr.print(&buf).unwrap();
    }
}

pub mod errors {
    use codespan_reporting::diagnostic::{Diagnostic, Label};

    /// Triggered when you misspell an argument, eg.:
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

    /// Triggered an input file does not exist.
    /// ```bash
    /// $ echo invalid_filename | mv-with vim
    /// ```
    pub struct FileDoesNotExist(pub Vec<Label<()>>);
    impl FileDoesNotExist {
        pub fn report(self) -> Diagnostic<()> {
            Diagnostic::error()
                .with_message("file does not exist")
                .with_labels(self.0)
        }
    }

    /// Triggered the StdIn/Directory is empty
    /// ```bash
    /// $ echo | mv-with vim
    /// ```
    #[derive(Debug)]
    pub enum FLParseError {
        EmptyDirectory,
        EmptyStdIn,
    }

    impl FLParseError {
        pub fn report(self) -> Diagnostic<()> {
            match self {
                FLParseError::EmptyDirectory => {
                    Diagnostic::warning().with_message("Directory is empty")
                }
                FLParseError::EmptyStdIn => Diagnostic::warning().with_message("StdIn is empty"),
            }
        }
    }

    use core::ops::Range;
    pub enum RRParseError {
        /// Triggered if the user removes lines from the tempfile
        TooFewLines(usize),
        /// Triggered if the user adds extra lines to the tempfile
        TooManyLines(Range<usize>),
    }

    impl RRParseError {
        pub fn report(self) -> Diagnostic<()> {
            match self {
                RRParseError::TooFewLines(end) => Diagnostic::error()
                    .with_message("Unexpected EOF")
                    .with_labels(vec![
                        Label::primary((), end..end).with_message("Unexpected EOF")
                    ]),
                RRParseError::TooManyLines(range) => Diagnostic::error()
                    .with_message("Too many lines in temporary file")
                    .with_labels(vec![Label::primary((), range).with_message("Expected EOF")]),
            }
            .with_notes(vec![
                "temporary file should have the same number of lines before and after editing"
                    .into(),
            ])
        }
    }
}

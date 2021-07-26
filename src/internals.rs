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

pub struct FileList(SharedPaths);

impl FileList {
    pub fn parse_walker(walker: Walk) -> Result<Self, errors::EmptyWarning> {
        let filenames = walker
            .skip(1) // Skip current directory
            .map(|r| r.map(|entry| entry.into_path().into_os_string().into_string()))
            .collect::<Result<Result<Vec<String>, OsString>, _>>()
            .expect("Cannot Read Directory")
            .expect("Non-Utf8 path not supported");
        let buf = filenames.join("\n");
        FileList::from_string(buf)
    }

    pub fn parse_reader<T: Read>(mut reader: T) -> Result<Self, errors::EmptyWarning> {
        let mut buf = String::new();
        reader
            .read_to_string(&mut buf)
            .expect("Non-Utf8 path not supported");
        buf.truncate(buf.trim_end().len());
        FileList::from_string(buf)
    }

    fn from_string(string: String) -> Result<Self, errors::EmptyWarning> {
        if string.trim().is_empty() {
            Err(errors::EmptyWarning)
        } else {
            Ok(FileList(SharedPaths::new(string, |s| {
                PathVec(s.lines().map(|s| Utf8Path::new(s)).collect())
            })))
        }
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
                    Label::primary((), self.substring_range(file))
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

    fn substring_range<T: AsRef<str>>(&self, substring: T) -> Range<usize> {
        let string_ptr: *const u8 = self.as_str().as_ptr();
        let substring_ptr: *const u8 = substring.as_ref().as_ptr();
        let start;

        debug_assert!({
            let string_end = string_ptr.wrapping_add(self.as_str().len());
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

pub struct RenameRequest {
    origin: SharedPaths,
    target: SharedPaths,
}

impl RenameRequest {
    pub fn new(origin: FileList, target: FileList) -> Result<Self, (String, errors::TooFewLines)> {
        let FileList(origin) = origin;
        let FileList(target) = target;

        use std::cmp::Ordering;
        let origin_len = origin.borrow_dependent().0.len();
        let target_len = target.borrow_dependent().0.len();

        match target_len.cmp(&origin_len) {
            Ordering::Equal => Ok(RenameRequest { origin, target }),
            _ => {
                let index = target.borrow_owner().len() - 1;
                Err((target.into_owner(), errors::TooFewLines(index)))
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
    /// Warning should include a message to indicate origin:
    /// ```rust
    /// EmptyWarning.report().with_message("StdIn is empty")
    /// ```
    pub struct EmptyWarning;
    impl EmptyWarning {
        pub fn report(&self) -> Diagnostic<()> {
            Diagnostic::warning()
        }
    }

    /// Triggered if the edited tempfile is empty
    pub struct EmptyTempFile;
    impl EmptyTempFile {
        pub fn report(&self) -> Diagnostic<()> {
            Diagnostic::error().with_message("tempfile should not be empty")
        }
    }

    /// Triggered if the user removes lines from the tempfile
    pub struct TooFewLines(pub usize);
    impl TooFewLines {
        pub fn report(self) -> Diagnostic<()> {
            Diagnostic::error()
                .with_message("Unexpected EOF")
                .with_labels(vec![
                    Label::primary((), (self.0)..(self.0)).with_message("Unexpected EOF")
                ])
                .with_notes(vec![
                    "temporary file should have the same number of lines before and after editing"
                        .into(),
                ])
        }
    }
}

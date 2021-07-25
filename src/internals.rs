use core::ops::Range;
use std::ffi::OsString;
use std::io::Read;

use camino::Utf8Path;
use codespan_reporting::diagnostic::Label;
use ignore::Walk;
use self_cell::self_cell;

#[derive(Debug, Eq, PartialEq)]
pub struct FileNames<'a>(pub Vec<&'a Utf8Path>);

self_cell!(
    pub struct FileList {
        owner: String,

        #[covariant]
        dependent: FileNames,
    }
    impl {Debug, Eq, PartialEq}
);

impl FileList {
    pub fn parse_walker(walker: Walk) -> Result<Self, errors::EmptyWarning> {
        let filenames = walker
            .skip(1)
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
            Ok(FileList::new(string, |s| {
                FileNames(s.lines().map(|s| Utf8Path::new(s)).collect())
            }))
        }
    }

    pub fn as_str(&self) -> &str {
        self.borrow_owner()
    }

    pub fn confirm_files_exist(&self) -> Result<(), errors::FileDoesNotExist> {
        let FileNames(list) = self.borrow_dependent();
        let mut labels = Vec::new();
        for file in list {
            if !file.exists() {
                labels.push(
                    Label::primary((), self.substring_index(file))
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

    fn substring_index<T: AsRef<str>>(&self, substring: T) -> Range<usize> {
        let string_ptr: *const u8 = self.as_str().as_ptr();
        let substring_ptr: *const u8 = substring.as_ref().as_ptr();
        let start;

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

pub mod errors {
    use codespan_reporting::diagnostic::{Diagnostic, Label};

    /// Triggered when you misspell an argument, eg.:
    /// ```bash
    /// $ mv-with nivm
    /// ```
    pub struct MisspelledBashCommand<'a>(pub &'a str);
    impl<'a> MisspelledBashCommand<'a> {
        pub fn report(self) -> Diagnostic<()> {
            Diagnostic::error().with_message(format!("cannot execute `{}`", self.0))
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
    pub struct EmptyWarning;
    impl EmptyWarning {
        pub fn report(&self) -> Diagnostic<()> {
            Diagnostic::warning()
        }
    }

    /// Triggered when the edited tempfile is empty
    pub struct EmptyTempFile;
    impl EmptyTempFile {
        pub fn report(&self) -> Diagnostic<()> {
            Diagnostic::error().with_message("tempfile should not be empty")
        }
    }
}

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

type ParseResult<T> = Result<T, Box<dyn errors::EmptyFileListError>>;

impl FileList {
    pub fn parse_walker(walker: Walk) -> ParseResult<Self> {
        let filenames = walker
            .skip(1)
            .map(|r| r.map(|entry| entry.into_path().into_os_string().into_string()))
            .collect::<Result<Result<Vec<String>, OsString>, _>>()
            .expect("Cannot Read Directory")
            .expect("Non-Utf8 path not supported");
        Ok(FileList::from_string(filenames.join("\n")))
    }

    pub fn parse_reader<T: Read>(mut reader: T) -> ParseResult<Self> {
        let mut buf = String::new();
        reader
            .read_to_string(&mut buf)
            .expect("Non-Utf8 path not supported");
        buf.truncate(buf.trim_end().len());
        Ok(FileList::from_string(buf))
    }

    fn from_string(string: String) -> Self {
        FileList::new(string, |s| {
            FileNames(s.lines().map(|s| Utf8Path::new(s)).collect())
        })
    }

    pub fn as_str(&self) -> &str {
        self.borrow_owner()
    }

    pub fn confirm_files_exist(&self) -> Result<(), errors::FileDoesNotExist> {
        let FileNames(list) = self.borrow_dependent();
        for file in list {
            if !file.exists() {
                return Err(errors::FileDoesNotExist(
                    Label::primary((), self.substring_index(file))
                        .with_message("File does not exist"),
                ));
            }
        }
        Ok(())
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
        //           `We all scream for ice cream`
        //            ^                     ^
        //     String pointer         substring pointer
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
    /// $ rename-with nivm
    /// ```
    pub struct MisspelledBashCommand<'a>(pub &'a str);
    impl<'a> MisspelledBashCommand<'a> {
        pub fn report(self) -> Diagnostic<()> {
            Diagnostic::error().with_message(format!("cannot execute `{}`", self.0))
        }
    }

    /// Triggered an input file does not exist.
    /// ```bash
    /// $ echo invalid_filename | rename-with vim
    /// ```
    pub struct FileDoesNotExist(pub Label<()>);
    impl FileDoesNotExist {
        pub fn report(self) -> Diagnostic<()> {
            Diagnostic::error()
                .with_message("file does not exist")
                .with_labels(vec![self.0])
        }
    }

    /// Triggered the StdIn/Directory is empty
    /// ```bash
    /// $ echo | rename-with vim
    /// ```
    pub trait EmptyFileListError {
        fn report(&self) -> Diagnostic<()>;
    }

    pub struct EmptyStdIn;
    impl EmptyFileListError for EmptyStdIn {
        fn report(&self) -> Diagnostic<()> {
            Diagnostic::warning().with_message("StdIn is empty")
        }
    }

    pub struct EmptyDirectory;
    impl EmptyFileListError for EmptyDirectory {
        fn report(&self) -> Diagnostic<()> {
            Diagnostic::warning().with_message("Directory is empty")
        }
    }
}

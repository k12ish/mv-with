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
    pub fn parse_walker(walker: Walk) -> Self {
        let filenames = walker
            .skip(1)
            .map(|r| r.map(|entry| entry.into_path().into_os_string().into_string()))
            .collect::<Result<Result<Vec<String>, OsString>, _>>()
            .expect("Cannot Read Directory")
            .expect("Non-Utf8 path not supported");
        FileList::from_string(filenames.join("\n"))
    }

    pub fn parse_reader<T: Read>(mut reader: T) -> Self {
        let mut buf = String::new();
        reader
            .read_to_string(&mut buf)
            .expect("Non-Utf8 path not supported");
        buf.truncate(buf.trim_end().len());
        FileList::from_string(buf)
    }

    fn from_string(string: String) -> Self {
        FileList::new(string, |s| {
            FileNames(s.lines().map(|s| Utf8Path::new(s)).collect())
        })
    }

    pub fn as_str(&self) -> &str {
        self.borrow_owner()
    }

    pub fn confirm_files_exist(&self) -> Result<(), String> {
        let FileNames(list) = self.borrow_dependent();
        for file in list {
            if !file.exists() {
                return Err(format!("File '{}' does not exist", file));
            }
        }
        Ok(())
    }
}

pub mod errors {
    use codespan_reporting::diagnostic::{Diagnostic, Label};

    pub trait CodespanError {
        fn report(self) -> Diagnostic<()>;
    }

    pub struct MisspelledBashCommand<'a>(pub &'a str);
    impl<'a> CodespanError for MisspelledBashCommand<'a> {
        fn report(self) -> Diagnostic<()> {
            Diagnostic::error().with_message(format!("cannot execute `{}`", self.0))
        }
    }

    pub struct FileDoesNotExist(pub Label<()>);
    impl CodespanError for FileDoesNotExist {
        fn report(self) -> Diagnostic<()> {
            Diagnostic::error().with_labels(self.0)
        }
    }
}

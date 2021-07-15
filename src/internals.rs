use std::ffi::OsString;
use std::io::Read;

use camino::Utf8Path;
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

pub struct Origin;
pub struct Destination;

pub trait ConfirmFilenames<T> {
    fn validate(&self) -> Result<(), ()>;
}

impl ConfirmFilenames<Destination> for FileList {
    fn validate(&self) -> Result<(), ()> {
        println!("Confirming Files do not exist");
        Ok(())
    }
}

impl ConfirmFilenames<Origin> for FileList {
    fn validate(&self) -> Result<(), ()> {
        println!("Confirming Files exist");
        Ok(())
    }
}

use codespan_reporting::diagnostic::{Diagnostic, Label};

pub enum UserError<'a> {
    MisspelledBashCommand(&'a str),
}

impl<'a> UserError<'a> {
    pub fn report(&self) -> Diagnostic<()> {
        match self {
            UserError::MisspelledBashCommand(slice) => {
                Diagnostic::error().with_message(format!("cannot execute `{}`", slice))
            }
        }
    }
}

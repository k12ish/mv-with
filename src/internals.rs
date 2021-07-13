use std::error::Error;
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

type BoxedError = Box<dyn Error>;

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

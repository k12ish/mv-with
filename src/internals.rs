use std::convert::TryFrom;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;

#[macro_use]
use self_cell::self_cell;
use camino::Utf8Path;
use ignore::Walk;

#[derive(Debug, Eq, PartialEq)]
struct FileNames<'a>(pub Vec<&'a Utf8Path>);

self_cell!(
    struct FileList {
        owner: String,

        #[covariant]
        dependent: FileNames,
    }

    impl {Debug, Eq, PartialEq}
);

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

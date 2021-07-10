use std::convert::TryFrom;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;

use ignore::Walk;

/// `FileOrigin` instances are guarenteed to correspond to a file
/// They are wrappers of the `PathBuf` type (akin to an `OsString`)
#[derive(Debug)]
pub struct FileOrigin(PathBuf);

impl TryFrom<PathBuf> for FileOrigin {
    type Error = io::Error;
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        fs::metadata(&path)?;
        Ok(FileOrigin(path))
    }
}

#[derive(Debug)]
pub struct OriginList {
    filenames: Vec<FileOrigin>,
}

impl OriginList {
    /// Lossily convert all paths into a String, separated by '\n'
    /// Used to display the files in an editor for the user
    pub fn as_string(&self) -> String {
        let mut buffer = String::new();
        for origin in self.filenames.iter() {
            buffer.push_str(&origin.0.to_string_lossy());
            buffer.push('\n');
        }
        buffer
    }

    /// Creates an `OriginList` from an `ignore::Walk`
    /// Used in the case where StdIn is empty and files in the current dir are read.
    ///
    /// Errors may arise if:
    ///   * File metadata cannot be accessed:
    ///     - User lacks sufficient permissions
    ///     - path does not exist
    ///   * Some sort of I/O error during iteration
    pub fn from_walker(walk: Walk) -> Result<OriginList, Box<dyn std::error::Error>> {
        let filenames = walk
            .skip(1)
            .map(|r| r.map(|p| FileOrigin::try_from(p.into_path())))
            .collect::<Result<Result<Vec<_>, _>, _>>()??;
        Ok(OriginList { filenames })
    }

    /// Creates an `OriginList` from a reader
    /// Used to parse StdIn.
    ///
    /// Errors may arise if:
    ///   * File metadata cannot be accessed:
    ///     - User lacks sufficient permissions
    ///     - path does not exist
    pub fn from_reader<R: Read>(reader: R) -> Result<OriginList, io::Error> {
        let filenames = BufReader::new(reader)
            .lines()
            .map(|r| r.map(|s| FileOrigin::try_from(PathBuf::from(s))))
            .collect::<Result<Result<Vec<_>, _>, _>>()??;
        Ok(OriginList { filenames })
    }
}

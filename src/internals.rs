use std::convert::TryFrom;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;

use ignore::Walk;

#[derive(Debug)]
pub struct FileOrigin(pub PathBuf);

impl TryFrom<PathBuf> for FileOrigin {
    type Error = io::Error;
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        fs::metadata(&path)?;
        Ok(FileOrigin(path))
    }
}

#[derive(Debug)]
pub struct InputList {
    filenames: Vec<FileOrigin>,
}

impl InputList {
    pub fn as_string(&self) -> String {
        let mut buffer = String::new();
        for origin in self.filenames.iter() {
            buffer.push_str(&origin.0.to_string_lossy());
            buffer.push('\n');
        }
        buffer
    }
}

pub fn parse_walker(walk: Walk) -> Result<InputList, Box<dyn std::error::Error>> {
    let filenames = walk
        .map(|r| r.map(|p| FileOrigin::try_from(p.into_path())))
        .collect::<Result<Result<Vec<_>, _>, _>>()??;
    Ok(InputList { filenames })
}

pub fn parse_reader<T>(reader: T) -> Result<InputList, io::Error>
where
    T: Read,
{
    let buf_reader = BufReader::new(reader);
    let filenames = buf_reader
        .lines()
        .map(|r| r.map(|s| FileOrigin::try_from(PathBuf::from(s))))
        .collect::<Result<Result<Vec<_>, _>, _>>()??;
    Ok(InputList { filenames })
}

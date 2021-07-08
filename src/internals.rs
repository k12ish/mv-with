use std::convert::TryFrom;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;

use ignore::Walk;

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
    pub fn as_string(&self) -> String {
        let mut buffer = String::new();
        for origin in self.filenames.iter() {
            buffer.push_str(&origin.0.to_string_lossy());
            buffer.push('\n');
        }
        buffer
    }

    pub fn inner(self) -> Vec<FileOrigin> {
        self.filenames
    }
}

pub fn parse_walker(walk: Walk) -> Result<OriginList, Box<dyn std::error::Error>> {
    let filenames = walk
        .map(|r| r.map(|p| FileOrigin::try_from(p.into_path())))
        .collect::<Result<Result<Vec<_>, _>, _>>()??;
    Ok(OriginList { filenames })
}

pub fn parse_reader<R: Read>(reader: R) -> Result<OriginList, io::Error> {
    let buf_reader = BufReader::new(reader);
    let filenames = buf_reader
        .lines()
        .map(|r| r.map(|s| FileOrigin::try_from(PathBuf::from(s))))
        .collect::<Result<Result<Vec<_>, _>, _>>()??;
    Ok(OriginList { filenames })
}

pub struct RenameRequest {
    from: FileOrigin,
    to: PathBuf,
}

pub fn process_changes<T: Read>(origin_list: OriginList, target_list: T) -> Result<(), io::Error> {
    let mut target = BufReader::new(target_list).lines();
    for origin in origin_list.inner() {
        match target.next() {
            Some(t) => compare_lines(origin, t?),
            None => compare_lines(origin, String::new()),
        }
    }

    Ok(())
}

fn compare_lines(mut from: FileOrigin, mut to: String) {
    unimplemented!()
}

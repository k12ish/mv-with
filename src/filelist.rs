use std::fs;
use std::io;
use std::io::Read;
use std::path::PathBuf;

use ignore::Walk;

#[derive(Debug)]
pub struct FileList(Vec<PathBuf>);

impl FileList {
    pub fn as_string(&self) -> String {
        let mut buffer = String::new();
        for path in self.0.iter() {
            buffer.push_str(&path.to_string_lossy());
            buffer.push('\n');
        }
        buffer
    }

    pub fn validate(&self) -> Result<(), io::Error> {
        for path in &self.0 {
            fs::metadata(path)?;
        }
        Ok(())
    }
}

pub fn parse_walker(walk: Walk) -> Result<FileList, ignore::Error> {
    let (len, _) = walk.size_hint();
    let mut filenames = Vec::with_capacity(len);
    for dir_entry in walk {
        filenames.push(dir_entry?.into_path());
    }
    Ok(FileList(filenames))
}

pub fn parse_reader<T>(mut reader: T) -> Result<FileList, io::Error>
where
    T: Read,
{
    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;
    let mut filenames = buf.split('\n').map(PathBuf::from).collect::<Vec<_>>();
    filenames.retain(|s| s != &PathBuf::from(""));
    Ok(FileList(filenames))
}

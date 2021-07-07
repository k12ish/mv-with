use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;

use ignore::Walk;

#[derive(Debug)]
pub struct InputList {
    filenames: Vec<PathBuf>,
}

impl InputList {
    pub fn as_string(&self) -> String {
        let mut buffer = String::new();
        for path in self.filenames.iter() {
            buffer.push_str(&path.to_string_lossy());
            buffer.push('\n');
        }
        buffer
    }

    pub fn validate(&self) -> Result<(), io::Error> {
        for path in &self.filenames {
            fs::metadata(path)?;
        }
        Ok(())
    }
}

pub fn parse_walker(walk: Walk) -> Result<InputList, ignore::Error> {
    let filenames = walk
        .map(|r| r.map(|p| p.into_path()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InputList { filenames })
}

pub fn parse_reader<T>(reader: T) -> Result<InputList, io::Error>
where
    T: Read,
{
    let buf_reader = BufReader::new(reader);
    let filenames = buf_reader
        .lines()
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .filter(|&s| s != &String::new())
        .map(PathBuf::from)
        .collect();
    Ok(InputList { filenames })
}

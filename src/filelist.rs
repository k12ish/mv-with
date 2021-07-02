use std::convert::TryFrom;
use std::io;
use std::io::Read;
use std::io::Stdin;
use std::path::PathBuf;

use ignore::Walk;

#[derive(Debug)]
pub struct FileList {
    filenames: Vec<PathBuf>,
}

impl FileList {
    pub fn as_string(&self) -> String {
        let mut buffer = String::new();
        for path in self.filenames.iter() {
            buffer.push_str(&path.to_string_lossy());
            buffer.push_str("\n");
        }
        buffer
    }
}

impl TryFrom<Walk> for FileList {
    type Error = ignore::Error;

    fn try_from(walk: Walk) -> Result<Self, Self::Error> {
        let (len, _) = walk.size_hint();
        let mut filenames = Vec::with_capacity(len);
        for dir_entry in walk {
            filenames.push(dir_entry?.into_path());
        }
        Ok(FileList { filenames })
    }
}

impl TryFrom<Stdin> for FileList {
    type Error = io::Error;

    /// Limitation: Non-Unicode filenames cannot be processed
    fn try_from(mut stdin: Stdin) -> Result<Self, Self::Error> {
        let mut buf = String::new();
        stdin.read_to_string(&mut buf)?;
        let mut filenames = buf.split('\n').map(PathBuf::from).collect::<Vec<_>>();
        filenames.retain(|s| s != &PathBuf::from(""));
        Ok(FileList { filenames })
    }
}

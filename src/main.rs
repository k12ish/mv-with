use std::convert::TryFrom;
use std::fs;
use std::fs::ReadDir;
use std::io;
use std::io::Read;
use std::io::Stdin;
use std::path::PathBuf;
use std::process::Command;

use atty::Stream;
use clap::App;
use clap::Arg;

#[derive(Debug)]
struct FileList {
    filenames: Vec<PathBuf>,
}

impl TryFrom<ReadDir> for FileList {
    type Error = io::Error;

    fn try_from(read_dir: ReadDir) -> Result<Self, Self::Error> {
        let (len, _) = read_dir.size_hint();
        let mut filenames = Vec::with_capacity(len);
        for dir_entry in read_dir {
            filenames.push(dir_entry?.path());
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
        let filenames = buf.split('\n').map(PathBuf::from).collect();
        Ok(FileList { filenames })
    }
}

fn main() -> io::Result<()> {
    let matches = App::new("rename-via")
        .version("0.1")
        .author("Krish S. <k4krish@gmail.com>")
        .about("Renames files with your preferred editor")
        .arg(
            Arg::with_name("EDITOR")
                .help("Sets the editor to use")
                .required(true)
                .index(1),
        )
        .get_matches();

    let file_list = {
        if atty::is(Stream::Stdin) {
            FileList::try_from(fs::read_dir("./").unwrap()).expect("Error Reading Directory")
        } else {
            FileList::try_from(io::stdin()).expect("Error Reading StdIn")
        }
    };
    fs::write("/tmp/rename-via", format!("{:?}", file_list))?;

    let editor = matches.value_of("EDITOR").unwrap();
    Command::new("/usr/bin/sh")
        .arg("-c")
        .arg(format!("{} /tmp/rename-via", editor))
        .spawn()
        .expect("Error: Failed to run editor")
        .wait()
        .expect("Error: Editor returned a non-zero status");
    Ok(())
}

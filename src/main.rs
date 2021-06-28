use std::convert::TryFrom;
use std::fs;
use std::fs::ReadDir;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

use atty::Stream;
use clap::App;
use clap::Arg;

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

    let mut buffer = String::new();

    if atty::is(Stream::Stdin) {
        let read_dir = fs::read_dir("./").unwrap();
        let file_list = FileList::try_from(read_dir);
    } else {
        io::stdin().read_to_string(&mut buffer)?;
    }
    fs::write("/tmp/rename-via", buffer)?;

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

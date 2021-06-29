use std::convert::TryFrom;
use std::fs;
use std::io;
use std::io::Read;
use std::io::Stdin;
use std::path::PathBuf;
use std::process::Command;

use atty::Stream;
use clap::App;
use clap::Arg;
use ignore::Walk;
use ignore::WalkBuilder;

#[derive(Debug)]
struct FileList {
    filenames: Vec<PathBuf>,
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
        .arg(
            Arg::with_name("hidden")
                .long("hidden")
                .takes_value(false)
                .help("Search hidden files and directories"),
        )
        .get_matches();

    let file_list = {
        if atty::is(Stream::Stdin) {
            FileList::try_from(
                WalkBuilder::new("./")
                    .hidden(!matches.is_present("hidden"))
                    .build(),
            )
            .expect("Error Reading Directory")
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

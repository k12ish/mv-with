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
use colored::Colorize;
use dissimilar;
use dissimilar::Chunk;
use ignore::Walk;
use ignore::WalkBuilder;

#[derive(Debug)]
struct FileList {
    filenames: Vec<PathBuf>,
}

impl FileList {
    fn as_string(&self) -> String {
        let mut buffer = String::new();
        for path in self.filenames.iter() {
            match path.file_name() {
                None => buffer.push_str(&path.to_string_lossy()),
                Some(os_str) => buffer.push_str(&os_str.to_string_lossy()),
            }
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

fn main() -> io::Result<()> {
    let matches = App::new("rename-with")
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
    let before = file_list.as_string();
    fs::write("/tmp/rename-with", &before)?;

    let editor = matches.value_of("EDITOR").unwrap();
    Command::new("/usr/bin/sh")
        .arg("-c")
        .arg(format!("{} /tmp/rename-with", editor))
        .spawn()
        .expect("Error: Failed to run editor")
        .wait()
        .expect("Error: Editor returned a non-zero status");

    let mut file = fs::File::open("/tmp/rename-with")?;
    let mut after = String::new();
    file.read_to_string(&mut after)?;

    for (line_before, line_after) in before.lines().zip(after.lines()) {
        linewise_diff(line_before, line_after)
    }
    Ok(())
}

fn linewise_diff(line_before: &str, line_after: &str) {
    let mut line_diff = String::new();
    line_diff.reserve(line_before.len());
    let chunk_vec = dissimilar::diff(&line_before, &line_after);
    for chunk in chunk_vec {
        match chunk {
            Chunk::Equal(s) => line_diff.push_str(&format!("{}", s.normal())),
            Chunk::Delete(s) => line_diff.push_str(&format!("{}", s.red().strikethrough())),
            Chunk::Insert(s) => line_diff.push_str(&format!("{}", s.green())),
        }
    }
    if line_diff.len() == line_before.len() {
        line_diff = format!("{:<55}  {}", line_diff, "(unchanged)".italic().dimmed());
    }
    println!("{}", line_diff)
}

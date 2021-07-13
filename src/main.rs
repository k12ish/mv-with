use std::ffi::OsString;
use std::io;
use std::process::Command;
use std::process::Stdio;

use atty::Stream;
use clap::App;
use clap::Arg;
use ignore::WalkBuilder;
use question::Answer;
use question::Question;

mod internals;
use internals::FileList;

// TODO: use tempfile::NamedTempFile;
static TEMP_FILE: &str = "/tmp/rename-with";

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
        .get_matches();

    // TODO: Graceful error handling for empty stdin / dir
    let file_origins = {
        if atty::is(Stream::Stdin) {
            FileList::parse_walker(WalkBuilder::new("./").build())
        } else {
            FileList::parse_reader(io::stdin())
        }
    };
    // fs::write(TEMP_FILE, &file_origins.as_string())?;

    let editor = matches.value_of("EDITOR").unwrap();
    let mut command = OsString::new();
    command.push(editor);
    command.push(" ");
    command.push(TEMP_FILE);
    let output = Command::new("/usr/bin/sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to run bash")
        .wait_with_output()
        .unwrap();

    //TODO: Better error handling when editor is misspelt
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        match output.status.code() {
            Some(127) => panic!(
                "Bash returned exit status 127: Did you misspell {}?",
                editor
            ),
            _ => panic!("Bash returned unsuccessful exit status: {}", stderr),
        }
    }

    match Question::new("Do you want to continue?")
        .yes_no()
        .until_acceptable()
        .default(Answer::YES)
        .show_defaults()
        .ask()
    {
        Some(Answer::YES) | None => {
            println!("Applying changes")
        }
        Some(Answer::NO) => {
            println!("No changes made")
        }
        Some(Answer::RESPONSE(_)) => {
            unreachable!("Yes/No Question requires Yes/No answer")
        }
    }
    Ok(())
}

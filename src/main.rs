use std::fs;
use std::io;
use std::process::Command;
use std::process::Stdio;

use atty::Stream;
use clap::App;
use clap::Arg;
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::ColorChoice;
use codespan_reporting::term::termcolor::StandardStream;
use codespan_reporting::term::Config;
use ignore::WalkBuilder;
use lazy_static::lazy_static;
use question::Answer;
use question::Question;

mod internals;
use internals::errors::*;
use internals::*;

// TODO: use tempfile::NamedTempFile;
static TEMP_FILE: &str = "/tmp/rename-with";

lazy_static! {
    static ref WRITER: StandardStream = StandardStream::stderr(ColorChoice::Always);
    static ref CONFIG: Config = Config::default();
}

fn main() {
    let exit_code = real_main();
    std::process::exit(exit_code);
}

fn real_main() -> i32 {
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

    match file_origins.confirm_files_exist() {
        Err(err) => {
            let file = SimpleFile::new("StdIn", file_origins.as_str());
            term::emit(&mut WRITER.lock(), &CONFIG, &file, &err.report()).unwrap();
            return 1;
        }
        _ => {}
    };
    fs::write(TEMP_FILE, file_origins.as_str()).unwrap();

    let editor = matches.value_of("EDITOR").unwrap();
    let command = format!("{} {}", &editor, TEMP_FILE);
    let output = Command::new("/usr/bin/sh")
        .arg("-c")
        .arg(&command)
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to run bash")
        .wait_with_output()
        .unwrap();

    match output.status.code() {
        Some(127) => {
            let file = SimpleFile::new("", &command);
            let err = errors::MisspelledBashCommand(&editor);
            term::emit(&mut WRITER.lock(), &CONFIG, &file, &err.report()).unwrap();
            return 1;
        }
        _ => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                panic!("Bash returned unsuccessful exit status: {}", stderr)
            }
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
    0
}

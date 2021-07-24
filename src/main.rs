use std::fs;
use std::io;
use std::process::Command;

use atty::Stream;
use clap::App;
use clap::Arg;
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::ColorChoice;
use codespan_reporting::term::termcolor::StandardStream;
use codespan_reporting::term::Config;
use dialoguer::Confirm;
use ignore::WalkBuilder;
use lazy_static::lazy_static;

mod internals;
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

    let file_origins = {
        match {
            if atty::is(Stream::Stdin) {
                FileList::parse_walker(WalkBuilder::new("./").build())
            } else {
                FileList::parse_reader(io::stdin().lock())
            }
        } {
            Ok(file_origins) => file_origins,
            // Graceful error handling for empty stdin / directory
            Err(err) => {
                let file = SimpleFile::new("", "");
                term::emit(&mut WRITER.lock(), &CONFIG, &file, &err.report()).unwrap();
                return 1;
            }
        }
    };

    if let Err(err) = file_origins.confirm_files_exist() {
        let file = SimpleFile::new("StdIn", file_origins.as_str());
        term::emit(&mut WRITER.lock(), &CONFIG, &file, &err.report()).unwrap();
        return 1;
    };

    fs::write(TEMP_FILE, file_origins.as_str()).unwrap();

    let editor = matches.value_of("EDITOR").unwrap();
    let command = format!("{} {}", &editor, TEMP_FILE);
    let status = Command::new("/usr/bin/sh")
        .arg("-c")
        .arg(&command)
        .spawn()
        .expect("Failed to run bash")
        .wait()
        .unwrap();

    match status.code() {
        Some(127) => {
            let file = SimpleFile::new("", &command);
            let err = errors::MisspelledBashCommand(editor);
            term::emit(&mut WRITER.lock(), &CONFIG, &file, &err.report()).unwrap();
            return 1;
        }
        _ => {
            if !status.success() {
                panic!("Bash returned unsuccessful exit status")
            }
        }
    }

    if Confirm::new()
        .with_prompt("Do you want to continue?")
        .interact()
        .unwrap()
    {
        println!("Looks like you want to continue");
    } else {
        println!("nevermind then :(");
    }

    0
}

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
static TEMP_FILE: &str = "/tmp/mv-with";

lazy_static! {
    static ref WRITER: StandardStream = StandardStream::stderr(ColorChoice::Always);
    static ref CONFIG: Config = Config::default();
}

fn main() {
    let exit_code = real_main();
    std::process::exit(exit_code);
}

fn real_main() -> i32 {
    let matches = App::new("mv-with")
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
        let warning;
        match {
            if atty::is(Stream::Stdin) {
                warning = "StdIn is empty";
                FileList::parse_walker(WalkBuilder::new("./").build())
            } else {
                warning = "Directory is empty";
                FileList::parse_reader(io::stdin().lock())
            }
        } {
            Ok(file_origins) => file_origins,
            // Graceful error handling for empty stdin / directory
            Err(warn) => {
                let file = SimpleFile::new("", "");
                let diagnostic = &warn.report().with_message(warning);
                term::emit(&mut WRITER.lock(), &CONFIG, &file, diagnostic).unwrap();
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
            let diagnostic = &errors::MisspelledBashCommand(editor).report();
            term::emit(&mut WRITER.lock(), &CONFIG, &file, diagnostic).unwrap();
            return 1;
        }
        _ => {
            if !status.success() {
                panic!("Bash returned unsuccessful exit status")
            }
        }
    }

    let file_targets = {
        match FileList::parse_reader(fs::File::open(TEMP_FILE).unwrap()) {
            Ok(filelist) => filelist,
            Err(_) => {
                let file = SimpleFile::new("", "");
                let diagnostic = &errors::EmptyTempFile.report();
                term::emit(&mut WRITER.lock(), &CONFIG, &file, diagnostic).unwrap();
                return 1;
            }
        }
    };

    let mut request = {
        match RenameRequest::new(file_origins, file_targets) {
            Ok(filelist) => filelist,
            Err((buf, err)) => {
                let file = SimpleFile::new(TEMP_FILE, buf);
                let diagnostic = &err.report();
                term::emit(&mut WRITER.lock(), &CONFIG, &file, diagnostic).unwrap();
                return 1;
            }
        }
    };

    request.sort();

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

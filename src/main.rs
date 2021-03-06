use std::fs;
use std::io;
use std::process::Command;

use atty::Stream;
use clap::App;
use clap::Arg;
use codespan_reporting::diagnostic::Diagnostic;
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::ColorChoice;
use codespan_reporting::term::termcolor::StandardStream;
use codespan_reporting::term::Config;
use dialoguer::Confirm;
use ignore::WalkBuilder;

mod internals;
use internals::*;

// TODO: use tempfile::NamedTempFile;
static TEMP_FILE: &str = "/tmp/mv-with";

fn emit(file: SimpleFile<&str, String>, diagnostic: Diagnostic<()>) {
    let writer = StandardStream::stderr(ColorChoice::Always);
    let config = Config::default();
    term::emit(&mut writer.lock(), &config, &file, &diagnostic).unwrap();
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

    let editor = matches.value_of("EDITOR").unwrap();

    let mut file_origins = {
        match {
            if atty::is(Stream::Stdin) {
                FileList::parse_walker(
                    WalkBuilder::new("./")
                        .sort_by_file_path(|a, b| a.cmp(b))
                        .build(),
                )
            } else {
                FileList::parse_reader(io::stdin().lock())
            }
            .map(|f| f.confirm_files_exist())
        } {
            Ok(Ok(file_origins)) => file_origins,
            // Error handling for empty stdin / directory or invalid filename
            Ok(Err((buf, error))) | Err((buf, error)) => {
                let file = SimpleFile::new("Stdin", buf);
                let status = error.status().unwrap();
                emit(file, error.report());
                return status;
            }
        }
    };

    // Sort the files by decreasing file depth
    // Hence, file `foo/bar` is renamed before `foo`
    file_origins.sort_by_file_depth();

    fs::write(TEMP_FILE, file_origins.as_string()).unwrap();

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
            // Status 127 means that bash couldn't find the command; implies that
            // the command was likely misspelt
            let file = SimpleFile::new("", command);
            let diagnostic = internals::MisspelledBashCommand(editor).report();
            emit(file, diagnostic);
            return 1;
        }
        _ => {
            if !status.success() {
                panic!("Bash returned unsuccessful exit status:\n{:?}", status)
            }
        }
    }

    let file_targets = FileList::parse_reader(fs::File::open(TEMP_FILE).unwrap())
        .expect("Temporary file should not be empty");

    let request = {
        match RenameRequest::new(file_origins, file_targets) {
            Ok(filelist) => filelist,
            Err((buf, err)) => {
                // Error handling for incompatible file_origins and file_targets
                let file = SimpleFile::new(TEMP_FILE, buf);
                let status = err.status().unwrap();
                emit(file, err.report());
                return status;
            }
        }
    };

    request.print_diffs();

    if !Confirm::new()
        .with_prompt("Do you want to continue?")
        .interact()
        .unwrap()
    {
        println!("nevermind then :(");
        return 0;
    }

    if let Err(error) = request.rename() {
        let file = SimpleFile::new("", "".into());
        emit(file, error.report());
        return 1;
    };
    0
}

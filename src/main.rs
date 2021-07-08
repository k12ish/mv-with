use std::fs;
use std::io;
use std::process::Command;

use atty::Stream;
use clap::App;
use clap::Arg;
use ignore::WalkBuilder;
use question::Answer;
use question::Question;

mod internals;
use internals::OriginList;

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

    let file_origins = {
        if atty::is(Stream::Stdin) {
            OriginList::from_walker(WalkBuilder::new("./").build())
                .expect("Error Reading Directory")
        } else {
            OriginList::from_reader(io::stdin()).expect("Error Reading StdIn")
        }
    };
    fs::write(TEMP_FILE, &file_origins.as_string())?;

    let editor = matches.value_of("EDITOR").unwrap();
    Command::new("/usr/bin/sh")
        .arg("-c")
        .arg(format!("{} {}", editor, TEMP_FILE))
        .spawn()
        .expect("Error: Failed to run editor")
        .wait()
        .expect("Error: Editor returned a non-zero status");

    let file = fs::File::open(TEMP_FILE)?;
    internals::process_changes(file_origins, file)?;

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

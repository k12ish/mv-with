use std::fs;
use std::io;
use std::io::Read;
use std::process::Command;

use atty::Stream;
use clap::App;
use clap::Arg;

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
        buffer.push_str("No StdIn :(");
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

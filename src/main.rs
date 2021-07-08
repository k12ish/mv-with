use std::fs;
use std::io;
use std::io::Read;
use std::process::Command;

use atty::Stream;
use clap::App;
use clap::Arg;
use colored::Colorize;
use dissimilar::Chunk;
use ignore::WalkBuilder;
use question::Answer;
use question::Question;

mod internals;

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

    let origin_list = {
        if atty::is(Stream::Stdin) {
            internals::parse_walker(WalkBuilder::new("./").build())
                .expect("Error Reading Directory")
        } else {
            internals::parse_reader(io::stdin()).expect("Error Reading StdIn")
        }
    };
    let before = origin_list.as_string();
    fs::write(TEMP_FILE, &before)?;

    let editor = matches.value_of("EDITOR").unwrap();
    Command::new("/usr/bin/sh")
        .arg("-c")
        .arg(format!("{} {}", editor, TEMP_FILE))
        .spawn()
        .expect("Error: Failed to run editor")
        .wait()
        .expect("Error: Editor returned a non-zero status");

    let mut file = fs::File::open(TEMP_FILE)?;
    let mut after = String::new();
    file.read_to_string(&mut after)?;
    after.truncate(after.trim_end().len());

    let mut line_before = before.lines();
    let mut line_after = after.lines();
    loop {
        match (line_before.next(), line_after.next()) {
            (Some(b), Some(a)) => linewise_diff(b, a),
            (Some(b), None) => linewise_diff(b, ""),
            (None, _) => break,
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

fn linewise_diff(line_before: &str, line_after: &str) {
    let mut line_diff = String::new();
    let chunk_vec = dissimilar::diff(&line_before, &line_after);

    // The padding is calculated manually because ANSI escape codes interfere with
    // formatting strings
    // Eg. These bars will not appear vertically aligned for formatted text
    //
    // println!("{:<55} |", &format!("{}", "text"));
    // println!("{:<55} |", &format!("{}", "text".normal()));
    // println!("{:<55} |", &format!("{}", "text".red()));
    // println!("{:<55} |", &format!("{}", "text".red().strikethrough()));
    let diff_len: isize = chunk_vec
        .iter()
        .map(|s| match s {
            Chunk::Equal(s) => s.chars().count(),
            Chunk::Delete(s) => s.chars().count(),
            Chunk::Insert(s) => s.chars().count(),
        } as isize)
        .sum();
    let padding_len = std::cmp::max(55 - diff_len, 0) as usize;
    let padding = std::iter::repeat(' ').take(padding_len).collect::<String>();

    let comment;
    match chunk_vec[..] {
        [Chunk::Equal(s)] => {
            line_diff.push_str(&format!("{}", s.normal()));
            comment = "(unchanged)".italic().dimmed()
        }
        _ => {
            for chunk in chunk_vec {
                match chunk {
                    Chunk::Equal(s) => line_diff.push_str(&format!("{}", s.normal())),
                    Chunk::Delete(s) => line_diff.push_str(&format!("{}", s.red().strikethrough())),
                    Chunk::Insert(s) => line_diff.push_str(&format!("{}", s.green())),
                }
            }
            comment = "(renamed)".italic()
        }
    }
    println!("  {}{}  {}", line_diff, padding, comment)
}

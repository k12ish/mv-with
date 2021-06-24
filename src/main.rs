use clap::App;
use clap::Arg;
use std::fs;
use std::io;
use std::io::Read;

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
    io::stdin().read_to_string(&mut buffer)?;
    fs::write("/tmp/rename-via", buffer)?;
    Ok(())
}

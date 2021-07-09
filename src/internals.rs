use std::convert::TryFrom;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;

use colored::Colorize;
use dissimilar::Chunk;
use ignore::Walk;

/// `FileOrigin` instances are guarenteed to correspond to a file
/// They are wrappers of the `PathBuf` type (akin to an `OsString`)
#[derive(Debug)]
pub struct FileOrigin(PathBuf);

impl TryFrom<PathBuf> for FileOrigin {
    type Error = io::Error;
    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        fs::metadata(&path)?;
        Ok(FileOrigin(path))
    }
}

#[derive(Debug)]
pub struct OriginList {
    filenames: Vec<FileOrigin>,
}

impl OriginList {
    /// Lossily convert all paths into a String, separated by '\n'
    /// Used to display the files in an editor for the user
    pub fn as_string(&self) -> String {
        let mut buffer = String::new();
        for origin in self.filenames.iter() {
            buffer.push_str(&origin.0.to_string_lossy());
            buffer.push('\n');
        }
        buffer
    }

    /// Creates an `OriginList` from an `ignore::Walk`
    /// Used in the case where StdIn is empty and files in the current dir are read.
    ///
    /// Errors may arise if:
    ///   * File metadata cannot be accessed:
    ///     - User lacks sufficient permissions
    ///     - path does not exist
    ///   * Some sort of I/O error during iteration
    pub fn from_walker(walk: Walk) -> Result<OriginList, Box<dyn std::error::Error>> {
        let filenames = walk
            .map(|r| r.map(|p| FileOrigin::try_from(p.into_path())))
            .collect::<Result<Result<Vec<_>, _>, _>>()??;
        Ok(OriginList { filenames })
    }

    /// Creates an `OriginList` from a reader
    /// Used to parse StdIn.
    ///
    /// Errors may arise if:
    ///   * File metadata cannot be accessed:
    ///     - User lacks sufficient permissions
    ///     - path does not exist
    pub fn from_reader<R: Read>(reader: R) -> Result<OriginList, io::Error> {
        let filenames = BufReader::new(reader)
            .lines()
            .map(|r| r.map(|s| FileOrigin::try_from(PathBuf::from(s))))
            .collect::<Result<Result<Vec<_>, _>, _>>()??;
        Ok(OriginList { filenames })
    }
}

pub struct RenameRequest {
    from: FileOrigin,
    to: PathBuf,
}

impl RenameRequest {
    fn diff(&self) -> String {
        let line_before = self.from.0.to_string_lossy();
        let line_after = self.to.to_string_lossy();
        let mut line_diff = String::new();
        let chunk_vec = dissimilar::diff(&line_before, &line_after);

        // The padding is calculated manually because ANSI escape codes interfere with
        // formatting strings, so "{:<55}" produces inconsistent alignment
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
                        Chunk::Delete(s) => {
                            line_diff.push_str(&format!("{}", s.red().strikethrough()))
                        }
                        Chunk::Insert(s) => line_diff.push_str(&format!("{}", s.green())),
                    }
                }
                comment = "(renamed)".italic()
            }
        }
        format!("  {}{}  {}", line_diff, padding, comment)
    }
}

pub fn process_changes<T: Read>(origin_list: OriginList, target_list: T) -> Result<(), io::Error> {
    let mut target = BufReader::new(target_list).lines();
    let mut vec = Vec::new();
    for origin in origin_list.filenames {
        let request = {
            match target.next() {
                Some(t) => compare_lines(origin, PathBuf::from(t?)),
                None => compare_lines(origin, PathBuf::new()),
            }
        };
        println!("{}", request.diff());
        vec.push(request);
    }

    Ok(())
}

fn compare_lines(from: FileOrigin, to: PathBuf) -> RenameRequest {
    RenameRequest { from, to }
}

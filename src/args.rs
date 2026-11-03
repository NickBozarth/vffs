use std::path::PathBuf;

use clap::Parser;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArgParseError {
    #[error("Provided source_dir is not a valid directory")]
    SourceIsNotDir,
    #[error("Provided out_dir is not a valid directory")]
    OutDirIsNotDir,
    #[error("Failed to construct rules_file from provided arguments")]
    RuleConstructError,
}

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    source_dir: PathBuf,
    #[arg(short, long)]
    out_dir: PathBuf,
    #[arg(short, long)]
    rules_file: Option<PathBuf>,
}

impl Args {
    pub fn into_validated(self) -> Result<(PathBuf, PathBuf, PathBuf), ArgParseError> {
        let Args { source_dir, out_dir, rules_file } = self;

        if !source_dir.is_dir() {
            return Err(ArgParseError::SourceIsNotDir);
        }
        if !out_dir.is_dir() {
            return Err(ArgParseError::OutDirIsNotDir);
        }

        let rules_file = match rules_file {
            Some(file) => file,
            None => {
                let source_basename = source_dir.file_name()
                    .ok_or(ArgParseError::RuleConstructError)?;
                let out_basename = out_dir.file_name()
                    .ok_or(ArgParseError::RuleConstructError)?;
                let mut home_dir = home::home_dir()
                    .ok_or(ArgParseError::RuleConstructError)?;

                home_dir.push("vffs");
                home_dir.push(source_basename);
                home_dir.push(out_basename);
                home_dir.set_extension("yaml");
                home_dir
            }
        };

        Ok((source_dir, out_dir, rules_file))
    }
}

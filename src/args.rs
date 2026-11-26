use std::path::PathBuf;

use clap::Parser;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArgParseError {
    #[error("Failed to canonicalize path `{0}`: {1}")]
    Canonicalize(PathBuf, std::io::Error),
    #[error("Provided source_dir is not a valid directory")]
    SourceIsNotDir,
    #[error("out_dir and source_dir must not contain one another")]
    MountOverlapsSource,
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

// rules_file will either be provided or default to
// {HOME}/.config/vffs/{source_basename}/{out_basename}.yaml
pub struct ValidatedArgs {
    pub source_dir: PathBuf,
    pub mountpoint: PathBuf,
    pub rules_file: PathBuf,
}

impl Args {
    pub fn into_validated(self) -> Result<ValidatedArgs, ArgParseError> {
        let Args { source_dir, out_dir, rules_file } = self;

        if !source_dir.is_dir() { return Err(ArgParseError::SourceIsNotDir); }
        if !out_dir.is_dir() { return Err(ArgParseError::OutDirIsNotDir); }

        let source_dir = source_dir.canonicalize()
            .map_err(|e| ArgParseError::Canonicalize(source_dir.clone(), e))?;
        let out_dir = out_dir.canonicalize()
            .map_err(|e| ArgParseError::Canonicalize(out_dir.clone(), e))?;

        if out_dir.starts_with(&source_dir) || source_dir.starts_with(&out_dir) {
            return Err(ArgParseError::MountOverlapsSource);
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

                let mut file_name = out_basename.to_os_string();
                file_name.push(".yaml");

                home_dir.push(".config");
                home_dir.push("vffs");
                home_dir.push(source_basename);
                home_dir.push(file_name);
                home_dir
            }
        };

        Ok(ValidatedArgs { source_dir, mountpoint: out_dir, rules_file })
    }
}

use thiserror::Error;

use crate::rules::{gen_fn::GenArg, rules::ActionType};


#[derive(Debug, Error)]
pub enum RuleManagerError {
    #[error("Io Error: {0}")]
    Io(std::io::Error),
    #[error("Error attempting to write: {0}")]
    WriteError(std::io::Error),
    #[error("Error when parsing yaml")]
    YamlParseError(yaml_serde::Error),
}

pub type RuleManagerResult<T> = Result<T, RuleManagerError>;


#[derive(Debug, Error)]
pub enum GenArgParseError {
    #[error("Could not parse GenArg [requested({1}) : found GenArg::{0:?}]")]
    VariantMismatch(GenArg, &'static str),
    #[error("Attempted to parse a GenArg::Void")]
    VoidParseError,
}

#[derive(Debug, Error)]
pub enum ModError {
    #[error("Attempted to match on `{0}` which was not covered")]
    ActionNotCovered(ActionType),
    #[error("Error when attempting to parse args: {0}")]
    ArgParseError(GenArgParseError),
    #[error("Io Error {0}")]
    Io(std::io::Error),
    #[error("Callback not found")]
    CallbackNotFound,

    #[error("Other {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>)
}

impl ModError {
    pub fn into_cb_error(self, rule_name: String, callback_name: String) -> CallbackError {
        CallbackError {
            rule_name,
            callback_name,
            mod_error: self
        }
    }
}

pub type ModResult<T> = Result<T, ModError>;

impl From<GenArgParseError> for ModError {
    fn from(e: GenArgParseError) -> Self { Self::ArgParseError(e) }
}

impl From<std::io::Error> for ModError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}



#[derive(Debug, Error)]
#[error("Rule `{rule_name}` => Callback `{callback_name}` encountered error: {mod_error}")]
pub struct CallbackError {
    rule_name: String,
    callback_name: String,
    mod_error: ModError
}

pub type CallbackResult<T> = Result<T, CallbackError>;



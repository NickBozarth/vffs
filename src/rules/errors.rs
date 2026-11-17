use thiserror::Error;

use crate::rules::gen_fn::GenArg;


#[derive(Debug, Error)]
pub enum GenArgParseError {
    #[error("Could not parse GenArg [requested({1}) : found GenArg::{0:?}]")]
    VariantMismatch(GenArg, &'static str),
    #[error("Attempted to parse a GenArg::Void")]
    VoidParseError,
}


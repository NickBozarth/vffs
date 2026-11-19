use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::rules::{errors::GenArgParseError, rules::{ActionType, PreparseCondition}};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct GenFn {
    pub fn_name: String,
    #[serde(default)]
    pub args: GenArgs
}



macro_rules! define_mod_arg {
    ($($variant:ident => $ty:ty),* $(,)?) => {
        #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
        pub enum GenArg {
            $($variant($ty)),*
        }

        $(
            impl TryFrom<GenArg> for $ty {
                type Error = GenArgParseError;

                fn try_from(arg: GenArg) -> Result<Self, Self::Error> {
                    match arg {
                        GenArg::$variant(v) => Ok(v),
                        other => Err(GenArgParseError::VariantMismatch(other, stringify!( $ty )))
                    }
                }
            }


            impl TryFrom<GenArgs> for $ty {
                type Error = GenArgParseError;

                fn try_from(args: GenArgs) -> Result<Self, Self::Error> {
                    let (arg,) = args.try_into()?;
                    Ok(arg)
                }
            }
        )*
    };
}


define_mod_arg!(
    Bool   => bool,
    Char   => char,
    String => String,
    I8     => i8,
    I16    => i16,
    I32    => i32,
    I64    => i64,
    I128   => i128,
    Isize  => isize,
    U8     => u8,
    U16    => u16,
    U32    => u32,
    U64    => u64,
    U128   => u128,
    Usize  => usize,
    F32    => f32,
    F64    => f64,
    ActionType => ActionType,
    PreparseCondition => PreparseCondition,
    RngSeed => RngSeed,
    TimeAmount => TimeAmount,
);


#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum RngSeed {
    CurrentTimeAsSeed,
    Seeded(u64)
}

impl From<RngSeed> for u64 {
    fn from(seed: RngSeed) -> Self {
        match seed {
            RngSeed::Seeded(seed) => seed,
            RngSeed::CurrentTimeAsSeed => {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            }
        }
    }
}



#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct GenArgs(pub Vec<GenArg>);

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum TimeAmount {
    Nanos(u128),
    Micros(u64),
    Millis(u64),
    Secs(u64),
    Mins(u64),
    Hours(u64),
    Days(u64),
}

const SECS_PER_MIN: u64 = 60;
const SECS_PER_HOUR: u64 = 3_600;
const SECS_PER_DAY: u64 = 86_400;

impl From<TimeAmount> for Duration {
    fn from(amount: TimeAmount) -> Self {
        match amount {
            TimeAmount::Nanos(n) => {
                let secs = u64::try_from(n / 1_000_000_000).unwrap_or(u64::MAX);
                let nanos = (n % 1_000_000_000) as u32;
                Duration::new(secs, nanos)
            }
            TimeAmount::Micros(us) => Duration::from_micros(us),
            TimeAmount::Millis(ms) => Duration::from_millis(ms),
            TimeAmount::Secs(s) => Duration::from_secs(s),
            TimeAmount::Mins(m) => Duration::from_secs(m.saturating_mul(SECS_PER_MIN)),
            TimeAmount::Hours(h) => Duration::from_secs(h.saturating_mul(SECS_PER_HOUR)),
            TimeAmount::Days(d) => Duration::from_secs(d.saturating_mul(SECS_PER_DAY)),
        }
    }
}


// Implement TryFrom<GenArgs> for tuples where all elements implement TryFrom<GenArg>.
// The argument count must match the tuple arity exactly.
macro_rules! try_from_gen_args {
    ($len:literal: $( $generic:ident ),+) => {
        impl< $( $generic ),* > TryFrom<GenArgs> for ( $( $generic, )* )
        where
            $( $generic : TryFrom<GenArg, Error = GenArgParseError> ),*
        {
            type Error = GenArgParseError;
            fn try_from(args: GenArgs) -> Result<Self, Self::Error> {
                if args.0.len() != $len {
                    return Err(GenArgParseError::LengthMismatch(args.0.len(), $len));
                }

                let mut it = args.0.into_iter();
                Ok((
                    $( <$generic>::try_from(it.next().unwrap())?, )*
                ))
            }
        }
    }
}

try_from_gen_args!(1:  A);
try_from_gen_args!(2:  A, B);
try_from_gen_args!(3:  A, B, C);
try_from_gen_args!(4:  A, B, C, D);
try_from_gen_args!(5:  A, B, C, D, E);
try_from_gen_args!(6:  A, B, C, D, E, F);
try_from_gen_args!(7:  A, B, C, D, E, F, G);
try_from_gen_args!(8:  A, B, C, D, E, F, G, H);
try_from_gen_args!(9:  A, B, C, D, E, F, G, H, I);
try_from_gen_args!(10: A, B, C, D, E, F, G, H, I, J);

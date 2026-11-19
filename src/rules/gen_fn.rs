use std::{fmt, str::FromStr, time::Duration};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
#[derive(Debug, PartialEq, Clone)]
pub enum TimeAmount {
    Nanos(u128),
    Micros(u64),
    Millis(u64),
    Secs(u64),
    Mins(u64),
    Hours(u64),
    Days(u64),
}

impl fmt::Display for TimeAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nanos(n) => write!(f, "{n}ns"),
            Self::Micros(n) => write!(f, "{n}us"),
            Self::Millis(n) => write!(f, "{n}ms"),
            Self::Secs(n) => write!(f, "{n}s"),
            Self::Mins(n) => write!(f, "{n}m"),
            Self::Hours(n) => write!(f, "{n}h"),
            Self::Days(n) => write!(f, "{n}d"),
        }
    }
}

impl FromStr for TimeAmount {
    type Err = String;

    // Accepts "50ms", "2s", "5 m", ...
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let split = s
            .find(|c: char| !c.is_ascii_digit())
            .ok_or_else(|| format!("missing unit in `{s}` (e.g. 50ms)"))?;
        let (num, unit) = s.split_at(split);
        let bad = |e| format!("invalid number in `{s}`: {e}");

        Ok(match unit.trim() {
            "ns" => Self::Nanos(num.parse().map_err(bad)?),
            "us" => Self::Micros(num.parse().map_err(bad)?),
            "ms" => Self::Millis(num.parse().map_err(bad)?),
            "s" => Self::Secs(num.parse().map_err(bad)?),
            "m" => Self::Mins(num.parse().map_err(bad)?),
            "h" => Self::Hours(num.parse().map_err(bad)?),
            "d" => Self::Days(num.parse().map_err(bad)?),
            other => return Err(format!("unknown unit `{other}` in `{s}` (use ns, us, ms, s, m, h, d)")),
        })
    }
}

impl Serialize for TimeAmount {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TimeAmount {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
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

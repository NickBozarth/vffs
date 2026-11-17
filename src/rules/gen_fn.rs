use serde::{Deserialize, Serialize};

use crate::rules::{errors::GenArgParseError, rules::ActionType};

#[derive(Debug, Serialize, Deserialize)]
pub struct GenFn {
    pub fn_name: String,
    pub args: GenArgs
}



macro_rules! define_mod_arg {
    ($($variant:ident => $ty:ty),* $(,)?) => {
        #[derive(Debug, Serialize, Deserialize, PartialEq, Default, Clone)]
        pub enum GenArg {
            #[default]
            Void,
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
);


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenArgs(pub Vec<GenArg>);


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

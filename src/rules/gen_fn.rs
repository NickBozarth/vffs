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

                fn try_from(mod_arg: GenArg) -> Result<Self, Self::Error> {
                    if let GenArg::$variant(arg) = mod_arg {
                        Ok(arg)
                    } else if mod_arg == GenArg::Void {
                        Err(GenArgParseError::VoidParseError)
                    } else {
                        Err(GenArgParseError::VariantMismatch(mod_arg, stringify!( $ty )))
                    }
                }
            }


            impl TryFrom<GenArgs> for $ty {
                type Error = GenArgParseError;

                fn try_from(mod_args: GenArgs) -> Result<Self, Self::Error> {
                    if mod_args.arg1 != GenArg::Void
                    || mod_args.arg2 != GenArg::Void
                    || mod_args.arg3 != GenArg::Void
                    || mod_args.arg4 != GenArg::Void
                    || mod_args.arg5 != GenArg::Void
                    || mod_args.arg6 != GenArg::Void
                    || mod_args.arg7 != GenArg::Void
                    || mod_args.arg8 != GenArg::Void
                    || mod_args.arg9 != GenArg::Void
                    {
                        Err(GenArgParseError::VoidParseError)
                    } else {
                        mod_args.arg0.try_into()
                    }
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


#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct GenArgs {
    pub arg0: GenArg,
    pub arg1: GenArg,
    pub arg2: GenArg,
    pub arg3: GenArg,
    pub arg4: GenArg,
    pub arg5: GenArg,
    pub arg6: GenArg,
    pub arg7: GenArg,
    pub arg8: GenArg,
    pub arg9: GenArg,
}

type GenArgsTuple = (GenArg, GenArg, GenArg, GenArg, GenArg, GenArg, GenArg, GenArg, GenArg, GenArg);
impl From<GenArgsTuple> for GenArgs {
    fn from(value: GenArgsTuple) -> Self {
        let (
            arg0, arg1, arg2, arg3, arg4,
            arg5, arg6, arg7, arg8, arg9
        ) = value;

        Self { arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 }
    }
}



// Implement TryFrom<GenArg> for tuples where all elements implement TryFrom<ModArg>
// used: a list of generics and their mapping to what fielt they correspond to
// unused: a list of fields that will be checked to ensure that incorrectly sized GenArgs do
//         not parse to valid tuple
macro_rules! try_from_mod_args {
    (
        used: [ $( $generic:ident => $field:ident ),+],
        unused: [$( $unused:ident ),*]
    ) =>{
        impl<$( $generic, )+> TryFrom<GenArgs> for ($( $generic, )+)
        where
            $( $generic: TryFrom<GenArg, Error = GenArgParseError>, )+
        {
            type Error = GenArgParseError;
            fn try_from(mod_args: GenArgs) -> Result<Self, Self::Error> {
                if false
                $( || mod_args.$unused != GenArg::Void )*
                {
                    return Err(GenArgParseError::VoidParseError);
                }

                Ok((
                    $( mod_args.$field.try_into()?, )+
                ))
            }

        }
    };
}

try_from_mod_args!(
    used: [A => arg0, B => arg1],
    unused: [arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9]
);

try_from_mod_args!(
    used: [A => arg0, B => arg1, C => arg2],
    unused: [arg3, arg4, arg5, arg6, arg7, arg8, arg9]
);

try_from_mod_args!(
    used: [A => arg0, B => arg1, C => arg2, D => arg3],
    unused: [arg4, arg5, arg6, arg7, arg8, arg9]
);

try_from_mod_args!(
    used: [A => arg0, B => arg1, C => arg2, D => arg3, E => arg4],
    unused: [arg5, arg6, arg7, arg8, arg9]
);

try_from_mod_args!(
    used: [A => arg0, B => arg1, C => arg2, D => arg3, E => arg4, F => arg5],
    unused: [arg6, arg7, arg8, arg9]
);

try_from_mod_args!(
    used: [
        A => arg0, B => arg1, C => arg2, D => arg3, E => arg4, F => arg5,
        G => arg6
    ],
    unused: [arg7, arg8, arg9]
);

try_from_mod_args!(
    used: [
        A => arg0, B => arg1, C => arg2, D => arg3, E => arg4, F => arg5,
        G => arg6, H => arg7
    ],
    unused: [arg8, arg9]
);

try_from_mod_args!(
    used: [
        A => arg0, B => arg1, C => arg2, D => arg3, E => arg4, F => arg5,
        G => arg6, H => arg7, I => arg8
    ],
    unused: [arg9]
);

try_from_mod_args!(
    used: [
        A => arg0, B => arg1, C => arg2, D => arg3, E => arg4, F => arg5,
        G => arg6, H => arg7, I => arg8, J => arg9
    ],
    unused: []
);

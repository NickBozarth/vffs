use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::rules::gen_fn::{GenArg, GenFn};




#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum RuleType {
    PreAction,
    PostAction
}


#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
pub enum MatchType {
    #[default]
    Inactive,
    MatchAll,
    MatchOne,
    DenyMatchAll,
    DenyMatchOne,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone, Copy)]
pub enum ActionType {
    #[default]
    All,
    None,

    Lookup,
    Forget,
    Getattr,
    Setattr,
    Readdir,
    Open,
    Read,
    Write,
    Release,
    Fsync,
}





#[derive(Debug, Serialize, Deserialize)]
pub struct PreparseRules {
    pub rule_map: HashMap<String, PreparseRule>
}

impl PreparseRules {
    pub fn parse(self) -> Vec<Rule> {
        self.rule_map
            .into_iter()
            .map(|(name, ppr)| ppr.parse(name))
            .collect()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreparseRule {
    #[serde(default)]
    pub inactive: bool,
    pub rule_type: RuleType,
    pub condition_match_type: MatchType,
    pub conditions: Vec<PreparseCondition>,
    pub modifications: Vec<PreparseModification>
}

impl PreparseRule {
    pub fn parse(self, rule_name: String) -> Rule {
        let PreparseRule {
            inactive,
            rule_type,
            condition_match_type,
            conditions,
            modifications
        } = self;

        let conditions = conditions
            .into_iter()
            .map(|ppc| ppc.parse())
            .collect();

        let modifications = modifications
            .into_iter()
            .map(|ppm| ppm.parse())
            .collect();

        Rule {
            name: rule_name,
            inactive,
            rule_type,
            condition_match_type,
            conditions,
            modifications
        }
    }
}


macro_rules! parse_to_gen_fn {

    (
        // usually just self
        $match_on:expr,
        $arm_variant_prefix:ident,
        $(
            // enum variant that is optionally a tuple enum type to accept args
            // ex: Self::ActionType(ty)
            $variant:ident $( ( $( $item:ident ),* ) )?
            =>
            // Capture name of function and arguments (preserving order declared here)
            // argument type must correspond to a GenArg variant (Bool, U8, String, ActionType, ...)
            // ex: match_action(ty: ActionType)
            $fn_name:ident ( $( $arg_type:ident $arg_name:expr ),* )
        ),*
        // Optional comma delimiter at the end
        $(,)?
    ) => {
        match $match_on {
            $(
                $arm_variant_prefix::$variant $( ( $( $item ),* ) )? => { 
                    GenFn {
                        fn_name: stringify!( $fn_name ).to_string(),
                        args: parse_to_gen_fn! { @gen_args $( $arg_type $arg_name ),* }.into()
                    }
                },
            )*

            $arm_variant_prefix::Fn(gen_fn) => gen_fn
        }
    };

    (@gen_args $( $arg_type:ident $arg_expr:expr ),*) => {
        // Take all args and parse them
        // Add 10 GenArg::Void onto the end of args
        // Truncate args to 10 args
        parse_to_gen_fn! {
            @truncate10
            $( GenArg::$arg_type( $arg_expr ), )*
            GenArg::Void, GenArg::Void, GenArg::Void, GenArg::Void, GenArg::Void, 
            GenArg::Void, GenArg::Void, GenArg::Void, GenArg::Void, GenArg::Void
        }
    };

    // Truncate macro call of at least 10 to 10 args
    (
        @truncate10
        $arg0:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr,
        $arg5:expr, $arg6:expr, $arg7:expr, $arg8:expr, $arg9:expr,
        $( $_ignored_args:expr ),*
    ) => {
        (
            $arg0, $arg1, $arg2, $arg3, $arg4,
            $arg5, $arg6, $arg7, $arg8, $arg9
        )
    };
}


// Define a set of condition types
// The conditions will all just parse to a GenFn when parsed
#[derive(Debug, Serialize, Deserialize)]
pub enum PreparseCondition {
    ActionType(ActionType),
    MatchGlob(String),

    Fn(GenFn)
}

impl PreparseCondition {
    pub fn parse(self) -> Condition {
        let gen_fn = parse_to_gen_fn!(
            self,
            Self,
            ActionType(ty) => match_action(ActionType ty),
            MatchGlob(glob) => match_glob(String glob)
        );

        Condition(gen_fn)
    }
}


#[derive(Debug, Serialize, Deserialize)]
pub enum PreparseModification {
    WaitPct(f32),
    WaitPctDbg(f32),

    Fn(GenFn)
}

impl PreparseModification {
    pub fn parse(self) -> Modification {
        let gen_fn = parse_to_gen_fn!(
            self,
            Self,
            WaitPct(pct) => wait_pct(F32 pct, Bool false),
            WaitPctDbg(pct) => wait_pct(F32 pct, Bool false)
        );

        Modification(gen_fn)
    }
}





pub struct Rule {
    pub name: String,
    pub inactive: bool,
    pub rule_type: RuleType,
    pub condition_match_type: MatchType,
    pub conditions: Vec<Condition>,
    pub modifications: Vec<Modification>
}

pub struct Condition(pub GenFn);

pub struct Modification(pub GenFn);

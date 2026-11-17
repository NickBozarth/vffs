use std::{collections::HashMap, fmt::Display};

use serde::{Deserialize, Serialize};

use crate::rules::{context::{ConditionCallback, Context}, errors::ModError, gen_fn::{GenArg, GenFn}};




#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum RuleType {
    PreAction,
    PostAction
}


#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum MatchType {
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

impl Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ActionType({self:?})")
    }
}




#[derive(Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct PreparseRules(pub HashMap<String, PreparseRule>);

impl PreparseRules {
    pub fn parse(self) -> Vec<Rule> {
        self.0
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


// Generalizes pattern to GenFn so that it can be procedurally called later
//
// Structured like
// {
//      match_on,
//      arm_variant_prefix,
//
//      $(
//          variant( $( args )* ) => fn_name ( $( arg_type, arg_name ),* )
//      )*
// }
//
//
// When written out, it can look like
// {
//      self,
//      Self,
//
//      WaitPct(pct) => wait_pct(F32 pct)
// }
//
// Which will parse to
// GenFn {
//      fn_name: "wait_pct".to_string(),
//      args: GenArgs {
//
//      }
// }
macro_rules! parse_to_gen_fn {

    (
        // what is being matched on
        $match_on:expr,
        // the type of what is being matched on
        $arm_variant_prefix:ident,
        $(
            // enum variant that is optionally a tuple enum type to accept args
            // ex: ActionType(ty)
            //     WaitPct(pct)
            $variant:ident $( ( $( $item:ident ),* ) )?
            =>
            // Capture name of function and arguments (preserving order declared here)
            // argument type must correspond to a GenArg variant (Bool, U8, String, ActionType, ...)
            // ex: match_action(ActionType ty)
            //     wait_pct(F32 pct, Bool false)
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

impl Rule {
    pub fn conditions_match<CT>(
        &self,
        cond_callbacks: &HashMap<String, ConditionCallback<CT>>,
        ctx: &Context<CT>
    ) -> bool {
        if self.inactive {
            return false;
        }

        let mut condition_check = self.conditions
            .iter()
            .filter_map(|c| {
                match cond_callbacks.get(&c.0.fn_name) {
                    Some(cb) => {
                        let res = cb(c.0.args.clone(), ctx);
                        match res {
                            Ok(status) => Some(status),
                            Err(e) => {
                                eprint!("{e}");
                                None
                            }
                        }
                    },
                    None => {
                        let e = ModError::CallbackNotFound.into_cb_error(
                            self.name.clone(),
                            c.0.fn_name.clone()
                        );

                        eprintln!("{e}");
                        None
                    }
                }
            })
            .peekable();

        if condition_check.peek().is_none() {
            return false;
        }

        match self.condition_match_type {
            MatchType::MatchAll => { condition_check.all(|b| b) }
            MatchType::MatchOne => { condition_check.any(|b| b) }
            MatchType::DenyMatchAll => { !condition_check.all(|b| b) }
            MatchType::DenyMatchOne => { !condition_check.any(|b| b) }
        }
    }
}


pub struct Condition(pub GenFn);

pub struct Modification(pub GenFn);

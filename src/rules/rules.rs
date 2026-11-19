use std::{collections::HashMap, fmt::Display};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::rules::{context::{ConditionCallback, Context}, errors::{CallbackResult, ModError, ModResult}, gen_fn::{GenArg, GenArgs, GenFn, RngSeed, TimeAmount}};




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
pub struct PreparseRules(pub IndexMap<String, PreparseRule>);

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


// Generalizes pattern to GenFn sothat it can be procedurally called later
//
// Structured like
// {
//      match_on,
//      arm_variant_prefix,
//
//      $(
//          variant( $( args )* ) => fn_name ( $( arg_type arg_name ),* )
//      )*
// }
//
//
// When written out, it can look like
// {
//      self,
//      Self,
//
//      WaitPct(pct) => wait_pct(F32 pct, Bool false)
// }
//
// Which will parse to the same as
// GenFn {
//      fn_name: "wait_pct".to_string(),
//      args: GenArgs {
//          arg0: GenArg::F32(pct),
//          arg1: GenArg::Bool(false),
//          ..Default::default()
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
            $fn_name:ident ( $( $arg_type:ident $arg_expr:expr ),* )
        ),*

        $( , ( $( $unreachable_variant:ident ),* ) )?
        // Optional comma delimiter at the end
        $(,)?
    ) => {
        match $match_on {
            $(
                $arm_variant_prefix::$variant $( ( $( $item ),* ) )? => {
                    GenFn {
                        fn_name: stringify!( $fn_name ).to_string(),
                        args: GenArgs(vec![ $( GenArg::$arg_type ( $arg_expr ) ),* ])
                    }
                },
            )*

            $arm_variant_prefix::Fn(gen_fn) => gen_fn,


            $( $( $arm_variant_prefix::$unreachable_variant(..) => unreachable!(), )* )?
        }
    };
}


macro_rules! match_combinators {
    (
        $match_on:expr,
        $arm_variant_prefix:ident,
        $out_type:ident,
        $( $combinator:ident ( $( $cb:ident ),* ) $( => $out_expr:expr )? ),*
        $(,)?
    ) =>{
        match $match_on {
            $(
                $arm_variant_prefix::$combinator ( $($cb ),* ) => {
                    match_combinators! {
                        @match_body
                        $out_type,
                        $combinator ( $( $cb ),* ) $( => $out_expr  )?
                    }
                }
            ),*

            _ => {}
        }
    };

    (
        @match_body
        $out_type:ident,
        $combinator:ident ( $( $cb:ident ),* )
    ) => {
        return $out_type::$combinator ( $( Box::new( $cb.parse() ) ),* )
    };

    (
        @match_body
        $out_type:ident,
        $combinator:ident ( $( $cb:ident ),* ) => $out_expr:expr
    ) => {
        $out_expr
    };
}


#[derive(Debug, Serialize, Deserialize,PartialEq, Clone)]
#[repr(transparent)]
pub struct PreparseConditionVec(pub Vec<PreparseCondition>);

impl PreparseConditionVec {
    pub fn parse(self) -> Vec<Condition> {
        self.0
            .into_iter()
            .map(|c| c.parse())
            .collect()
    }
}

// Define a set of condition types
// The conditions will all just parse to a GenFn when parsed
#[derive(Debug, Serialize, Deserialize,PartialEq, Clone)]
#[non_exhaustive]
pub enum PreparseCondition {
    ActionType(ActionType),
    Probability(f64, RngSeed),
    MatchPid(u32),
    MatchUid(u32),
    MatchGid(u32),
    IsErr,
    ErrMatches(i32),
    DurationOver(TimeAmount),

    Fn(GenFn),


    NOT(Box<PreparseCondition>),
    AND(Box<PreparseCondition>, Box<PreparseCondition>),
    NAND(Box<PreparseCondition>, Box<PreparseCondition>),
    OR(Box<PreparseCondition>, Box<PreparseCondition>),
    NOR(Box<PreparseCondition>, Box<PreparseCondition>),
    XOR(Box<PreparseCondition>, Box<PreparseCondition>),
    IMPLY(Box<PreparseCondition>, Box<PreparseCondition>),
    NIMPLY(Box<PreparseCondition>, Box<PreparseCondition>),
    CIMPLY(Box<PreparseCondition>, Box<PreparseCondition>),
    CNIMPLY(Box<PreparseCondition>, Box<PreparseCondition>),

    ALL(PreparseConditionVec),
    ANY(PreparseConditionVec),
}

impl PreparseCondition {
    pub fn parse(self) -> Condition {
        match_combinators!(
            self.clone(),
            Self,
            Condition,
            NOT(a),
            AND(a, b),
            NAND(a, b),
            OR(a, b),
            NOR(a, b),
            XOR(a, b),
            IMPLY(a, b),
            NIMPLY(a, b),
            CIMPLY(a, b),
            CNIMPLY(a, b),
            ALL(cs) => return Condition::ALL(cs.parse()),
            ANY(cs) => return Condition::ANY(cs.parse())
        );

        let gen_fn = parse_to_gen_fn!(
            self,
            Self,
            ActionType(ty) => match_action(ActionType ty),
            Probability(pct, seed) => probability(F64 pct, RngSeed seed),
            MatchPid(id) => match_pid(U32 id),
            MatchUid(id) => match_uid(U32 id),
            MatchGid(id) => match_gid(U32 id),
            DurationOver(dur) => duration_over(TimeAmount dur),
            IsErr => is_err(),
            ErrMatches(errno) => err_matches(I32 errno),
            (
                NOT,
                AND,
                NAND,
                OR,
                NOR,
                XOR,
                IMPLY,
                NIMPLY,
                CIMPLY,
                CNIMPLY,
                ALL,
                ANY
            )
        );

        Condition::Fn(gen_fn)
    }
}


#[derive(Debug, Serialize, Deserialize)]
pub enum PreparseModification {
    DelayBy(TimeAmount),
    DelayByPct(f32),
    MapErrno(i32, i32),
    FailWithCode(i32),
    CorruptRead(f64, RngSeed),
    LogActionCtx(bool),
    LogActionType,
    LogReply(bool),
    LogCtx(bool),

    Fn(GenFn)
}

impl PreparseModification {
    pub fn parse(self) -> Modification {
        let gen_fn = parse_to_gen_fn!(
            self,
            Self,
            DelayBy(amt) => delay_by(TimeAmount amt),
            DelayByPct(pct) => delay_by_pct(F32 pct),
            MapErrno(from, to) => map_errno(I32 from, I32 to),
            FailWithCode(errno) => fail_with_code(I32 errno),
            CorruptRead(pct, seed) => corrupt_read(F64 pct, RngSeed seed),
            LogActionCtx(pretty) => log_action_ctx(Bool pretty),
            LogActionType => log_action_type(),
            LogReply(pretty) => log_reply(Bool pretty),
            LogCtx(pretty) => log_ctx(Bool pretty),
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
    fn run_conditions<CT>(
        &self,
        cond_callbacks: &IndexMap<String, ConditionCallback<CT>>,
        ctx: &Context<CT>
    ) -> CallbackResult<Vec<bool>> {
        let mut results = Vec::with_capacity(self.conditions.len());
        for c in &self.conditions {
            let res = c.evaluate(cond_callbacks, ctx, &self.name);

            // Collect the result and early return if an error is found
            match res {
                Ok(status) => results.push(status),
                Err(e) => return Err(e)
            }
        }

        Ok(results)
    }


    pub fn conditions_match<CT>(
        &self,
        cond_callbacks: &IndexMap<String, ConditionCallback<CT>>,
        ctx: &Context<CT>
    ) -> bool {
        if self.inactive || self.conditions.is_empty() {
            return false;
        }


        let mut results = match self.run_conditions(cond_callbacks, ctx) {
            Ok(results) => results.into_iter(),
            Err(e) => {
                eprintln!("{e}");
                return false;
            }
        };


        match self.condition_match_type {
            MatchType::MatchAll => { results.all(|b| b) }
            MatchType::MatchOne => { results.any(|b| b) }
            MatchType::DenyMatchAll => { !results.all(|b| b) }
            MatchType::DenyMatchOne => { !results.any(|b| b) }
        }
    }
}


pub enum Condition {
    Fn(GenFn),

    NOT(Box<Condition>),
    AND(Box<Condition>, Box<Condition>),
    NAND(Box<Condition>, Box<Condition>),
    OR(Box<Condition>, Box<Condition>),
    NOR(Box<Condition>, Box<Condition>),
    XOR(Box<Condition>, Box<Condition>),
    IMPLY(Box<Condition>, Box<Condition>),
    NIMPLY(Box<Condition>, Box<Condition>),
    CIMPLY(Box<Condition>, Box<Condition>),
    CNIMPLY(Box<Condition>, Box<Condition>),

    ALL(Vec<Condition>),
    ANY(Vec<Condition>),

}

impl Condition {
    pub fn evaluate<CT>(
        &self,
        cond_callbacks: &IndexMap<String, ConditionCallback<CT>>,
        ctx: &Context<CT>,
        rule_name: &str
    ) -> CallbackResult<bool> {
        let res = match self {
            Self::Fn(f) => {
                let cb = cond_callbacks.get(&f.fn_name)
                    .ok_or(ModError::CallbackNotFound.into_cb_error(
                        rule_name.to_string(),
                        f.fn_name.clone()
                    ))?;

                cb(f.args.clone(), ctx)
                    .map_err(|e| e.into_cb_error(
                        rule_name.to_string(),
                        f.fn_name.clone()
                    ))?
            },


            // !c
            Self::NOT(c) => !c.evaluate(cond_callbacks, ctx, rule_name)?,
            // a && b
            Self::AND(a, b) => {
                a.evaluate(cond_callbacks, ctx, rule_name)?
                && b.evaluate(cond_callbacks, ctx, rule_name)?
            }
            // !( a && b )
            Self::NAND(a, b) => {
                !(
                    a.evaluate(cond_callbacks, ctx, rule_name)?
                    && b.evaluate(cond_callbacks, ctx, rule_name)?
                )
            }
            // a || b
            Self::OR(a, b) => {
                a.evaluate(cond_callbacks, ctx, rule_name)?
                || b.evaluate(cond_callbacks, ctx, rule_name)?
            }
            // !( a || b )
            Self::NOR(a, b) => {
                !(
                    a.evaluate(cond_callbacks, ctx, rule_name)?
                    || b.evaluate(cond_callbacks, ctx, rule_name)?
                )
            }
            // a ^ b
            Self::XOR(a, b) => {
                a.evaluate(cond_callbacks, ctx, rule_name)?
                ^ b.evaluate(cond_callbacks, ctx, rule_name)?
            }
            // !a || b
            Self::IMPLY(a, b) => {
                !a.evaluate(cond_callbacks, ctx, rule_name)?
                || b.evaluate(cond_callbacks, ctx, rule_name)?
            }
            // a && !b
            Self::NIMPLY(a, b) => {
                a.evaluate(cond_callbacks, ctx, rule_name)?
                && !b.evaluate(cond_callbacks, ctx, rule_name)?
            }
            // a || !b
            Self::CIMPLY(a, b) => {
                a.evaluate(cond_callbacks, ctx, rule_name)?
                || !b.evaluate(cond_callbacks, ctx, rule_name)?
            }
            // !a && b
            Self::CNIMPLY(a, b) => {
                !a.evaluate(cond_callbacks, ctx, rule_name)?
                && b.evaluate(cond_callbacks, ctx, rule_name)?
            }


            Self::ALL(cs) => {
                for c in cs {
                    if !c.evaluate(cond_callbacks, ctx, rule_name)? {
                        return Ok(false);
                    }
                }
                true
            }
            Self::ANY(cs) => {
                for c in cs {
                    if c.evaluate(cond_callbacks, ctx, rule_name)? {
                        return Ok(true);
                    }
                }
                false
            }
        };

        Ok(res)
    }


    pub fn validate_callbacks<CT>(
        &self,
        cond_callbacks: &IndexMap<String, ConditionCallback<CT>>
    ) -> Vec<&String> {
        match self {
            Self::Fn(f) => {
                if cond_callbacks.contains_key(&f.fn_name) {
                    Vec::new()
                } else {
                    vec![&f.fn_name]
                }
            }
            Self::NOT(c) => c.validate_callbacks(cond_callbacks),

            Self::AND(a, b)
            | Self::NAND(a, b)
            | Self::OR(a, b)
            | Self::NOR(a, b)
            | Self::XOR(a, b)
            | Self::IMPLY(a, b)
            | Self::NIMPLY(a, b)
            | Self::CIMPLY(a, b)
            | Self::CNIMPLY(a, b) => {
                [
                    a.validate_callbacks(cond_callbacks),
                    b.validate_callbacks(cond_callbacks)
                ].concat()
            }

            Self::ALL(cs)
            | Self::ANY(cs) => {
                let mut all_invalid = Vec::new();
                for c in cs {
                    let invalid = c.validate_callbacks(cond_callbacks);
                    all_invalid.extend_from_slice(&invalid);
                }
                all_invalid
            }
        }

    }
}



pub struct Modification(pub GenFn);

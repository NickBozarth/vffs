use crate::rules::{context::{Context, CtxPostAction}, errors::ModResult, gen_fn::GenArgs, rules::PreparseCondition};

pub fn cond_or(args: GenArgs, ctx: &Context<CtxPostAction>) -> ModResult<bool> {
    let (f1, f2): (PreparseCondition, PreparseCondition) = args.try_into()?;

    let f1 = f1.parse();
    let f2 = f2.parse();




    Ok(false)
}

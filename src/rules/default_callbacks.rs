use std::{cell::OnceCell, fmt::Debug, sync::{Mutex, OnceLock}, time::Duration};

use fuser::Errno;
use rand::{RngExt, SeedableRng, rngs::{SmallRng, StdRng}};

use crate::rules::{context::{Context, CtxPostAction, Reply}, errors::{ModError, ModResult}, gen_fn::{GenArgs, RngSeed, TimeAmount}, rules::ActionType};




pub fn match_action<CT>(args: GenArgs, ctx: &Context<CT>) -> ModResult<bool> {
    let act = ActionType::try_from(args)?;

    Ok(match act {
        ActionType::All => true,
        ActionType::None => false,
        act => act == ctx.action_type()
    })
}

// TODO
// pub fn match_glob<CT>(args: GenArgs, ctx: &Context<CT>) -> ModResult<bool> {
//     Ok(false)
// }

pub fn probability<CT>(args: GenArgs, _ctx: &Context<CT>) -> ModResult<bool> {
    let (pct, seed): (f64, RngSeed) = args.try_into()?;

    if pct < 0.0 || pct > 100.0 {
        return Err(ModError::Other(format!("corrupt_read: pct must be 0..=100, got {pct}").into()));
    }

    static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();

    let rng = RNG.get_or_init(|| {
        Mutex::new(StdRng::seed_from_u64(seed.into()))
    });

    let res = rng.lock()?
        .random_bool(pct / 100.0);

    Ok(res)
}

pub fn match_pid<CT>(args: GenArgs, ctx: &Context<CT>) -> ModResult<bool> {
    Ok(u32::try_from(args)? == ctx.action_ctx().req().pid())
}

pub fn match_uid<CT>(args: GenArgs, ctx: &Context<CT>) -> ModResult<bool> {
    Ok(u32::try_from(args)? == ctx.action_ctx().req().uid())
}

pub fn match_gid<CT>(args: GenArgs, ctx: &Context<CT>) -> ModResult<bool> {
    Ok(u32::try_from(args)? == ctx.action_ctx().req().gid())
}

pub fn is_err(_: GenArgs, ctx: &Context<CtxPostAction>) -> ModResult<bool> {
    match ctx.reply() {
        Reply::Err(_) => Ok(true),
        _ => Ok(false)
    }
}

pub fn err_matches(args: GenArgs, ctx: &Context<CtxPostAction>) -> ModResult<bool> {
    let code = i32::try_from(args)?;

    match ctx.reply() {
        Reply::Err(e) if code == e.code() => Ok(true),
        _ => Ok(false)
    }
}

pub fn duration_over(args: GenArgs, ctx: &Context<CtxPostAction>) -> ModResult<bool> {
    let time_amount = TimeAmount::try_from(args)?;
    let duration = Duration::from(time_amount);

    Ok(ctx.action_duration() > duration)
}

pub fn match_process_name<CT>(args: GenArgs, ctx: &mut Context<CT>) -> ModResult<bool> {
    let name = String::try_from(args)?;
    let pid = ctx.action_ctx().req().pid();
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm"))?;
    Ok(comm.trim_end() == name)
}












pub fn delay_by<CT>(args: GenArgs, _: &mut Context<CT>) -> ModResult<()> {
    let time_amount = TimeAmount::try_from(args)?;
    let duration = Duration::from(time_amount);

    std::thread::sleep(duration);
    Ok(())
}

pub fn delay_by_pct(args: GenArgs, ctx: &mut Context<CtxPostAction>) -> ModResult<()> {
    let pct = f32::try_from(args)?;
    let chg = pct / 100.0;
    let duration = ctx.action_duration()
        .mul_f32(chg);
    std::thread::sleep(duration);

    Ok(())
}

pub fn map_errno(args: GenArgs, ctx: &mut Context<CtxPostAction>) -> ModResult<()> {
    let (from, to): (i32, i32) = args.try_into()?;
    match ctx.reply_mut() {
        Reply::Err(e) if from == e.code() => {
            *e = Errno::from_i32(to);
        }
        _ => {}
    }

    Ok(())
}

pub fn fail_with_code(args: GenArgs, ctx: &mut Context<CtxPostAction>) -> ModResult<()> {
    let code = i32::try_from(args)?;
    ctx.into_err_reply(Errno::from_i32(code));
    Ok(())
}


fn corrupt_bytes<R: RngExt + ?Sized>(buf: &mut[u8], p: f64, rng: &mut R) {
    for byte in buf.iter_mut() {
        let mut mask = 0u8;
        for bit in 0..8 {
            if rng.random_bool(p) {
                mask |= 1 << bit;
            }
        }
        *byte ^= mask;
    }
}

pub fn corrupt_read(args: GenArgs, ctx: &mut Context<CtxPostAction>) -> ModResult<()> {
    let (pct, seed): (f64, RngSeed) = args.try_into()?;

    if pct < 0.0 || pct > 100.0 {
        return Err(ModError::Other(format!("corrupt_read: pct must be 0..=100, got {pct}").into()))
    }

    static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();
    let mut rng = RNG
        .get_or_init(|| {Mutex::new(StdRng::seed_from_u64(seed.into()))})
        .lock()?;

    match ctx.reply_mut() {
        Reply::Read(buf) => corrupt_bytes(buf, pct / 100.0, &mut rng),
        Reply::Err(_) => {},
        _ => return Err(ModError::ActionNotCovered(ctx.action_type()))
    };

    Ok(())
}

pub fn log_action_ctx<CT>(args: GenArgs, ctx: &mut Context<CT>) -> ModResult<()> {
    let pretty = bool::try_from(args)?;
    if pretty {
        println!("ActionCtx: {:#?}", ctx.action_ctx());
    } else {
        println!("ActionCtx: {:?}", ctx.action_ctx());
    }
    Ok(())
}

pub fn log_action_type<CT>(_: GenArgs, ctx: &mut Context<CT>) -> ModResult<()> {
    println!("{}", ctx.action_type());
    Ok(())
}

pub fn log_reply(args: GenArgs, ctx: &mut Context<CtxPostAction>) -> ModResult<()> {
    let pretty = bool::try_from(args)?;
    if pretty {
        println!("Reply: {:#?}", ctx.reply());
    } else {
        println!("Reply: {:?}", ctx.reply());
    }
    Ok(())
}

pub fn log_ctx<CT: Debug>(args: GenArgs, ctx: &mut Context<CT>) -> ModResult<()> {
    let pretty = bool::try_from(args)?;
    if pretty {
        println!("Ctx: {ctx:#?}");
    } else {
        println!("Ctx: {ctx:?}");
    }
    Ok(())
}

pub fn log_read_as_string(_: GenArgs, ctx: &mut Context<CtxPostAction>) -> ModResult<()> {
    let buf = match ctx.reply() {
        Reply::Read(buf) => buf,
        Reply::Err(_) => return Ok(()),
        _ => return Err(ModError::ActionNotCovered(ctx.action_type()))
    };


    println!("Intercepted read");
    match String::from_utf8(buf.to_vec()) {
        Ok(s) => println!("{s}"),
        Err(e) => println!("{e}")
    }

    Ok(())
}

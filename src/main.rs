use std::{sync::mpsc, time::Duration};

use clap::Parser;

use crate::{fs::fault_fs::FaultInjectionFileSystem, rules::{context::{Context, CtxPostAction}, errors::{ModError, ModResult}, gen_fn::GenArgs, rules::ActionType}};

mod fs;
mod args;
mod rules;



fn wait_pct(args: GenArgs, ctx: &mut Context<CtxPostAction>) -> ModResult<()> {
    let (pct, debug) = args.try_into()?;
    let dur = ctx.action_duration()
        .mul_f32(pct)
        .div_f32(100.0);
    std::thread::sleep(dur);

    if debug {
        println!(
            "Extended action {} from {:?} to {:?}",
            ctx.action_type(),
            ctx.action_duration(),
            dur
        );
    }

    Ok(())
}


fn match_action(args: GenArgs, ctx: &Context<CtxPostAction>) -> ModResult<bool> {
    ActionType::try_from(args)
        .map(|act| act == ctx.action_type())
        .map_err(ModError::from)
}


fn main() {
    let args = args::Args::parse()
        .into_validated()
        .expect("Failed to validate command line arguments");


    // _session_guard is an RAII object that unmounts FUSE filesystem on drop
    let _session_guard = FaultInjectionFileSystem::new(
        args.source_dir,
        args.mountpoint,
        args.rules_file,
        Duration::from_secs(1)
    )
        .expect("Failed to create FaultInjectionFileSystem")
        .with_postcondition("match_action", match_action)
        .with_postaction("wait_pct", wait_pct)
        .spawn_mount()
        .expect("Failed to mount FUSE session. Make sure FUSE is installed");


    // Stop thread from completing until ctrlc is enountered
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || { let _ = tx.send(()); })
        .expect("Failed to setup ctrlc handler");
    rx.recv().expect("Failed to receive message from ctrlc handler");
}

use std::{sync::mpsc, time::Duration};

use clap::Parser;

use crate::fs::fs::FaultInjectionFileSystem;

mod fs;
mod args;

fn main() {
    let args = args::Args::parse()
        .into_validated()
        .expect("Failed to validate command line arguments");


    // _session_guard is an RAII object that unmounts FUSE filesystem on drop
    let _session_guard = FaultInjectionFileSystem::new(
        args.source_dir,
        args.mountpoint,
        Duration::from_secs(1)
    )
        .spawn_mount()
        .expect("Failed to mount FUSE session. Make sure FUSE is installed");


    // Stop thread from completing until ctrlc is enountered
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || { let _ = tx.send(()); })
        .expect("Failed to setup ctrlc handler");
    rx.recv().expect("Failed to receive message from ctrlc handler");
}

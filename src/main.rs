use std::time::Duration;

use clap::Parser;

use crate::fs::fs::FaultInjectionFileSystem;

mod fs;
mod args;

fn main() -> std::io::Result<()> {
    let args = args::Args::parse();
    let (src, out, _rules)= args.into_validated().unwrap();

    let _session = FaultInjectionFileSystem::new(
        src,
        out,
        Duration::from_secs(1)
    )
        .spawn_mount()?;

    let main_thread = std::thread::current();
    ctrlc::set_handler(move || main_thread.unpark())
        .expect("Failed to setup ctrlc handler");
    std::thread::park();

    Ok(())
}

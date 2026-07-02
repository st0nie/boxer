#![warn(clippy::all)]

pub mod args;
pub mod build;
pub mod check;
pub mod debian;
pub mod deps;
pub mod exec;
pub mod fmt;
pub mod tools;

pub use fmt::Colors;

use std::io::Write;

use log::LevelFilter;

pub fn setup_logging(verbose: u8, quiet: bool) {
    let level = if quiet {
        LevelFilter::Off
    } else {
        match verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        }
    };

    env_logger::Builder::new()
        .filter_level(level)
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .init();
}

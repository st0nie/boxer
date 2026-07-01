use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "boxer")]
#[command(about = "Debian packaging helper")]
#[command(version)]
pub struct Args {
    #[command(subcommand)]
    pub op: Op,

    /// Increase verbosity
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress non-error output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// When to use color
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorMode,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand)]
pub enum Op {
    /// Generate and install the build-dependency meta package
    Deps {
        /// Package name (defaults to source package from debian/changelog)
        package: Option<String>,
    },
    /// Test that quilt patches apply and unapply cleanly
    Check,
    /// Run check + deps, build the package, archive debs, then clean up
    Build,
}

impl Args {
    pub fn color_enabled(&self) -> bool {
        match self.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => atty::is(atty::Stream::Stdout),
        }
    }
}

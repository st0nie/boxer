use boxer::args::{Args, Op};
use boxer::fmt::error;
use boxer::{Colors, setup_logging};
use clap::Parser;

fn main() {
    let args = Args::parse();
    let c = Colors::new(args.color_enabled());
    setup_logging(args.verbose, args.quiet);

    let result = match &args.op {
        Op::Deps { package } => boxer::deps::run_deps(&c, &args, package.as_deref()),
        Op::Check => boxer::check::run_check(&c, &args),
        Op::Build => boxer::build::run_build(&c, &args),
    };

    if let Err(e) = result {
        error(&c, &format!("{:#}", e));
        std::process::exit(1);
    }
}

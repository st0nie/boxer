use colored::Colorize;

pub struct Colors {
    pub enabled: bool,
}

impl Colors {
    pub fn new(enabled: bool) -> Self {
        colored::control::set_override(enabled);
        Self { enabled }
    }

    pub fn action_prefix(&self) -> String {
        "::".blue().bold().to_string()
    }

    pub fn success_prefix(&self) -> String {
        "::".green().bold().to_string()
    }

    pub fn error_prefix(&self) -> String {
        "::".red().bold().to_string()
    }

    pub fn warning_prefix(&self) -> String {
        "::".yellow().bold().to_string()
    }

    pub fn bold(&self, s: &str) -> String {
        s.bold().to_string()
    }
}

pub fn info(c: &Colors, msg: &str) {
    if c.enabled {
        println!("{} {}", c.action_prefix(), c.bold(msg));
    } else {
        println!(":: {}", msg);
    }
}

pub fn success(c: &Colors, msg: &str) {
    if c.enabled {
        println!("{} {}", c.success_prefix(), msg.green());
    } else {
        println!(":: {}", msg);
    }
}

pub fn warning(c: &Colors, msg: &str) {
    if c.enabled {
        eprintln!("{} {}", c.warning_prefix(), msg.yellow());
    } else {
        eprintln!(":: {}", msg);
    }
}

pub fn error(c: &Colors, msg: &str) {
    if c.enabled {
        eprintln!(
            "{} {}",
            c.error_prefix(),
            format!("{}: {}", env!("CARGO_PKG_NAME"), msg).red().bold()
        );
    } else {
        eprintln!(":: {}: error: {}", env!("CARGO_PKG_NAME"), msg);
    }
}

pub fn detail(c: &Colors, msg: &str) {
    if c.enabled {
        println!("{} {}", "  ->".dimmed(), msg.dimmed());
    } else {
        println!("  -> {}", msg);
    }
}

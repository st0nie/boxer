use ansiterm::Color;
use ansiterm::Style;

/// Colour palette used across boxer, modelled after paru's `Colors`.
///
/// Every style is constructed once at startup; when colour is disabled every
/// field keeps the default (plain) style.
#[derive(Debug, Copy, Clone, Default)]
pub struct Colors {
    pub action: Style,
    pub bold: Style,
    pub error: Style,
    pub warning: Style,
    pub success: Style,
    pub green: Style,
    pub dim: Style,
}

impl Colors {
    pub fn new(enabled: bool) -> Self {
        if enabled {
            Colors {
                action: Style::new().fg(Color::Blue).bold(),
                bold: Style::new().bold(),
                error: Style::new().fg(Color::Red).bold(),
                warning: Style::new().fg(Color::Yellow),
                success: Style::new().fg(Color::Green).bold(),
                green: Style::new().fg(Color::Green),
                dim: Style::new().dimmed(),
            }
        } else {
            Colors::default()
        }
    }
}

pub fn info(c: &Colors, msg: &str) {
    println!("{} {}", c.action.paint("::"), c.bold.paint(msg));
}

pub fn success(c: &Colors, msg: &str) {
    println!("{} {}", c.success.paint("::"), c.green.paint(msg));
}

pub fn warning(c: &Colors, msg: &str) {
    eprintln!("{} {}", c.warning.paint("::"), c.warning.paint(msg));
}

pub fn error(c: &Colors, msg: &str) {
    eprintln!(
        "{} {}",
        c.error.paint("::"),
        c.error
            .paint(format!("{}: {}", env!("CARGO_PKG_NAME"), msg))
    );
}

pub fn detail(c: &Colors, msg: &str) {
    println!("{} {}", c.dim.paint("  ->"), c.dim.paint(msg));
}

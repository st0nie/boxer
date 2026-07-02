use ansiterm::Color;
use ansiterm::Style;

/// Colour palette used across boxer, modelled after paru's `Colors`.
///
/// Every style is constructed once at startup and stored as a field, so the
/// output helpers never branch on `enabled` — when colour is disabled we
/// simply use `Style::new()` (plain) for every field.
#[derive(Debug, Copy, Clone)]
pub struct Colors {
    pub enabled: bool,
    pub action: Style,
    pub bold: Style,
    pub error: Style,
    pub warning: Style,
    pub success: Style,
    pub green: Style,
    pub dim: Style,
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            enabled: false,
            action: Style::new(),
            bold: Style::new(),
            error: Style::new(),
            warning: Style::new(),
            success: Style::new(),
            green: Style::new(),
            dim: Style::new(),
        }
    }
}

impl Colors {
    pub fn new(enabled: bool) -> Self {
        if enabled {
            Colors {
                enabled: true,
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

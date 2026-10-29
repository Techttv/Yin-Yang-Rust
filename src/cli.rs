use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "yinyang", version, about = "Switches KDE Plasma between its light and dark theme")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Apply the light or dark theme to every enabled plugin (what the timer runs)
    Apply(ApplyArgs),
    /// Switch to manual mode and flip between light and dark
    Toggle,
    /// Print the current state, switch times and location as JSON
    Status,
    /// Print every plugin, its availability and its themes as JSON
    Plugins {
        /// Accepted for clarity; the output is always JSON
        #[arg(long)]
        json: bool,
    },
    /// Read or write the configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage the systemd timer
    Timer {
        #[command(subcommand)]
        action: TimerAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print the configuration as JSON
    Show,
    /// Replace the configuration with the JSON on stdin
    Save,
}

#[derive(Debug, Subcommand)]
pub enum TimerAction {
    /// Rewrite and (re)start the timer from the configuration
    Refresh,
}

#[derive(Debug, Args, Default, Clone)]
pub struct ApplyArgs {
    /// Pick the theme from the current time (the default)
    #[arg(long, conflicts_with_all = ["dark", "light"])]
    pub auto: bool,
    /// Apply the dark theme
    #[arg(long, conflicts_with = "light")]
    pub dark: bool,
    /// Apply the light theme
    #[arg(long)]
    pub light: bool,
    /// Apply even if that theme is already the current one, without notifying
    #[arg(long)]
    pub force: bool,
    /// Only print what would be done
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Auto,
    Dark,
    Light,
}

impl ApplyArgs {
    pub fn choice(&self) -> Choice {
        if self.dark {
            Choice::Dark
        } else if self.light {
            Choice::Light
        } else {
            Choice::Auto
        }
    }
}

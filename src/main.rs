use std::io::{stderr, stdin, stdout};
use std::process::ExitCode;

use chrono::Local;
use clap::Parser;
use yinyang::app::{self, Ctx};
use yinyang::cli::{Cli, Command, ConfigAction, TimerAction};
use yinyang::location::IpInfoLocation;
use yinyang::paths::Paths;
use yinyang::plugin::registry;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .init();
    let cli = Cli::parse();

    match run(cli) {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    let paths = Paths::from_env();
    let env = app::system_env(&paths);
    let plugins = registry();
    let exe = std::env::current_exe()?;
    let ctx = Ctx {
        env: &env,
        paths: &paths,
        plugins: &plugins,
        location: &IpInfoLocation,
        exe: &exe,
        now: Local::now().fixed_offset(),
    };

    let (mut out, mut err) = (stdout().lock(), stderr().lock());
    match cli.command {
        Command::Apply(args) => app::apply(&ctx, &args, &mut out, &mut err),
        Command::Toggle => app::toggle(&ctx, &mut out, &mut err),
        Command::Status => app::status(&ctx, &mut out),
        Command::Plugins { .. } => app::plugins_json(&ctx, &mut out),
        Command::Config { action: ConfigAction::Show { defaults } } => app::config_show(&ctx, defaults, &mut out),
        Command::Config { action: ConfigAction::Save } => app::config_save(&ctx, &mut stdin().lock(), &mut err),
        Command::Timer { action: TimerAction::Refresh } => app::timer_refresh(&ctx, &mut err),
    }
}

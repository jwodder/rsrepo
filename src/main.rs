mod changelog;
mod cmd;
mod commands;
mod config;
mod git;
mod github;
mod http_util;
mod project;
mod readme;
mod tmpltr;
mod util;
use crate::commands::Command;
use crate::config::Config;
use anstream::AutoStream;
use anstyle::{AnsiColor, Style};
use anyhow::Context;
use clap::Parser;
use log::{Level, LevelFilter};
use std::env::set_current_dir;
use std::io;
use std::path::PathBuf;

/// Manage Cargo project boilerplate
#[derive(Debug, Eq, Parser, PartialEq)]
#[clap(version)]
struct Arguments {
    /// Change to the given directory before doing anything else
    #[clap(short = 'C', long, value_name = "DIRECTORY")]
    chdir: Option<PathBuf>,

    /// Use the specified configuration file [default: ~/.config/rsrepo.toml]
    #[clap(short = 'c', long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Set logging level
    #[clap(
        short,
        long,
        default_value = "INFO",
        value_name = "OFF|ERROR|WARN|INFO|DEBUG|TRACE"
    )]
    log_level: LevelFilter,

    #[command(subcommand)]
    command: Command,
}

impl Arguments {
    fn run(self) -> anyhow::Result<()> {
        init_logging(self.log_level);
        if let Some(dir) = self.chdir {
            set_current_dir(dir).context("Failed to change directory")?;
        }
        let config = Config::load(self.config.as_deref())?;
        self.command.run(config)
    }
}

fn main() -> anyhow::Result<()> {
    Arguments::parse().run()
}

fn init_logging(log_level: LevelFilter) {
    let stderr: Box<dyn io::Write + Send> = Box::new(AutoStream::auto(io::stderr()));
    fern::Dispatch::new()
        .format(|out, message, record| {
            use AnsiColor::*;
            let style = match record.level() {
                Level::Error => Style::new().fg_color(Some(Red.into())),
                Level::Warn => Style::new().fg_color(Some(Yellow.into())),
                Level::Info => Style::new().bold(),
                Level::Debug => Style::new().fg_color(Some(Cyan.into())),
                Level::Trace => Style::new().fg_color(Some(Green.into())),
            };
            out.finish(format_args!(
                "{}[{:<5}] {}{}",
                style.render(),
                record.level(),
                message,
                style.render_reset(),
            ))
        })
        .level(log_level)
        .chain(stderr)
        .apply()
        .unwrap();
}

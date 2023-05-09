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
use anyhow::Context;
use clap::Parser;
use std::env::set_current_dir;
use std::path::PathBuf;

#[derive(Debug, Eq, Parser, PartialEq)]
#[clap(version)]
struct Arguments {
    #[clap(short = 'C', long)]
    chdir: Option<PathBuf>,

    #[clap(short = 'c', long)]
    config: Option<PathBuf>,

    /// Set logging level
    #[clap(
        short,
        long,
        default_value = "INFO",
        value_name = "OFF|ERROR|WARN|INFO|DEBUG|TRACE"
    )]
    log_level: log::LevelFilter,

    #[command(subcommand)]
    command: Command,
}

impl Arguments {
    fn run(self) -> anyhow::Result<()> {
        fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!("[{:<5}] {}", record.level(), message))
            })
            .level(self.log_level)
            .chain(std::io::stderr())
            .apply()
            .unwrap();
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

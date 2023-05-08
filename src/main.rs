mod cmd;
mod commands;
mod config;
mod project;
mod tmpltr;
use crate::commands::Command;
use crate::config::Config;
use clap::Parser;
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
        let config = Config::load(self.config.as_deref())?;
        self.command.run(config)
    }
}

fn main() -> anyhow::Result<()> {
    Arguments::parse().run()
}

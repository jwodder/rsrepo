mod changelog;
mod cmd;
mod commands;
mod config;
mod git;
mod github;
mod project;
mod provider;
mod readme;
mod tmpltr;
mod util;
use crate::commands::Command;
use crate::provider::Provider;
use anstream::AutoStream;
use anstyle::{AnsiColor, Style};
use anyhow::Context;
use clap::Parser;
use log::{Level, LevelFilter};
use std::env::set_current_dir;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

/// Manage Cargo project boilerplate
#[derive(Debug, Eq, Parser, PartialEq)]
#[command(version = env!("VERSION_WITH_GIT"))]
struct Arguments {
    /// Change to the given directory before doing anything else
    #[arg(short = 'C', long, value_name = "DIRECTORY")]
    chdir: Option<PathBuf>,

    /// Use the specified configuration file [default: ~/.config/rsrepo.toml]
    #[arg(short = 'c', long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Set logging level
    #[arg(
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
        self.command.run(Provider::new(self.config))
    }
}

fn main() -> ExitCode {
    match Arguments::parse().run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if let Some(minigh::RequestError::Status(stat)) = e.downcast_ref() {
                log::error!("{stat:#}");
            } else {
                log::error!("{e:?}");
            }
            ExitCode::FAILURE
        }
    }
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
            ));
        })
        .level(LevelFilter::Info)
        .level_for("minigh", log_level)
        .level_for("rsrepo", log_level)
        .chain(stderr)
        .apply()
        .expect("no other logger should have been previously initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::Bump;
    use clap::error::ErrorKind;
    use clap::CommandFactory;

    #[test]
    fn validate_cli() {
        Arguments::command().debug_assert();
    }

    #[test]
    fn new_implicit_lib() {
        let args = Arguments::try_parse_from(["arg0", "new", "dirpath"]).unwrap();
        let Command::New(new) = args.command else {
            panic!("`new` subcommand did not yield `New` variant");
        };
        assert!(new.lib());
        assert!(!new.bin());
    }

    #[test]
    fn new_explicit_lib() {
        let args = Arguments::try_parse_from(["arg0", "new", "--lib", "dirpath"]).unwrap();
        let Command::New(new) = args.command else {
            panic!("`new` subcommand did not yield `New` variant");
        };
        assert!(new.lib());
        assert!(!new.bin());
    }

    #[test]
    fn new_bin() {
        let args = Arguments::try_parse_from(["arg0", "new", "--bin", "dirpath"]).unwrap();
        let Command::New(new) = args.command else {
            panic!("`new` subcommand did not yield `New` variant");
        };
        assert!(!new.lib());
        assert!(new.bin());
    }

    #[test]
    fn new_bin_lib() {
        let args = Arguments::try_parse_from(["arg0", "new", "--bin", "--lib", "dirpath"]).unwrap();
        let Command::New(new) = args.command else {
            panic!("`new` subcommand did not yield `New` variant");
        };
        assert!(new.lib());
        assert!(new.bin());
    }

    #[test]
    fn release_bump_version() {
        let args = Arguments::try_parse_from(["arg0", "release", "--minor", "v0.2.0"]);
        assert!(args.is_err());
        assert_eq!(args.unwrap_err().kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn release_multi_bump() {
        let args = Arguments::try_parse_from(["arg0", "release", "--minor", "--patch"]);
        assert!(args.is_err());
        assert_eq!(args.unwrap_err().kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn release_major() {
        let args = Arguments::try_parse_from(["arg0", "release", "--major"]).unwrap();
        let Command::Release(rel) = args.command else {
            panic!("`release` subcommand did not yield `Release` variant");
        };
        assert_eq!(rel.bumping.level(), Some(Bump::Major));
    }

    #[test]
    fn release_minor() {
        let args = Arguments::try_parse_from(["arg0", "release", "--minor"]).unwrap();
        let Command::Release(rel) = args.command else {
            panic!("`release` subcommand did not yield `Release` variant");
        };
        assert_eq!(rel.bumping.level(), Some(Bump::Minor));
    }

    #[test]
    fn release_patch() {
        let args = Arguments::try_parse_from(["arg0", "release", "--patch"]).unwrap();
        let Command::Release(rel) = args.command else {
            panic!("`release` subcommand did not yield `Release` variant");
        };
        assert_eq!(rel.bumping.level(), Some(Bump::Patch));
    }
}

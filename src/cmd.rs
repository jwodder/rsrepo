#![allow(dead_code)]
use std::ffi::{OsStr, OsString};
use std::process::Command;
use std::process::{ExitStatus, Stdio};
use thiserror::Error;

pub struct LoggedCommand {
    cmdline: String,
    cmd: Command,
}

impl LoggedCommand {
    pub fn new<I, S>(arg0: &str, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args = args
            .into_iter()
            .map(|s| OsString::from(s.as_ref()))
            .collect::<Vec<_>>();
        let cmdline = format!(
            "{} {}",
            shell_words::quote(arg0),
            shell_words::join(args.iter().map(|s| s.to_string_lossy()))
        );
        let mut cmd = Command::new(arg0);
        cmd.args(args);
        LoggedCommand { cmdline, cmd }
    }

    pub fn status(mut self) -> Result<(), CommandError> {
        log::debug!("Running: {}", self.cmdline);
        match self.cmd.status() {
            Ok(rc) if rc.success() => Ok(()),
            Ok(rc) => Err(CommandError::Exit {
                cmdline: self.cmdline,
                rc,
            }),
            Err(e) => Err(CommandError::Startup {
                cmdline: self.cmdline,
                source: e,
            }),
        }
    }

    pub fn check_output(mut self) -> Result<String, CommandOutputError> {
        log::debug!("Running: {}", self.cmdline);
        match self.cmd.stderr(Stdio::inherit()).output() {
            Ok(output) if output.status.success() => match String::from_utf8(output.stdout) {
                Ok(s) => Ok(s),
                Err(e) => Err(CommandOutputError::Decode {
                    cmdline: self.cmdline,
                    source: e.utf8_error(),
                }),
            },
            Ok(output) => Err(CommandOutputError::Exit {
                cmdline: self.cmdline,
                rc: output.status,
            }),
            Err(e) => Err(CommandOutputError::Startup {
                cmdline: self.cmdline,
                source: e,
            }),
        }
    }
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("failed to run `{cmdline}`: {source}")]
    Startup {
        cmdline: String,
        source: std::io::Error,
    },
    #[error("command `{cmdline}` failed: {rc}")]
    Exit { cmdline: String, rc: ExitStatus },
}

#[derive(Debug, Error)]
pub enum CommandOutputError {
    #[error("failed to run `{cmdline}`: {source}")]
    Startup {
        cmdline: String,
        source: std::io::Error,
    },
    #[error("command `{cmdline}` failed: {rc}")]
    Exit { cmdline: String, rc: ExitStatus },
    #[error("could not decode `{cmdline}` output: {source}")]
    Decode {
        cmdline: String,
        source: std::str::Utf8Error,
    },
}

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::process::{ExitStatus, Stdio};
use thiserror::Error;

pub struct LoggedCommand {
    cmdline: String,
    cmd: Command,
}

impl LoggedCommand {
    pub fn new<S: AsRef<OsStr>>(arg0: S) -> Self {
        let arg0 = arg0.as_ref();
        LoggedCommand {
            cmdline: quote_osstr(arg0),
            cmd: Command::new(arg0),
        }
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        let arg = arg.as_ref();
        self.cmdline.push(' ');
        self.cmdline.push_str(&quote_osstr(arg));
        self.cmd.arg(arg);
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            let arg = arg.as_ref();
            self.cmdline.push(' ');
            self.cmdline.push_str(&quote_osstr(arg));
            self.cmd.arg(arg);
        }
        self
    }

    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        // TODO: Include the dir in the log message?
        self.cmd.current_dir(dir);
        self
    }

    pub fn status(&mut self) -> Result<(), CommandError> {
        log::debug!("Running: {}", self.cmdline);
        match self.cmd.status() {
            Ok(rc) if rc.success() => Ok(()),
            Ok(rc) => Err(CommandError::Exit {
                cmdline: self.cmdline.clone(),
                rc,
            }),
            Err(e) => Err(CommandError::Startup {
                cmdline: self.cmdline.clone(),
                source: e,
            }),
        }
    }

    pub fn check_output(&mut self) -> Result<String, CommandOutputError> {
        log::debug!("Running: {}", self.cmdline);
        match self.cmd.stderr(Stdio::inherit()).output() {
            Ok(output) if output.status.success() => match String::from_utf8(output.stdout) {
                Ok(s) => Ok(s),
                Err(e) => Err(CommandOutputError::Decode {
                    cmdline: self.cmdline.clone(),
                    source: e.utf8_error(),
                }),
            },
            Ok(output) => Err(CommandOutputError::Exit {
                cmdline: self.cmdline.clone(),
                rc: output.status,
            }),
            Err(e) => Err(CommandOutputError::Startup {
                cmdline: self.cmdline.clone(),
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

fn quote_osstr(s: &OsStr) -> String {
    shell_words::quote(&s.to_string_lossy()).to_string()
}

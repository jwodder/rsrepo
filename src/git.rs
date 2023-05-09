#![allow(dead_code)]
use crate::cmd::{CommandError, CommandOutputError, LoggedCommand};
use anyhow::{bail, Context};
use chrono::Datelike;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Git<'a> {
    path: &'a Path,
}

impl<'a> Git<'a> {
    pub fn new(path: &'a Path) -> Self {
        Git { path }
    }

    pub fn run<I, S>(&self, arg0: &str, args: I) -> Result<(), CommandError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        LoggedCommand::new(arg0)
            .args(args)
            .current_dir(self.path)
            .status()
    }

    pub fn read<I, S>(&self, arg0: &str, args: I) -> Result<String, CommandOutputError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        LoggedCommand::new(arg0)
            .args(args)
            .current_dir(self.path)
            .check_output()
            .map(|s| s.trim().to_string())
    }

    // TODO: Make this return an iterator of String lines instead of a Vec
    pub fn readlines<I, S>(&self, arg0: &str, args: I) -> Result<Vec<String>, CommandOutputError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        LoggedCommand::new(arg0)
            .args(args)
            .current_dir(self.path)
            .check_output()
            .map(|s| s.lines().map(|ln| ln.to_string()).collect())
    }

    pub fn remotes(&self) -> Result<Vec<String>, CommandOutputError> {
        self.readlines::<[&str; 0], _>("remote", [])
    }

    pub fn rm_remote(&self, remote: &str) -> Result<(), CommandError> {
        self.run("remote", ["rm", remote])
    }

    pub fn add_remote(&self, remote: &str, url: &str) -> Result<(), CommandError> {
        self.run("remote", ["add", remote, url])
    }

    pub fn commit_years(&self, include_now: bool) -> anyhow::Result<HashSet<i32>> {
        let mut years = self
            .readlines("log", ["--format=%ad", "--date=format:%Y"])?
            .into_iter()
            .map(|s| s.parse())
            .collect::<Result<HashSet<i32>, _>>()
            .context("Error parsing Git commit years")?;
        if include_now {
            years.insert(chrono::Local::now().year());
        }
        Ok(years)
    }

    pub fn default_branch(&self) -> anyhow::Result<String> {
        let branches: HashSet<_> = self
            .readlines("branch", ["--format=%(refname:short)"])?
            .into_iter()
            .collect();
        if let Some(initdefault) = self.get_config("init.defaultBranch", None)? {
            if branches.contains(&initdefault) {
                return Ok(initdefault);
            }
        }
        for guess in ["main", "master", "trunk", "draft"] {
            if branches.contains(guess) {
                return Ok(guess.into());
            }
        }
        bail!("Could not determine default Git branch");
    }

    pub fn latest_tag(&self) -> Result<Option<String>, CommandOutputError> {
        Ok(self
            .readlines("tag", ["-l", "--sort=-creatordate"])?
            .into_iter()
            .next())
    }

    pub fn get_config(
        &self,
        key: &str,
        default: Option<&str>,
    ) -> Result<Option<String>, CommandOutputError> {
        let mut args = Vec::from(["--get"]);
        if let Some(s) = default {
            args.push("--default");
            args.push(s);
        }
        args.push("--");
        args.push(key);
        match self.read("config", args) {
            Ok(s) => Ok(Some(s)),
            Err(CommandOutputError::Exit { rc, .. }) if rc.code() == Some(1) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

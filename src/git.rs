use crate::cmd::{CommandError, CommandOutputError, LoggedCommand};
use crate::util::StringLines;
use anyhow::Context;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Git<'a> {
    path: &'a Path,
}

impl<'a> Git<'a> {
    pub fn new(path: &'a Path) -> Self {
        Git { path }
    }

    pub fn command(&self) -> LoggedCommand {
        let mut cmd = LoggedCommand::new("git");
        cmd.current_dir(self.path);
        cmd
    }

    pub fn run<I, S>(&self, cmd: &str, args: I) -> Result<(), CommandError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command().arg(cmd).args(args).status()
    }

    pub fn read<I, S>(&self, cmd: &str, args: I) -> Result<String, CommandOutputError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command()
            .arg(cmd)
            .args(args)
            .check_output()
            .map(|s| s.trim().to_string())
    }

    pub fn readlines<I, S>(&self, cmd: &str, args: I) -> Result<StringLines, CommandOutputError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command()
            .arg(cmd)
            .args(args)
            .check_output()
            .map(StringLines::new)
    }

    pub fn remotes(&self) -> Result<HashSet<String>, CommandOutputError> {
        self.readlines::<[&str; 0], _>("remote", [])
            .map(Iterator::collect)
    }

    pub fn rm_remote(&self, remote: &str) -> Result<(), CommandError> {
        self.run("remote", ["rm", remote])
    }

    pub fn add_remote(&self, remote: &str, url: &str) -> Result<(), CommandError> {
        self.run("remote", ["add", remote, url])
    }

    pub fn commit_years(&self) -> anyhow::Result<HashSet<i32>> {
        self.readlines("log", ["--format=%ad", "--date=format:%Y"])?
            .map(|s| s.parse())
            .collect::<Result<HashSet<i32>, _>>()
            .context("Error parsing Git commit years")
    }

    pub fn latest_tag(&self) -> Result<Option<String>, CommandOutputError> {
        Ok(self.readlines("tag", ["-l", "--sort=-creatordate"])?.next())
    }

    pub fn current_branch(&self) -> Result<Option<String>, CommandOutputError> {
        match self.read("symbolic-ref", ["--short", "-q", "HEAD"]) {
            Ok(branch) => Ok(Some(branch)),
            Err(CommandOutputError::Exit { rc, .. }) if rc.code() == Some(1) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn untracked_files(&self) -> Result<Vec<PathBuf>, CommandOutputError> {
        let s = self.read(
            "ls-files",
            ["-z", "-o", "--exclude-standard", "--directory"],
        )?;
        Ok(s.split_terminator('\0')
            .map(PathBuf::from)
            .collect::<Vec<_>>())
    }

    pub fn tag_exists(&self, tag: &str) -> Result<bool, CommandError> {
        match self
            .command()
            .arg("show-ref")
            .arg("--verify")
            .arg("--quiet")
            .arg(format!("refs/tags/{tag}"))
            .status()
        {
            Ok(()) => Ok(true),
            Err(CommandError::Exit { rc, .. }) if rc.code() == Some(1) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn toplevel(&self) -> Result<PathBuf, CommandOutputError> {
        // Don't use `Git::read()`, as that can strip off too much if the
        // directory name ends in whitespace.
        let mut s = self
            .command()
            .arg("rev-parse")
            .arg("--show-toplevel")
            .check_output()?;
        if s.ends_with('\n') {
            s.pop();
            #[cfg(windows)]
            if s.ends_with('\r') {
                // Although Git on Windows (at least under GitHub Actions)
                // seems to use LF as the newline sequence in its output, we
                // should still take care to strip final CR on Windows if it
                // ever shows up.  As Windows doesn't allow CR in file names, a
                // CR here will always be part of a line ending.
                s.pop();
            }
        }
        Ok(PathBuf::from(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use tempfile::tempdir;

    #[test]
    fn toplevel() {
        // Assumes Git is installed and the package code is located in the root
        // of a Git repository
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let srcdir = manifest_dir.join("src");
        let git = Git::new(&srcdir);
        assert_eq!(git.toplevel().unwrap(), manifest_dir);
    }

    // These are illegal filenames on Windows.
    #[cfg(not(windows))]
    #[rstest]
    #[case("foobar\r")]
    #[case("foobar\n")]
    #[case("foobar ")]
    fn toplevel_basename_endswith_space(#[case] fname: &str) {
        let tmp_path = tempdir().unwrap();
        let repo = tmp_path.path().join(fname);
        LoggedCommand::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .arg("--")
            .arg(&repo)
            .status()
            .unwrap();
        let git = Git::new(&repo);
        assert_eq!(git.toplevel().unwrap(), repo.canonicalize().unwrap());
    }
}

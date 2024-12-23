use crate::cmd::{CommandError, CommandOutputError, LoggedCommand};
use crate::util::StringLines;
use anyhow::Context;
use semver::Version;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Git<'a> {
    path: &'a Path,
}

impl<'a> Git<'a> {
    pub(crate) fn new(path: &'a Path) -> Self {
        Git { path }
    }

    pub(crate) fn command(&self) -> LoggedCommand {
        let mut cmd = LoggedCommand::new("git");
        cmd.current_dir(self.path);
        cmd
    }

    pub(crate) fn run<I, S>(&self, cmd: &str, args: I) -> Result<(), CommandError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command().arg(cmd).args(args).status()
    }

    pub(crate) fn read<I, S>(&self, cmd: &str, args: I) -> Result<String, CommandOutputError>
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

    pub(crate) fn readlines<I, S>(
        &self,
        cmd: &str,
        args: I,
    ) -> Result<StringLines, CommandOutputError>
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

    pub(crate) fn remotes(&self) -> Result<HashSet<String>, CommandOutputError> {
        self.readlines::<[&str; 0], _>("remote", [])
            .map(Iterator::collect)
    }

    pub(crate) fn rm_remote(&self, remote: &str) -> Result<(), CommandError> {
        self.run("remote", ["rm", remote])
    }

    pub(crate) fn add_remote(&self, remote: &str, url: &str) -> Result<(), CommandError> {
        self.run("remote", ["add", remote, url])
    }

    pub(crate) fn commit_years(&self) -> anyhow::Result<HashSet<i32>> {
        self.readlines("log", ["--format=%ad", "--date=format:%Y"])?
            .map(|s| s.parse())
            .collect::<Result<HashSet<i32>, _>>()
            .context("Error parsing Git commit years")
    }

    pub(crate) fn latest_tag(
        &self,
        prefix: Option<&str>,
    ) -> Result<Option<String>, CommandOutputError> {
        let mut args = vec![String::from("-l"), String::from("--sort=-creatordate")];
        if let Some(pre) = prefix {
            args.push(format!("{pre}*"));
        }
        Ok(self.readlines("tag", args)?.next())
    }

    pub(crate) fn latest_tag_version(
        &self,
        prefix: Option<&str>,
    ) -> anyhow::Result<Option<Version>> {
        if let Some(tag) = self.latest_tag(prefix)? {
            let tagv = match prefix {
                Some(pre) => tag.strip_prefix(pre).unwrap_or(&*tag),
                None => &*tag,
            };
            tagv.strip_prefix('v')
                .unwrap_or(tagv)
                .parse::<Version>()
                .with_context(|| format!("Failed to parse latest Git tag {tag:?} as a version"))
                .map(Some)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn current_branch(&self) -> Result<Option<String>, CommandOutputError> {
        match self.read("symbolic-ref", ["--short", "-q", "HEAD"]) {
            Ok(branch) => Ok(Some(branch)),
            Err(CommandOutputError::Exit { rc, .. }) if rc.code() == Some(1) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(crate) fn untracked_files(&self) -> Result<Vec<PathBuf>, CommandOutputError> {
        let s = self.read(
            "ls-files",
            ["-z", "-o", "--exclude-standard", "--directory"],
        )?;
        Ok(s.split_terminator('\0')
            .map(PathBuf::from)
            .collect::<Vec<_>>())
    }

    pub(crate) fn tag_exists(&self, tag: &str) -> Result<bool, CommandError> {
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

    pub(crate) fn toplevel(&self) -> Result<PathBuf, CommandOutputError> {
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

    // Returns None if the default branch could not be determined
    pub(crate) fn default_branch(&self) -> Result<Option<&'static str>, CommandOutputError> {
        let branches = self
            .readlines("branch", ["--format=%(refname:short)"])?
            .collect::<HashSet<_>>();
        for guess in ["main", "master"] {
            if branches.contains(guess) {
                return Ok(Some(guess));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toplevel() {
        // Assumes Git is installed and the package code is located in the root
        // of a Git repository
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let srcdir = manifest_dir.join("src");
        let git = Git::new(&srcdir);
        assert_eq!(git.toplevel().unwrap(), manifest_dir);
    }

    #[cfg(not(windows))]
    mod not_windows {
        use super::*;
        use rstest::rstest;
        use tempfile::tempdir;

        // These are illegal filenames on Windows.
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
}

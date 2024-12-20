mod util;
use crate::util::{copytree, unzip, CmpDirtrees};
use assert_cmd::Command;
use rstest::rstest;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

pub static DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

#[test]
fn new_implicit_lib() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--copyright-year=2525")
        .arg("--msrv=1.69")
        .arg(&repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "master")
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(Path::new(DATA_DIR).join("new").join("lib"), repo)
        .exclude([".git"])
        .assert_eq();
}

#[test]
fn new_explicit_lib() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--lib")
        .arg("--copyright-year=2525")
        .arg("--msrv=1.69")
        .arg(&repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "master")
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(Path::new(DATA_DIR).join("new").join("lib"), repo)
        .exclude([".git"])
        .assert_eq();
}

#[test]
fn new_bin() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--bin")
        .arg("--copyright-year=2525")
        .arg("--msrv=1.69")
        .arg(&repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "master")
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(Path::new(DATA_DIR).join("new").join("bin"), repo)
        .exclude([".git"])
        .assert_eq();
}

#[test]
fn new_bin_lib() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--bin")
        .arg("--lib")
        .arg("--copyright-year=2525")
        .arg("--msrv=1.69")
        .arg(&repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "master")
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(Path::new(DATA_DIR).join("new").join("bin-lib"), repo)
        .exclude([".git"])
        .assert_eq();
}

#[test]
fn new_custom_project_name() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--lib")
        .arg("--copyright-year=2525")
        .arg("--msrv=1.69")
        .arg("--name=quux")
        .arg(&repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "master")
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(
        Path::new(DATA_DIR).join("new").join("custom-project-name"),
        repo,
    )
    .exclude([".git"])
    .assert_eq();
}

#[test]
fn new_custom_repo_name() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--lib")
        .arg("--copyright-year=2525")
        .arg("--msrv=1.69")
        .arg("--repo-name=quux")
        .arg(&repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "master")
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(
        Path::new(DATA_DIR).join("new").join("custom-repo-name"),
        repo,
    )
    .exclude([".git"])
    .assert_eq();
}

#[test]
fn new_custom_project_repo_name() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--lib")
        .arg("--copyright-year=2525")
        .arg("--msrv=1.69")
        .arg("--name=gnusto")
        .arg("--repo-name=cleesh")
        .arg(&repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "master")
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(
        Path::new(DATA_DIR)
            .join("new")
            .join("custom-project-repo-name"),
        repo,
    )
    .exclude([".git"])
    .assert_eq();
}

#[test]
fn new_description() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--copyright-year=2525")
        .arg("--msrv=1.69")
        .arg("-d")
        .arg("A library for foo'ing bars")
        .arg(&repo)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "init.defaultBranch")
        .env("GIT_CONFIG_VALUE_0", "master")
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(Path::new(DATA_DIR).join("new").join("description"), repo)
        .exclude([".git"])
        .assert_eq();
}

#[rstest]
#[case("plain")]
#[case("no-entry")]
#[case("big-chlog")]
#[case("newly-set")]
fn set_msrv(#[case] case: &str) {
    let tmp_path = tempdir().unwrap();
    copytree(
        Path::new(DATA_DIR)
            .join("set-msrv")
            .join(format!("{case}-before")),
        tmp_path.path(),
    )
    .unwrap();
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("set-msrv")
        .arg("1.66")
        .current_dir(tmp_path.path())
        .assert()
        .success();
    CmpDirtrees::new(
        Path::new(DATA_DIR)
            .join("set-msrv")
            .join(format!("{case}-after")),
        tmp_path.path(),
    )
    .assert_eq();
}

#[rstest]
#[case("package", None, false, "root.json")]
#[case("package", None, true, "root-workspace.json")]
#[case("package-repo", None, false, "root.json")]
#[case("package-repo", None, true, "root-workspace.json")]
#[case("workspace", None, false, "root.json")]
#[case("workspace", None, true, "root-workspace.json")]
#[case("workspace", Some("cli"), false, "bin-crate.json")]
#[case("workspace", Some("cli"), true, "bin-crate-workspace.json")]
#[case("workspace-repo", None, false, "root.json")]
#[case("workspace-repo", None, true, "root-workspace.json")]
#[case("workspace-repo", Some("cli"), false, "bin-crate.json")]
#[case("workspace-repo", Some("cli"), true, "bin-crate-workspace.json")]
#[case("virtual", None, false, "root.json")]
#[case("virtual", None, true, "root-workspace.json")]
#[case("virtual", Some("crates/fibonacci"), false, "lib-crate.json")]
#[case("virtual", Some("crates/fibonacci"), true, "lib-crate-workspace.json")]
#[case("virtual", Some("crates/cli"), false, "bin-crate.json")]
#[case("virtual", Some("crates/cli"), true, "bin-crate-workspace.json")]
#[case("virtual-repo", None, false, "root.json")]
#[case("virtual-repo", None, true, "root-workspace.json")]
#[case("virtual-repo", Some("crates/fibonacci"), false, "lib-crate.json")]
#[case(
    "virtual-repo",
    Some("crates/fibonacci"),
    true,
    "lib-crate-workspace.json"
)]
#[case("virtual-repo", Some("crates/cli"), false, "bin-crate.json")]
#[case("virtual-repo", Some("crates/cli"), true, "bin-crate-workspace.json")]
fn inspect(
    #[case] project: &str,
    #[case] subdir: Option<&str>,
    #[case] workspace: bool,
    #[case] jsonfile: &str,
) {
    let tmp_path = tempdir().unwrap();
    let projdir = Path::new(DATA_DIR).join("inspect").join(project);
    unzip(projdir.join("project.zip"), tmp_path.path()).unwrap();
    let expected = fs_err::read_to_string(projdir.join(jsonfile))
        .unwrap()
        .replace(
            "{root}",
            tmp_path.path().canonicalize().unwrap().to_str().unwrap(),
        );
    let mut cwd = PathBuf::from(tmp_path.path());
    if let Some(p) = subdir {
        cwd.push(p);
    }
    let mut cmd = Command::cargo_bin("rsrepo").unwrap();
    cmd.arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("inspect");
    if workspace {
        cmd.arg("--workspace");
    }
    cmd.current_dir(cwd);
    cmd.assert().stdout(expected);
}

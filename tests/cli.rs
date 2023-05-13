mod util;
use crate::util::CmpDirtrees;
use assert_cmd::Command;
use std::path::Path;
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
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(&Path::new(DATA_DIR).join("new").join("lib"), &repo)
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
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(&Path::new(DATA_DIR).join("new").join("lib"), &repo)
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
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(&Path::new(DATA_DIR).join("new").join("bin"), &repo)
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
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(&Path::new(DATA_DIR).join("new").join("bin-lib"), &repo)
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
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(
        &Path::new(DATA_DIR).join("new").join("custom-project-name"),
        &repo,
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
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(
        &Path::new(DATA_DIR).join("new").join("custom-repo-name"),
        &repo,
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
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(
        &Path::new(DATA_DIR)
            .join("new")
            .join("custom-project-repo-name"),
        &repo,
    )
    .exclude([".git"])
    .assert_eq();
}

#[test]
fn description() {
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
        .assert()
        .success();
    Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(&repo)
        .assert()
        .success();
    CmpDirtrees::new(&Path::new(DATA_DIR).join("new").join("description"), &repo)
        .exclude([".git"])
        .assert_eq();
}

mod util;
use crate::util::CmpDirtrees;
use assert_cmd::Command;
use std::path::Path;
use tempfile::tempdir;

pub static DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

#[test]
fn new_implicit_bin() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    Command::cargo_bin("rsrepo")
        .unwrap()
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("new")
        .arg("--copyright-year=2525")
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

mod util;
use crate::util::{CmpDirtrees, opt_subdir, unzip};
use assert_cmd::{Command, cargo::cargo_bin_cmd};
use cfg_if::cfg_if;
use rstest::rstest;
use std::path::Path;
use tempfile::tempdir;

pub static DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data");

#[test]
fn new_implicit_lib() {
    let tmp_path = tempdir().unwrap();
    let repo = tmp_path.path().join("foobar");
    cargo_bin_cmd!("rsrepo")
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
    cargo_bin_cmd!("rsrepo")
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
    cargo_bin_cmd!("rsrepo")
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
    cargo_bin_cmd!("rsrepo")
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
    cargo_bin_cmd!("rsrepo")
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
    cargo_bin_cmd!("rsrepo")
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
    cargo_bin_cmd!("rsrepo")
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
    cargo_bin_cmd!("rsrepo")
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
#[case("plain", Vec::new(), None)]
#[case("no-entry", Vec::new(), None)]
#[case("big-chlog", Vec::new(), None)]
#[case("newly-set", Vec::new(), None)]
#[case("workspace", vec!["--workspace"], None)]
#[case("wrkspc-locate", Vec::new(), Some("crates/api-test"))]
#[case("wrkspc-locate", vec!["-p", "sudoku-api-test"], Some("crates/sudoku"))]
fn set_msrv(#[case] case: &str, #[case] opts: Vec<&str>, #[case] subdir: Option<&str>) {
    let tmp_path = tempdir().unwrap();
    let workdir = tmp_path.path().join("work");
    let gooddir = tmp_path.path().join("good");
    unzip(
        Path::new(DATA_DIR)
            .join("set-msrv")
            .join(case)
            .join("before.zip"),
        &workdir,
    )
    .unwrap();
    cargo_bin_cmd!("rsrepo")
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("set-msrv")
        .args(opts)
        .arg("1.75")
        .current_dir(opt_subdir(&workdir, subdir))
        .assert()
        .success();
    unzip(
        Path::new(DATA_DIR)
            .join("set-msrv")
            .join(case)
            .join("after.zip"),
        &gooddir,
    )
    .unwrap();
    CmpDirtrees::new(gooddir, workdir).assert_eq();
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

    cfg_if! {
        if #[cfg(target_os = "macos")] {
            let rootpath = tmp_path.path().canonicalize().unwrap();
        } else {
            let rootpath = tmp_path.path();
        }
    }
    let root = rootpath.to_str().unwrap();

    let mut expected = fs_err::read_to_string(projdir.join(jsonfile)).unwrap();
    cfg_if! {
        if #[cfg(windows)] {
            let root = root.replace('\\', "\\\\");
            expected = {
                let mut s = String::new();
                for line in expected.lines() {
                    if line.contains("{root}") {
                        s.push_str(&line.replace("{root}", &root).replace('/', "\\\\"));
                    } else {
                        s.push_str(line);
                    }
                    s.push('\n');
                }
                s
            }
        } else {
            expected = expected.replace("{root}", root);
        }
    }

    cargo_bin_cmd!("rsrepo")
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("inspect")
        .args(workspace.then_some("--workspace"))
        .current_dir(opt_subdir(tmp_path.path(), subdir))
        .assert()
        .success()
        .stdout(expected);
}

#[rstest]
#[case("lib.zip", vec!["--no-codecov-token"], "lib.json")]
#[case("lib.zip", vec!["--codecov-token=hunter2", "fibseqlib"], "lib-cli-name.json")]
#[case("bin.zip", vec!["--no-codecov-token"], "bin.json")]
#[case("bin.zip", vec!["--no-codecov-token", "fibseqcli"], "bin-cli-name.json")]
#[case("workspace.zip", vec!["--no-codecov-token"], "workspace.json")]
#[case("workspace.zip", vec!["--no-codecov-token", "fibstuff"], "workspace-cli-name.json")]
#[case("virtual.zip", vec!["--no-codecov-token"], "virtual.json")]
#[case("virtual.zip", vec!["--no-codecov-token", "fib"], "virtual-cli-name.json")]
fn mkgithub(#[case] zipfile: &str, #[case] args: Vec<&str>, #[case] jsonfile: &str) {
    let tmp_path = tempdir().unwrap();
    unzip(
        Path::new(DATA_DIR).join("mkgithub").join(zipfile),
        tmp_path.path(),
    )
    .unwrap();
    Command::new("git")
        .arg("init")
        .arg("-b")
        .arg("main")
        .current_dir(tmp_path.path())
        .assert()
        .success();
    Command::new("git")
        .arg("add")
        .arg(".")
        .current_dir(tmp_path.path())
        .assert()
        .success();
    Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg("Ensure that default branch exists")
        .current_dir(tmp_path.path())
        .assert()
        .success();
    let expected =
        fs_err::read_to_string(Path::new(DATA_DIR).join("mkgithub").join(jsonfile)).unwrap();
    cargo_bin_cmd!("rsrepo")
        .arg("--log-level=TRACE")
        .arg("--config")
        .arg(Path::new(DATA_DIR).join("config.toml"))
        .arg("mkgithub")
        .arg("--plan-only")
        .args(args)
        .current_dir(tmp_path.path())
        .assert()
        .success()
        .stdout(expected);
}

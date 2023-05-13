[![Project Status: Concept – Minimal or no implementation has been done yet, or the repository is only intended to be a limited example, demo, or proof-of-concept.](https://www.repostatus.org/badges/latest/concept.svg)](https://www.repostatus.org/#concept)
[![CI Status](https://github.com/jwodder/rsrepo/actions/workflows/test.yml/badge.svg)](https://github.com/jwodder/rsrepo/actions/workflows/test.yml)
[![codecov.io](https://codecov.io/gh/jwodder/rsrepo/branch/master/graph/badge.svg)](https://codecov.io/gh/jwodder/rsrepo)
[![MIT License](https://img.shields.io/github/license/jwodder/rsrepo.svg)](https://opensource.org/licenses/MIT)

[GitHub](https://github.com/jwodder/rsrepo) | [Issues](https://github.com/jwodder/rsrepo/issues)

`rsrepo` is my personal command-line program for managing my Rust project
repositories, including generating packaging boilerplate and performing
releases.  While it may in theory be suitable for general use, I make no
guarantees, nor do I intend to release it for general consumption.  Use at your
own risk.

Usage
=====

    rsrepo [<global options>] <subcommand> ...

All `rsrepo` subcommands other than `rsrepo new` must be run inside a Cargo
package directory & Git repository (after processing the `--chdir` option, if
given).  Cargo workspaces are currently not supported.

Certain commands automatically edit projects' `README.md` and/or `CHANGELOG.md`
files; these files are expected to adhere to specific formats, documented in
[`doc/readme-format.md`][readme] and [`doc/changelog-format.md`][changelog],
respectively.

[readme]: https://github.com/jwodder/rsrepo/blob/master/doc/readme-format.md
[changelog]: https://github.com/jwodder/rsrepo/blob/master/doc/changelog-format.md

Global Options
--------------

- `-c <file>`, `--config <file>` — Read configuration from the given file; by
  default, configuration is read from `~/.config/rsrepo.toml`.  See
  "Configuration File" below for more information.

- `-C <dir>`, `--chdir <dir>` — Change to the given directory before taking any
  further actions

- `-l <level>`, `--log-level <level>` — Set the logging level to the given
  value.  The possible options are "`OFF`", "`ERROR`", "`WARN`", "`INFO`",
  "`DEBUG`", and "`TRACE`", all case-insensitive.  The default value is
  "`INFO`".

External Dependencies
---------------------

Various `rsrepo` subcommands make use of the following external programs or
configurations:

- Git — required by the `new`, `mkgithub`, and `release` subcommands

- [pre-commit](https://github.com/pre-commit/pre-commit) — optional dependency
  of the `new` subcommand; a warning will be emitted if not installed

- [Cargo](https://docs.rs/cargo) — required by the `release` subcommand

    - If using `rsrepo release` to publish a package, a Cargo registry API
      token must have been saved with Cargo.

- A GitHub API token must have been saved with
  [`gh`](https://github.com/cli/cli) in order for the `mkgithub` and `release`
  subcommands to work

- The `release` subcommand creates a signed Git tag, and so `gpg` (or another
  program specified via Git's `gpg.program` config variable) must be installed
  and usable

Configuration File
------------------

The configuration file (located at `~/.config/rsrepo.toml` by default) is a
[TOML](https://toml.io) file with the following keys:

- `author` — The author name to use when `rsrepo new` generates `Cargo.toml`
  and `LICENSE` files

- `author-email` — The author e-mail to use when `rsrepo new` generates a
  `Cargo.toml` file; this may contain a placeholder of the form `{package}`,
  which will be replaced with the name of the package being initialized.

- `github-user` — The GitHub username to use when `rsrepo new` generates
  `Cargo.toml` and `README.md` files

`rsrepo new`
------------

    rsrepo [<global options>] new [<options>] <directory>

Create a new Git repository at the given directory path and populate it with
basic files for a new Cargo project.

The following files are created in the directory:

- `.github/dependabot.yml`
- `.github/workflows/test.yml`
- `.gitignore`
- `.pre-commit-config.yaml` (`pre-commit install` is also run if `pre-commit`
  is installed)
- `Cargo.toml`
- `LICENSE`
- `README.md`
- `src/lib.rs` (if creating a library crate)
- `src/main.rs` (if create a binary crate)

### Options

- `--bin` — Create a binary crate

- `--copyright-year <string>` — Specify the copyright year(s) to put in the
  `LICENSE` file; defaults to the current year

- `-d <text>`, `--description <text>` — Specify a description for the new
  package; if not specified, the `description` key in `Cargo.toml` will be
  commented out.

- `--lib` — Create a library crate.  This is the default if neither `--bin` nor
  `--lib` is given.

- `--msrv VERSION` — Specify the minimum supported Rust version to declare for
  the new package; defaults to the latest stable rustc version.  The version
  must be given as either two or three dot-separated integers.

- `--name <name>` — Specify the package name to declare in the `Cargo.toml`
  file; defaults to the basename of the directory

- `--repo-name <name>` — Specify the GitHub repository name (sans owner) to use
  in URLs in generated files; defaults to the package name

`rsrepo mkgithub`
-----------------

    rsrepo [<global options>] mkgithub [<options>] [<name>]

Create a new GitHub repository for the project, set the local repository's
`origin` remote to point to the GitHub repository, and push all branches & tags
to the remote.

The project's description (if any) is used as the repository description.  The
project's keywords are used as the repository's topics, along with the "`rust`"
topic; in addition, if the project's `README.md` file has a "WIP"
[repostatus.org](https://www.repostatus.org) badge, the "`work-in-progress`"
topic is added.  The custom labels used by the `dependabot.yml` file generated
by `rsrepo new` are created in the repository as well.

The bare name of the repository to create (e.g. `hello-world`, not
`octocat/hello-world` or `https://github.com/octocat/hello-world`) can
optionally be specified as an argument on the command line; if not given, the
repository name is determined by parsing the `repository` key in the
`Cargo.toml` file, falling back to the package name if there is no such key.

The GitHub repository will be created under the account for the user associated
with the GitHub API token stored by `gh`.  Creating a repository under an
organization is currently not supported.

### Options

- `-P`, `--private` — Make the new repository private

`rsrepo release`
----------------

    rsrepo [<global-options>] release [<options>] [<version>]

Create & publish a new release for a project.

The version of the release can be either specified explicitly on the command
line or (if one of `--major`, `--minor`, or `--patch` is given) calculated by
bumping the version extracted from the most recently-created Git tag; in the
latter case, the metadata identifier (if any) is discarded from the version,
and it is an error if the bumped version is a prerelease.  If no version or
bump option is given on the command line, the version declared in the
`Cargo.toml` file is used after stripping any prerelease & metadata components;
it is an error if this version is less than or equal to the version of the
latest Git tag.  Except when an explicit version argument is given, it is an
error for the latest Git tag to not be a Cargo semver version with optional
leading `v`.

This command performs the following operations in order:

- The version key in `Cargo.toml` is set to the release version.  If the
  project contains a binary crate, the version in `Cargo.lock` is set as well.

- If `CHANGELOG.md` exists, the header for the topmost section is edited to
  contain the release version and the current date.  It is an error if the
  topmost section header already contains a date.

- If the release version is not a prerelease and the `README.md` has a
  repostatus.org "WIP" badge, the badge is changed to "Active."

- If `publish` in `Cargo.toml` is not `false`, links to `crates.io` and (if the
  project contains a library crate) `docs.rs` are added to `README.md`'s header
  links.

- The copyright years in the first copyright line in `LICENSE` are updated to
  include all years in which commits were made to the repository, including the
  current year.  A line is treated as a copyright line if it is of the form
  "Copyright (c) YEARS AUTHOR", where the "YEARS" component consists of year
  numbers, dashes, commas, and/or spaces.  It is an error if `LICENSE` does not
  contain a copyright line.

- All changes made to tracked files in the repository are committed; the text
  of the most recent `CHANGELOG.md` section is included in the commit message
  template.

    - The release can be cancelled at this point by either leaving the commit
      message unchanged or by deleting the entire commit message.

- The commit is tagged (as `v{version}`) and signed.

- If `publish` in `Cargo.toml` is not `false`:

    - Any untracked files in the repository are moved to
      `$GIT_WORK_TREE.stash/`, where `$GIT_WORK_TREE` is the path to the
      toplevel of the Git repository's working tree.  (Note that ".stash" is
      here an extension appended to the basename of the directory, not a
      subdirectory of `$GIT_WORK_TREE`.)

    - `cargo publish` is run.

    - Any files in `$GIT_WORK_TREE.stash/` are moved back to the Git
      repository, and the stash directory is deleted.

- The commit & tag are pushed; it is assumed that they are pushed to GitHub.

- A GitHub release pointing to the new tag is created in the project's GitHub
  repository.  The name of the release is the first line of the tagged commit's
  commit message, and its body is the rest of the commit message.  If the new
  version is a prerelease, the GitHub release is marked as a prerelease as
  well.

    - The project's GitHub repository is identified by parsing the URL for the
      local Git repository's `origin` remote.

- If the repostatus.org badge in `README.md` was set to "Active" earlier, then
  any "`work-in-progress`" topic is removed from the GitHub repository's
  topics, and if `publish` in `Cargo.toml` is additionally not `false`, the
  "`available-on-crates-io`" topic is added.

- Development on the next version is started:

    - The version key in `Cargo.toml` is set to the next minor version after
      the just-released version, plus a "-dev" prerelease segment.

    - If a `CHANGELOG.md` file does not exist, one is created with a section
      for the release that was just made (with text set to "Initial release").
      Either way, an empty section for the next minor version is added to the
      top of the changelog.  In addition, a link to `CHANGELOG.md` on GitHub is
      added to `README.md`'s header links if not already present.

### Options

- `--major` — Set the release's version to the next major version after the
  most recent Git tag

- `--minor` — Set the release's version to the next minor version after the
  most recent Git tag

- `--patch` — Set the release's version to the next micro version after the
  most recent Git tag

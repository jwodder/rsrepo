[![Project Status: Concept – Minimal or no implementation has been done yet, or the repository is only intended to be a limited example, demo, or proof-of-concept.](https://www.repostatus.org/badges/latest/concept.svg)](https://www.repostatus.org/#concept)
[![CI Status](https://github.com/jwodder/rsrepo/actions/workflows/test.yml/badge.svg)](https://github.com/jwodder/rsrepo/actions/workflows/test.yml)
[![codecov.io](https://codecov.io/gh/jwodder/rsrepo/branch/master/graph/badge.svg)](https://codecov.io/gh/jwodder/rsrepo)
[![MIT License](https://img.shields.io/github/license/jwodder/rsrepo.svg)](https://opensource.org/licenses/MIT)

[GitHub](https://github.com/jwodder/rsrepo) | [Issues](https://github.com/jwodder/rsrepo/issues) | [Changelog](https://github.com/jwodder/rsrepo/blob/master/CHANGELOG.md)

`rsrepo` is my personal command-line program for managing my Rust project
repositories, including generating packaging boilerplate and performing
releases.  While it may in theory be suitable for general use, I make no
guarantees, nor do I intend to release it for general consumption.  Use at your
own risk.

Usage
=====

    rsrepo [<global options>] <subcommand> ...

All `rsrepo` subcommands other than `rsrepo new` must be run inside a Cargo
project directory & Git repository (after processing the `--chdir` option, if
given).

Certain commands automatically edit packages' `README.md` and/or `CHANGELOG.md`
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

- A GitHub access token must either be set via the `GH_TOKEN` or `GITHUB_TOKEN`
  environment variable or else have been saved with
  [`gh`](https://github.com/cli/cli) in order for various commands to perform
  GitHub REST API requests

- The `release` subcommand creates a signed Git tag, and so `gpg` (or another
  program specified via Git's `gpg.program` config variable) must be installed
  and usable

Configuration File
------------------

The configuration file (located at `~/.config/rsrepo.toml` by default) is a
[TOML](https://toml.io) file with the following fields:

- `author` *(required)* — The author name to use when `rsrepo new` generates
  `Cargo.toml` and `LICENSE` files

- `author-email` *(required)* — The author e-mail to use when `rsrepo new`
  generates a `Cargo.toml` file; this may contain a placeholder of the form
  `{package}`, which will be replaced with the name of the package being
  initialized.

- `github-user` — The GitHub username to use when `rsrepo new` generates
  `Cargo.toml` and `README.md` files and when `rsrepo mkgithub` creates a
  repository.  If this is not set, the value is fetched via the GitHub API when
  needed.

- `codecov-token` — Default value that the `rsrepo mkgithub` command should use
  for the `CODECOV_TOKEN` secret when no value is specified on the command line
  or in the environment

`rsrepo new`
------------

    rsrepo [<global options>] new [<options>] <directory>

Create a new Git repository at the given directory path and populate it with
basic files for a new Cargo project.

The following files are created in the directory:

- `.github/renovate.json5`
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
  package; if not specified, the `description` field in `Cargo.toml` will be
  commented out.

- `--lib` — Create a library crate.  This is the default if neither `--bin` nor
  `--lib` is given.

- `--msrv VERSION` — Specify the minimum supported Rust version to declare for
  the new package; defaults to the latest stable rustc version with the patch
  component removed.  The version must be given as either two or three
  dot-separated integers.

- `--name <name>` — Specify the package name to declare in the `Cargo.toml`
  file; defaults to the basename of the directory

- `--repo-name <name>` — Specify the GitHub repository name (sans owner) to use
  in URLs in generated files; defaults to the package name

`rsrepo begin-dev`
------------------

    rsrepo [<global options>] begin-dev

Prepare for development on the next version of the current package:

- Set `package.version` in `Cargo.toml` to the next minor version number plus
  "-dev"

- In a workspace, update the version requirements of the package's dependents

- If `CHANGELOG.md` exists, add a new section for the next minor version to the top

This is (almost) the same behavior as the last step of `rsrepo release`.

If the project is already in "dev mode", nothing is done.

`rsrepo inspect`
----------------

    rsrepo [<global options>] inspect [<options>]

Emit a JSON object describing the project and the workspace package (if any) in
the current directory.

### Options

- `-w`, `--workspace` — Also include details on all packages in the workspace

`rsrepo mkgithub`
-----------------

    rsrepo [<global options>] mkgithub [<options>] [<name>]

Create a new GitHub repository for the project, set the local repository's
`origin` remote to point to the GitHub repository, and push all branches & tags
to the remote.  In addition, if the `package.repository` field in the root
`Cargo.toml` is unset (or the `workspace.package.repository` field if the
project is a virtual workspace), it is set to the web URL of the GitHub
repository; if instead the field differs from the web URL, a warning is
emitted.

The GitHub repository will be created under the user account for the GitHub
access token in use.  Creating a repository under an organization is currently
not supported.

The package description (if any) is used as the repository description.  The
package's keywords are used as the repository's topics, along with the "`rust`"
topic; in addition, if the root `README.md` file has a "WIP"
[repostatus.org](https://www.repostatus.org) badge, the "`work-in-progress`"
topic is added.

- If the project is a virtual workspace, the description and keywords are drawn
  from the `[workspace.package]` table.

The custom labels used by the `.github/renovate.json5` file generated by
`rsrepo new` are created in the repository, a `CODECOV_TOKEN` secret is set for
GitHub Actions, automerging of PRs is enabled, and the tests listed in the
`.github/workflows/test.yml` file generated by `rsrepo new` are registered as
required checks for merging PRs.

The bare name of the repository to create (e.g. `hello-world`, not
`octocat/hello-world` or `https://github.com/octocat/hello-world`) can
optionally be specified as an argument on the command line; if not given, the
repository name is determined by parsing the `package.repository` field or (for
virtual workspaces) `workspace.package.repository` field in the `Cargo.toml`
file; if there is no such field, the root package name is used, erroring if the
project is a virtual workspace.  When parsing this field, it is an error if the
repository owner given in the URL differs from the `github-user` configuration
value.

### Options

- `--codecov-token <secret>` — Specify the value to use for the `CODECOV_TOKEN`
  secret.  This option can be set via the `CODECOV_TOKEN` environment variable,
  and a default value can be set via the configuration file.

  If no value is not set and `--no-codecov-token` is not given, a warning is
  emitted.

- `--no-codecov-token` — Do not set the `CODECOV_TOKEN` secret

- `--plan-only` — Do not create a GitHub repository, but do print a JSON object
  describing what would be created

- `-P`, `--private` — Make the new repository private

`rsrepo release`
----------------

    rsrepo [<global-options>] release [<options>] [<version>]

Prepare & publish a new release for a package.

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

When operating in a workspace, tags are prefixed with `{package_name}/`.  This
prefix is used when searching for the most recently-created Git tag, and it is
also stripped along with the optional leading `v` before checking for a valid
Cargo semver version.

This command performs the following operations in order:

- The version field in `Cargo.toml` is set to the release version.  If the
  project contains a `Cargo.lock` file, the version therein is set as well.

    - If operating in a workspace, any packages in the workspace that depend on
      the package being released have their dependency requirements updated to
      the new version if they're not already compatible with it, and their
      changelogs (if they have one) will be updated.

- If `CHANGELOG.md` exists, the header for the topmost section is edited to
  contain the release version and the current date.  It is an error if the
  topmost section header already contains a date.

- If the release version is not a prerelease and the `README.md` has a
  repostatus.org "WIP" badge, the badge is changed to "Active."

- If `publish` in `Cargo.toml` is not `false`, links to `crates.io` and (if the
  package contains a library crate) `docs.rs` are added to `README.md`'s header
  links.

- The copyright years in the first copyright line in `LICENSE` are updated to
  include all years in which commits were made to the repository, including the
  current year.  A line is treated as a copyright line if it is of the form
  "Copyright YEARS AUTHOR" or "Copyright (c) YEARS AUTHOR" (optional leading
  whitespace allowed for both forms), where the "YEARS" component consists of
  year numbers, dashes, commas, and/or spaces.  It is an error if `LICENSE`
  does not contain a copyright line.

- All changes made to tracked files in the repository are committed; the text
  of the most recent `CHANGELOG.md` section is included in the commit message
  template.

    - The release can be cancelled at this point by either leaving the commit
      message unchanged or by deleting the entire commit message.

- The commit is tagged as `v{version}` (or `{package_name}/v{version}` if in a
  workspace) and signed.

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

- If the repository does not contain a `.github/workflows/release.yml`
  workflow, then a GitHub release pointing to the new tag is created in the
  project's GitHub repository.  The name of the release is the first line of
  the tagged commit's commit message, and its body is the rest of the commit
  message.  If the new version is a prerelease, the GitHub release is marked as
  a prerelease as well.

    - The project's GitHub repository is identified by parsing the URL for the
      local Git repository's `origin` remote.

- If the repostatus.org badge in `README.md` was set to "Active" earlier, then
  any "`work-in-progress`" topic is removed from the GitHub repository's
  topics, and if `publish` in `Cargo.toml` is additionally not `false`, the
  "`available-on-crates-io`" topic is added.

- Development on the next version is started:

    - The version field in `Cargo.toml` (and `Cargo.lock`, if present) is set
      to the next minor version after the just-released version, plus a "-dev"
      prerelease segment.  In a workspace, versions required by dependent
      packages are updated as well.

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

- `-p <NAME>`, `--package <NAME>` — Release the package with the given name in
  the workspace.  By default, the package for the current directory is
  released.

- `--patch` — Set the release's version to the next micro version after the
  most recent Git tag

`rsrepo set-msrv`
-----------------

    rsrepo [<global-options>] set-msrv <version>

Set the package's MSRV as declared in `Cargo.toml` and `README.md`'s badges to
the given rustc version.  If the package has a `CHANGELOG.md` file, a basic
attempt at updating it to mention the MSRV change is performed, and the package
is also put into "dev mode" by running `begin-dev` on it.

### Options

- `-p <NAME>`, `--package <NAME>` — Update the package with the given name in
  the workspace.  By default, the package for the current directory is updated.

- `-w`, `--workspace` — Instead of updating a single package's
  `package.rust-version`, update `workspace.package.rust-version` in the
  project's root `Cargo.toml`, update the README in the project root, and
  update the README and CHANGELOG for all packages in the workspace that
  inherit the workspace MSRV.

  This option is mutually exclusive with `--package`.

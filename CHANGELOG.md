v0.7.0 (in development)
-----------------------
- `test.yml` template:
    - Update `actions/checkout` to v5
- Update `.pre-commit-config.yaml` template
- `Cargo.toml` template:
    - Update lints for Rust 1.91
    - Set `package.edition` to 2024

v0.6.0 (2025-06-27)
-------------------
- Fix support for updating versions of dependencies declared in non-inline
  tables
- `Cargo.toml` template:
    - Set rustc's `renamed_and_removed_lints` lint back to "warn" level
    - Rename `temporary_cstring_as_ptr` lint to
      `dangling_pointers_from_temporaries`
    - Update lints for Rust 1.87
    - Remove lints relating to pointers & FFI
- `test.yml` template:
    - Improve `clippy` invocation
    - Add `--cfg docsrs` to `RUSTDOCFLAGS`
    - Assume project is always a workspace with features
- `mkgithub`: Handle creating repositories without descriptions
- `release`:
    - **Bugfix**: Don't panic when run on a project whose README lacks header
      links
    - **Bugfix**: When bumping local workspace inter-dependencies:
        - Don't add `version` keys to specifiers that lack them
        - Don't treat `^x.y.z-dev` as accepting `x.y.z`
        - Don't update `Cargo.lock` until after dependents are updated
    - Unstash files in `{dir}.stash/` if `cargo publish` fails

v0.5.0 (2025-01-01)
-------------------
- Added `inspect` command
- Added workspace support to `release`
    - Added `--package` option for selecting the package to release
    - When releasing in a workspace, tags are now prefixed with
      `{package_name}/`.
- `release` now always updates `Cargo.lock` when the file is present,
  regardless of package type
- Header links in README files are now optional
- Added workspace support to `set-msrv`
    - Added `--package` option for selecting the package to update
    - Added `--workspace` option for setting MSRV workspace-wide
- Added workspace support to `mkgithub`
- `mkgithub`: Added `--plan-only` option
- When running `release` in a workspace, dependents with version requirements
  that are incompatible with the new or post-release version have their version
  requirements updated

v0.4.0 (2024-12-17)
-------------------
- Replace Dependabot with Renovate
- Use `cargo-minimal-versions` in `test.yml`
- `mkgithub` now additionally enables PR automerging and requiring tests to
  pass before merging
- `Cargo.toml` template:
    - Remove `pointer_structural_match` lint
    - Update lints for Rust 1.80
    - Set rustc's `unknown_lints` lint back to "warn" level
- `test.yml` template: Use v5 of Codecov action

v0.3.0 (2024-05-17)
-------------------
- Switch from nom to winnow
- `release`: Take name of repository's default branch into account when adding
  changelog link to README
- `mkgithub`: Set `CODECOV_TOKEN` secret for newly-created repositories
- `test.yml` template:
    - Improve Dependabot exclusion condition
    - Use v4 of Codecov action with token
- `Cargo.toml` template:
    - Update lints for Rust 1.76
    - Add empty `[dev-dependencies]` table after `[dependencies]` and above
      `[lints.*]`
    - Set `lints.rust.unused_variables = "warn"`
- `.github/dependabot.yml` template:
    - Change cargo update interval from weekly to monthly
    - Remove "include: scope" lines

v0.2.0 (2023-11-22)
-------------------
- `new`: When determining the default MSRV from the latest Rust release, strip
  the patch version
- `release`: Stop passing `--offline` to the `cargo update` command run to
  update a binary project's version in the lockfile, as the flag was causing
  problems
- `release`: When setting the version to the next development version after
  releasing, also update `Cargo.lock`
- Adjust `.github/workflows/test.yml` template:
    - `llvm-tools-preview` is now called `llvm-tools`
    - Restrict push triggers to the default branch
- If the `github-user` configuration field is not set, its value will now be
  fetched via the GitHub API when needed
- Add more extensive linting to project template
    - Add `[lints]` tables to `Cargo.toml`
    - Add `clippy.toml`
    - Remove `-Dwarnings` from `clippy` hook in `.pre-commit-config.yaml`
- `.pre-commit-config.yaml` template: Remove `exclude: '^tests/data/'` from
  end-of-file-fixer hook

v0.1.0 (2023-09-29)
-------------------
Initial release

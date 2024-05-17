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

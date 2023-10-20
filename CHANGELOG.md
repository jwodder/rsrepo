v0.2.0 (in development)
-----------------------
- `new`: When determining the default MSRV from the latest Rust release, strip
  the patch version
- `release`: Stop passing `--offline` to the `cargo update` command run to
  update a binary project's version in the lockfile, as the flag was causing
  problems
- `release`: When setting the version to the next development version after
  releasing, also update `Cargo.lock`

v0.1.0 (2023-09-29)
-------------------
Initial release

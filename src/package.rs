use crate::changelog::Changelog;
use crate::cmd::{CommandOutputError, LoggedCommand};
use crate::git::Git;
use crate::readme::Readme;
use crate::util::CopyrightLine;
use anyhow::{bail, Context};
use cargo_metadata::{MetadataCommand, Package as CargoPackage};
use semver::Version;
use serde::Deserialize;
use std::borrow::Cow;
use std::fs::{read_to_string, File};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::Document;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Package {
    manifest_path: PathBuf,
}

impl Package {
    pub fn locate() -> Result<Package, LocatePackageError> {
        let output = LoggedCommand::new("cargo")
            .arg("locate-project")
            .check_output()?;
        let location = serde_json::from_str::<LocateProject<'_>>(&output)?;
        if !location.root.is_absolute() {
            return Err(LocatePackageError::InvalidPath(location.root.into()));
        }
        if location.root.parent().is_some() {
            Ok(Package {
                manifest_path: location.root.into(),
            })
        } else {
            Err(LocatePackageError::InvalidPath(location.root.into()))
        }
    }

    pub fn path(&self) -> &Path {
        self.manifest_path.parent().unwrap()
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn is_bin(&self) -> anyhow::Result<bool> {
        let srcdir = self.path().join("src");
        Ok(srcdir
            .join("main.rs")
            .try_exists()
            .context("could not determine whether src/main.rs exists")?
            || srcdir
                .join("bin")
                .try_exists()
                .context("could not determine whether src/bin/ exists")?)
    }

    pub fn is_lib(&self) -> anyhow::Result<bool> {
        let srcdir = self.path().join("src");
        srcdir
            .join("lib.rs")
            .try_exists()
            .context("could not determine whether src/main.rs exists")
    }

    pub fn latest_tag_version(&self) -> anyhow::Result<Option<Version>> {
        if let Some(tag) = self.git().latest_tag()? {
            tag.strip_prefix('v')
                .unwrap_or(&tag)
                .parse::<Version>()
                .with_context(|| format!("Failed to parse latest Git tag {tag:?} as a version"))
                .map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn set_cargo_version(&self, v: Version) -> anyhow::Result<()> {
        let src =
            read_to_string(self.path().join("Cargo.toml")).context("Failed to read Cargo.toml")?;
        let mut doc = src
            .parse::<Document>()
            .context("Failed to parse Cargo.toml")?;
        doc["package"]["version"] = toml_edit::value(v.to_string());
        let mut fp = File::create(self.path().join("Cargo.toml"))
            .context("failed to open Cargo.toml for writing")?;
        write!(&mut fp, "{}", doc).context("failed writing to Cargo.toml")?;
        Ok(())
    }

    pub fn git(&self) -> Git<'_> {
        Git::new(self.path())
    }

    pub fn metadata(&self) -> anyhow::Result<CargoPackage> {
        MetadataCommand::new()
            .manifest_path(self.manifest_path())
            .no_deps()
            .exec()
            .context("Failed to get project metadata")?
            .packages
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No packages listed in metadata"))
    }

    pub fn readme(&self) -> anyhow::Result<Option<Readme>> {
        match read_to_string(self.path().join("README.md")) {
            Ok(s) => Ok(Some(s.parse::<Readme>()?)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("failed to read README.md"),
        }
    }

    pub fn set_readme(&self, readme: Readme) -> anyhow::Result<()> {
        let mut fp = File::create(self.path().join("README.md"))
            .context("failed to open README.md for writing")?;
        write!(&mut fp, "{}", readme).context("failed writing to README.md")?;
        Ok(())
    }

    pub fn changelog(&self) -> anyhow::Result<Option<Changelog>> {
        match read_to_string(self.path().join("CHANGELOG.md")) {
            Ok(s) => Ok(Some(s.parse::<Changelog>()?)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).context("failed to read CHANGELOG.md"),
        }
    }

    pub fn set_changelog(&self, changelog: Changelog) -> anyhow::Result<()> {
        let mut fp = File::create(self.path().join("CHANGELOG.md"))
            .context("failed to open CHANGELOG.md for writing")?;
        write!(&mut fp, "{}", changelog).context("failed writing to CHANGELOG.md")?;
        Ok(())
    }

    pub fn update_license_years<I>(&self, years: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = i32>,
    {
        let mut years = Some(years);
        let license =
            read_to_string(self.path().join("LICENSE")).context("failed to read LICENSE file")?;
        let mut found = false;
        let mut fp = File::create(self.path().join("LICENSE"))
            .context("failed to open LICENSE for writing")?;
        for line in license.lines() {
            match (found, line.parse::<CopyrightLine>()) {
                (false, Ok(mut crl)) => {
                    found = true;
                    if let Some(years) = years.take() {
                        for y in years {
                            crl.add_year(y);
                        }
                    }
                    writeln!(fp, "{crl}").context("error writing to LICENSE")?;
                }
                _ => writeln!(fp, "{line}").context("error writing to LICENSE")?,
            }
        }
        if !found {
            bail!("Copyright line not found in LICENSE");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
struct LocateProject<'a> {
    #[serde(borrow)]
    root: Cow<'a, Path>,
}

#[derive(Debug, Error)]
pub enum LocatePackageError {
    #[error("could not get project root from cargo")]
    Command(#[from] CommandOutputError),
    #[error("could not deserialize `cargo locate-project` output")]
    Deserialize(#[from] serde_json::Error),
    #[error("manifest path is absolute or parentless: {}", .0.display())]
    InvalidPath(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use assert_fs::TempDir;

    #[test]
    fn set_cargo_version() {
        let tmpdir = TempDir::new().unwrap();
        let manifest = tmpdir.child("Cargo.toml");
        manifest
            .write_str(concat!(
                "[package]\n",
                "name = \"foobar\"\n",
                "version = \"0.1.0\"\n",
                "edition = \"2021\"\n",
                "\n",
                "[dependencies]\n",
            ))
            .unwrap();
        let package = Package {
            manifest_path: manifest.path().into(),
        };
        package.set_cargo_version(Version::new(1, 2, 3)).unwrap();
        manifest.assert(concat!(
            "[package]\n",
            "name = \"foobar\"\n",
            "version = \"1.2.3\"\n",
            "edition = \"2021\"\n",
            "\n",
            "[dependencies]\n",
        ));
    }

    #[test]
    fn update_license_years() {
        let tmpdir = TempDir::new().unwrap();
        let manifest = tmpdir.child("Cargo.toml");
        let license = tmpdir.child("LICENSE");
        license
            .write_str(concat!(
                "The Foobar License\n",
                "\n",
                "Copyright (c) 2021-2022 John T. Wodder II\n",
                "Copyright (c) 2020 The Prime Mover and their Agents\n",
                "\n",
                "Permission is not granted.\n",
            ))
            .unwrap();
        let package = Package {
            manifest_path: manifest.path().into(),
        };
        package.update_license_years([2023]).unwrap();
        license.assert(concat!(
            "The Foobar License\n",
            "\n",
            "Copyright (c) 2021-2023 John T. Wodder II\n",
            "Copyright (c) 2020 The Prime Mover and their Agents\n",
            "\n",
            "Permission is not granted.\n",
        ));
    }
}

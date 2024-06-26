use crate::changelog::Changelog;
use crate::cmd::{CommandOutputError, LoggedCommand};
use crate::git::Git;
use crate::readme::Readme;
use crate::util::CopyrightLine;
use anyhow::{bail, Context};
use cargo_metadata::{MetadataCommand, Package as CargoPackage};
use fs_err::{read_to_string, File};
use in_place::InPlace;
use semver::Version;
use serde::Deserialize;
use std::borrow::Cow;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::DocumentMut;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Package {
    manifest_path: PathBuf,
}

impl Package {
    pub(crate) fn locate() -> Result<Package, LocatePackageError> {
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

    pub(crate) fn path(&self) -> &Path {
        self.manifest_path
            .parent()
            .expect("manifest_path was verified to have a parent on Package construction")
    }

    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub(crate) fn is_bin(&self) -> anyhow::Result<bool> {
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

    pub(crate) fn is_lib(&self) -> anyhow::Result<bool> {
        let srcdir = self.path().join("src");
        srcdir
            .join("lib.rs")
            .try_exists()
            .context("could not determine whether src/main.rs exists")
    }

    pub(crate) fn latest_tag_version(&self) -> anyhow::Result<Option<Version>> {
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

    pub(crate) fn git(&self) -> Git<'_> {
        Git::new(self.path())
    }

    pub(crate) fn metadata(&self) -> anyhow::Result<CargoPackage> {
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

    pub(crate) fn readme(&self) -> TextFile<'_, Readme> {
        TextFile {
            dirpath: self.path(),
            filename: "README.md",
            _type: PhantomData,
        }
    }

    pub(crate) fn changelog(&self) -> TextFile<'_, Changelog> {
        TextFile {
            dirpath: self.path(),
            filename: "CHANGELOG.md",
            _type: PhantomData,
        }
    }

    pub(crate) fn manifest(&self) -> TextFile<'_, DocumentMut> {
        TextFile {
            dirpath: self.path(),
            filename: "Cargo.toml",
            _type: PhantomData,
        }
    }

    pub(crate) fn set_package_field<V: Into<toml_edit::Value>>(
        &self,
        key: &str,
        value: V,
    ) -> anyhow::Result<()> {
        let manifest = self.manifest();
        let Some(mut doc) = manifest.get()? else {
            bail!("Package lacks Cargo.toml");
        };
        let Some(pkg) = doc.get_mut("package").and_then(|it| it.as_table_like_mut()) else {
            bail!("No [package] table in Cargo.toml");
        };
        pkg.insert(key, toml_edit::value(value));
        manifest.set(doc)?;
        Ok(())
    }

    pub(crate) fn set_cargo_version(&self, v: Version) -> anyhow::Result<()> {
        let vs = v.to_string();
        self.set_package_field("version", &vs)?;
        if self.is_bin()? && self.path().join("Cargo.lock").exists() {
            LoggedCommand::new("cargo")
                .arg("update")
                .arg("-p")
                .arg(self.metadata()?.name)
                .arg("--precise")
                .arg(vs)
                .current_dir(self.path())
                .status()?;
        }
        Ok(())
    }

    pub(crate) fn update_license_years<I>(&self, years: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = i32>,
    {
        let mut years = Some(years);
        let inp = InPlace::new(self.path().join("LICENSE"))
            .open()
            .context("failed to open LICENSE file for in-place editing")?;
        let reader = BufReader::new(inp.reader());
        let mut writer = inp.writer();
        let mut found = false;
        for line in reader.lines() {
            let line = line.context("failed to read lines from LICENSE")?;
            match (found, line.parse::<CopyrightLine>()) {
                (false, Ok(mut crl)) => {
                    found = true;
                    if let Some(years) = years.take() {
                        for y in years {
                            crl.add_year(y);
                        }
                    }
                    writeln!(writer, "{crl}").context("error writing to LICENSE")?;
                }
                _ => writeln!(writer, "{line}").context("error writing to LICENSE")?,
            }
        }
        if !found {
            bail!("copyright line not found in LICENSE");
        }
        inp.save().context("failed to save changed to LICENSE")?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
struct LocateProject<'a> {
    #[serde(borrow)]
    root: Cow<'a, Path>,
}

#[derive(Debug, Error)]
pub(crate) enum LocatePackageError {
    #[error("could not get project root from cargo")]
    Command(#[from] CommandOutputError),
    #[error("could not deserialize `cargo locate-project` output")]
    Deserialize(#[from] serde_json::Error),
    #[error("manifest path is absolute or parentless: {}", .0.display())]
    InvalidPath(PathBuf),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextFile<'a, T> {
    dirpath: &'a Path,
    filename: &'static str,
    _type: PhantomData<T>,
}

impl<T> TextFile<'_, T> {
    pub(crate) fn get(&self) -> anyhow::Result<Option<T>>
    where
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::error::Error + Send + Sync + 'static,
    {
        match read_to_string(self.dirpath.join(self.filename)) {
            Ok(s) => {
                Ok(Some(s.parse::<T>().with_context(|| {
                    format!("failed to parse {}", self.filename)
                })?))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn set(&self, content: T) -> anyhow::Result<()>
    where
        T: std::fmt::Display,
    {
        let mut fp = File::create(self.dirpath.join(self.filename))
            .with_context(|| format!("failed to open {} for writing", self.filename))?;
        write!(&mut fp, "{content}")
            .with_context(|| format!("failed writing to {}", self.filename))?;
        Ok(())
    }
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
    fn set_cargo_version_inline() {
        let tmpdir = TempDir::new().unwrap();
        let manifest = tmpdir.child("Cargo.toml");
        manifest
            .write_str("package = { name = \"foobar\", version = \"0.1.0\", edition = \"2021\" }\ndependencies = {}\n")
            .unwrap();
        let package = Package {
            manifest_path: manifest.path().into(),
        };
        package.set_cargo_version(Version::new(1, 2, 3)).unwrap();
        manifest.assert("package = { name = \"foobar\", version = \"1.2.3\", edition = \"2021\" }\ndependencies = {}\n");
    }

    #[test]
    fn set_cargo_version_unset() {
        let tmpdir = TempDir::new().unwrap();
        let manifest = tmpdir.child("Cargo.toml");
        manifest
            .write_str(concat!(
                "[package]\n",
                "name = \"foobar\"\n",
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
            "edition = \"2021\"\n",
            "version = \"1.2.3\"\n",
            "\n",
            "[dependencies]\n",
        ));
    }

    #[test]
    fn set_cargo_version_no_package() {
        let tmpdir = TempDir::new().unwrap();
        let manifest = tmpdir.child("Cargo.toml");
        manifest.write_str("[dependencies]\n").unwrap();
        let package = Package {
            manifest_path: manifest.path().into(),
        };
        assert!(package.set_cargo_version(Version::new(1, 2, 3)).is_err());
        manifest.assert("[dependencies]\n");
    }

    #[test]
    fn set_cargo_version_package_not_table() {
        let tmpdir = TempDir::new().unwrap();
        let manifest = tmpdir.child("Cargo.toml");
        manifest.write_str("package = 42\n").unwrap();
        let package = Package {
            manifest_path: manifest.path().into(),
        };
        assert!(package.set_cargo_version(Version::new(1, 2, 3)).is_err());
        manifest.assert("package = 42\n");
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

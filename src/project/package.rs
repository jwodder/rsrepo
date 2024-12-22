use super::textfile::TextFile;
use crate::changelog::Changelog;
use crate::cmd::LoggedCommand;
use crate::git::Git;
use crate::project::Project;
use crate::readme::Readme;
use crate::util::CopyrightLine;
use anyhow::{bail, Context};
use cargo_metadata::{Package as CargoPackage, TargetKind};
use in_place::InPlace;
use semver::Version;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use toml_edit::DocumentMut;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Package {
    metadata: CargoPackage,
    is_root: bool,
}

impl Package {
    pub(crate) fn new(metadata: CargoPackage, is_root: bool) -> Package {
        Package { metadata, is_root }
    }

    pub(crate) fn locate() -> anyhow::Result<Package> {
        let project = Project::locate()?;
        let Some(package) = project.package_set()?.into_current_package()? else {
            bail!("no package in current directory");
        };
        Ok(package)
    }

    pub(crate) fn path(&self) -> &Path {
        self.manifest_path()
            .parent()
            .expect("manifest_path should have a parent")
    }

    pub(crate) fn manifest_path(&self) -> &Path {
        self.metadata.manifest_path.as_std_path()
    }

    pub(crate) fn is_bin(&self) -> bool {
        self.metadata
            .targets
            .iter()
            .flat_map(|t| t.kind.iter())
            .any(|k| k == &TargetKind::Bin)
    }

    pub(crate) fn is_lib(&self) -> bool {
        self.metadata
            .targets
            .iter()
            .flat_map(|t| t.kind.iter())
            .any(|k| k == &TargetKind::Lib)
    }

    pub(crate) fn latest_tag_version(
        &self,
        prefix: Option<&str>,
    ) -> anyhow::Result<Option<Version>> {
        if let Some(tag) = self.git().latest_tag(prefix)? {
            let tagv = match prefix {
                Some(pre) => tag.strip_prefix(pre).unwrap_or(&*tag),
                None => &*tag,
            };
            tagv.strip_prefix('v')
                .unwrap_or(tagv)
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

    pub(crate) fn metadata(&self) -> &CargoPackage {
        &self.metadata
    }

    pub(crate) fn name(&self) -> &str {
        &self.metadata.name
    }

    pub(crate) fn is_root_package(&self) -> bool {
        self.is_root
    }

    pub(crate) fn readme(&self) -> TextFile<'_, Readme> {
        TextFile::new(self.path(), "README.md")
    }

    pub(crate) fn changelog(&self) -> TextFile<'_, Changelog> {
        TextFile::new(self.path(), "CHANGELOG.md")
    }

    pub(crate) fn manifest(&self) -> TextFile<'_, DocumentMut> {
        TextFile::new(self.path(), "Cargo.toml")
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

    pub(crate) fn set_cargo_version(&self, v: Version, update_lock: bool) -> anyhow::Result<()> {
        let vs = v.to_string();
        self.set_package_field("version", &vs)?;
        if update_lock {
            LoggedCommand::new("cargo")
                .arg("update")
                .arg("-p")
                .arg(self.name())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::ChildPath, TempDir};

    struct TestPackage {
        package: Package,
        tmpdir: TempDir,
        manifest: ChildPath,
    }

    impl TestPackage {
        fn new(manifest_src: &str) -> TestPackage {
            let tmpdir = TempDir::new().unwrap();
            let manifest = tmpdir.child("Cargo.toml");
            manifest.write_str(manifest_src).unwrap();
            tmpdir.child("src").create_dir_all().unwrap();
            tmpdir.child("src").child("main.rs").touch().unwrap();
            let package = Project::for_manifest_path(manifest.path())
                .unwrap()
                .package_set()
                .unwrap()
                .into_root_package()
                .unwrap();
            TestPackage {
                package,
                tmpdir,
                manifest,
            }
        }
    }

    #[test]
    fn set_cargo_version() {
        let tpkg = TestPackage::new(concat!(
            "[package]\n",
            "name = \"foobar\"\n",
            "version = \"0.1.0\"\n",
            "edition = \"2021\"\n",
            "\n",
            "[dependencies]\n",
        ));
        tpkg.package
            .set_cargo_version(Version::new(1, 2, 3), false)
            .unwrap();
        tpkg.manifest.assert(concat!(
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
        let tpkg = TestPackage::new("package = { name = \"foobar\", version = \"0.1.0\", edition = \"2021\" }\ndependencies = {}\n");
        tpkg.package
            .set_cargo_version(Version::new(1, 2, 3), false)
            .unwrap();
        tpkg.manifest.assert("package = { name = \"foobar\", version = \"1.2.3\", edition = \"2021\" }\ndependencies = {}\n");
    }

    #[test]
    fn set_cargo_version_unset() {
        let tpkg = TestPackage::new(concat!(
            "[package]\n",
            "name = \"foobar\"\n",
            "edition = \"2021\"\n",
            "\n",
            "[dependencies]\n",
        ));
        tpkg.package
            .set_cargo_version(Version::new(1, 2, 3), false)
            .unwrap();
        tpkg.manifest.assert(concat!(
            "[package]\n",
            "name = \"foobar\"\n",
            "edition = \"2021\"\n",
            "version = \"1.2.3\"\n",
            "\n",
            "[dependencies]\n",
        ));
    }

    #[test]
    #[ignore] // TODO: Update or remove
    fn set_cargo_version_no_package() {
        let tpkg = TestPackage::new("[dependencies]\n");
        assert!(tpkg
            .package
            .set_cargo_version(Version::new(1, 2, 3), false)
            .is_err());
        tpkg.manifest.assert("[dependencies]\n");
    }

    #[test]
    #[ignore] // TODO: Update or remove
    fn set_cargo_version_package_not_table() {
        let tpkg = TestPackage::new("package = 42\n");
        assert!(tpkg
            .package
            .set_cargo_version(Version::new(1, 2, 3), false)
            .is_err());
        tpkg.manifest.assert("package = 42\n");
    }

    #[test]
    fn update_license_years() {
        let tpkg = TestPackage::new(concat!(
            "[package]\n",
            "name = \"foobar\"\n",
            "version = \"0.1.0\"\n",
            "edition = \"2021\"\n",
            "\n",
            "[dependencies]\n",
        ));
        let license = tpkg.tmpdir.child("LICENSE");
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
        tpkg.package.update_license_years([2023]).unwrap();
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

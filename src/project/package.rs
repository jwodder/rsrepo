use super::textfile::TextFile;
use super::traits::HasReadme;
use super::Flavor;
use crate::changelog::Changelog;
use crate::cmd::LoggedCommand;
use crate::project::Project;
use crate::readme::Readme;
use crate::util::CopyrightLine;
use anyhow::{bail, Context};
use cargo_metadata::{Package as CargoPackage, TargetKind};
use in_place::InPlace;
use semver::{Version, VersionReq};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use toml_edit::DocumentMut;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Package {
    metadata: CargoPackage,
    is_root: bool,
    dependents: BTreeMap<String, VersionReq>,
}

impl Package {
    pub(super) fn new(
        metadata: CargoPackage,
        is_root: bool,
        dependents: BTreeMap<String, VersionReq>,
    ) -> Package {
        Package {
            metadata,
            is_root,
            dependents,
        }
    }

    #[allow(dead_code)]
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

    pub(crate) fn metadata(&self) -> &CargoPackage {
        &self.metadata
    }

    pub(crate) fn name(&self) -> &str {
        &self.metadata.name
    }

    pub(crate) fn is_root_package(&self) -> bool {
        self.is_root
    }

    pub(crate) fn dependents(&self) -> &BTreeMap<String, VersionReq> {
        &self.dependents
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

    pub(crate) fn set_dependency_version<V: Into<toml_edit::Value> + Clone>(
        &self,
        package: &str,
        req: V,
    ) -> anyhow::Result<()> {
        let manifest = self.manifest();
        let Some(mut doc) = manifest.get()? else {
            bail!("Package lacks Cargo.toml");
        };
        for tblname in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(tbl) = doc.get_mut(tblname) else {
                continue;
            };
            let Some(tbl) = tbl.as_table_like_mut() else {
                bail!("{tblname:?} field in Cargo.toml is not a table");
            };
            let Some(reqitem) = tbl.get_mut(package) else {
                continue;
            };
            if reqitem.is_str() {
                tbl.insert(package, toml_edit::value(req.clone()));
            } else if let Some(t) = reqitem.as_table_like_mut() {
                t.insert("version", toml_edit::value(req.clone()));
            } else {
                bail!("{tblname}.{package} in Cargo.toml is not a string or table");
            }
        }
        manifest.set(doc)?;
        Ok(())
    }

    pub(crate) fn package_key_inherits_workspace(&self, key: &str) -> anyhow::Result<bool> {
        let manifest = self.manifest();
        let Some(doc) = manifest.get()? else {
            bail!("Package lacks Cargo.toml");
        };
        let Some(pkg) = doc.get("package").and_then(|it| it.as_table_like()) else {
            bail!("No [package] table in Cargo.toml");
        };
        Ok(pkg
            .get(key)
            .and_then(|it| it.as_table_like())
            .and_then(|tbl| tbl.get("workspace"))
            .and_then(|it| it.as_value())
            .and_then(toml_edit::Value::as_bool)
            == Some(true))
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

    pub(crate) fn flavor(&self) -> Flavor {
        Flavor {
            name: Some(self.metadata.name.clone()),
            description: self.metadata.description.clone(),
            repository: self.metadata.repository.clone(),
            keywords: self.metadata.keywords.clone(),
        }
    }
}

impl HasReadme for Package {
    fn readme(&self) -> TextFile<'_, Readme> {
        TextFile::new(self.path(), "README.md")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use assert_fs::{fixture::ChildPath, prelude::*, TempDir};
    use indoc::indoc;

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

    mod set_cargo_version {
        use super::*;

        #[test]
        fn normal() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
            "#});
            tpkg.package
                .set_cargo_version(Version::new(1, 2, 3), false)
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "1.2.3"
                edition = "2021"

                [dependencies]
            "#});
        }

        #[test]
        fn inline() {
            let tpkg = TestPackage::new("package = { name = \"foobar\", version = \"0.1.0\", edition = \"2021\" }\ndependencies = {}\n");
            tpkg.package
                .set_cargo_version(Version::new(1, 2, 3), false)
                .unwrap();
            tpkg.manifest.assert("package = { name = \"foobar\", version = \"1.2.3\", edition = \"2021\" }\ndependencies = {}\n");
        }

        #[test]
        fn unset() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                edition = "2021"

                [dependencies]
            "#});
            tpkg.package
                .set_cargo_version(Version::new(1, 2, 3), false)
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                edition = "2021"
                version = "1.2.3"

                [dependencies]
            "#});
        }
    }

    #[test]
    fn update_license_years() {
        let tpkg = TestPackage::new(indoc! {r#"
            [package]
            name = "foobar"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
        "#});
        let license = tpkg.tmpdir.child("LICENSE");
        license
            .write_str(indoc! {"
                The Foobar License

                Copyright (c) 2021-2022 John T. Wodder II
                Copyright (c) 2020 The Prime Mover and their Agents

                Permission is not granted.
            "})
            .unwrap();
        tpkg.package.update_license_years([2023]).unwrap();
        license.assert(indoc! {"
            The Foobar License

            Copyright (c) 2021-2023 John T. Wodder II
            Copyright (c) 2020 The Prime Mover and their Agents

            Permission is not granted.
        "});
    }

    mod set_dependency_version {
        use super::*;

        #[test]
        fn normal_dep() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = "0.1.0"
            "#});
            tpkg.package
                .set_dependency_version("quux", "1.2.3")
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = "1.2.3"
            "#});
        }

        #[test]
        fn dev_dep() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = "0.1.0"

                [dev-dependencies]
                glarch = "1.2.3"
            "#});
            tpkg.package
                .set_dependency_version("glarch", "42.0")
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = "0.1.0"

                [dev-dependencies]
                glarch = "42.0"
            "#});
        }

        #[test]
        fn build_dep() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = "0.1.0"

                [build-dependencies]
                glarch = "1.2.3"
            "#});
            tpkg.package
                .set_dependency_version("glarch", "42.0")
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = "0.1.0"

                [build-dependencies]
                glarch = "42.0"
            "#});
        }

        #[test]
        fn every_dep_type() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                glarch = "1.2.3"
                quux = "0.1.0"

                [dev-dependencies]
                glarch = "1.2.3"
                quux = "0.1.0"

                [build-dependencies]
                glarch = "1.2.3"
                quux = "0.1.0"
            "#});
            tpkg.package
                .set_dependency_version("glarch", "42.0")
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                glarch = "42.0"
                quux = "0.1.0"

                [dev-dependencies]
                glarch = "42.0"
                quux = "0.1.0"

                [build-dependencies]
                glarch = "42.0"
                quux = "0.1.0"
            "#});
        }

        #[test]
        fn inline_table_dep() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = { version = "0.1.0", default-features = false }
            "#});
            tpkg.package
                .set_dependency_version("quux", "1.2.3")
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = { version = "1.2.3", default-features = false }
            "#});
        }

        #[test]
        fn inline_table_dep_no_version() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = { path = "../quux", default-features = false }
            "#});
            tpkg.package
                .set_dependency_version("quux", "1.2.3")
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = { path = "../quux", default-features = false , version = "1.2.3" }
            "#});
        }

        #[test]
        fn table_dep() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies.quux]
                version = "0.1.0"
                default-features = false
            "#});
            tpkg.package
                .set_dependency_version("quux", "1.2.3")
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies.quux]
                version = "1.2.3"
                default-features = false
            "#});
        }

        #[test]
        fn table_dep_no_version() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies.quux]
                path = "../quux"
                default-features = false
            "#});
            tpkg.package
                .set_dependency_version("quux", "1.2.3")
                .unwrap();
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies.quux]
                path = "../quux"
                default-features = false
                version = "1.2.3"
            "#});
        }
    }
}

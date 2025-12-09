use super::textfile::TextFile;
use super::traits::HasReadme;
use super::{Flavor, PackageSet, Project};
use crate::changelog::{Changelog, ChangelogHeader, ChangelogSection};
use crate::cmd::LoggedCommand;
use crate::readme::Readme;
use crate::util::{Bump, CopyrightLine, bump_version};
use anyhow::{Context, bail};
use cargo_metadata::{
    Package as CargoPackage, TargetKind,
    semver::{Op, Prerelease, Version, VersionReq},
};
use in_place::InPlace;
use std::collections::BTreeMap;
use std::fmt::Write as _;
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
        self.metadata.name.as_ref()
    }

    pub(crate) fn is_root_package(&self) -> bool {
        self.is_root
    }

    pub(crate) fn dependents(&self) -> &BTreeMap<String, VersionReq> {
        &self.dependents
    }

    pub(crate) fn is_public(&self) -> bool {
        self.metadata.publish.as_deref() != Some(&[])
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

    pub(crate) fn set_cargo_version(&self, v: &Version) -> anyhow::Result<()> {
        self.set_package_field("version", v.to_string())?;
        Ok(())
    }

    pub(crate) fn set_version_and_bump_dependents(
        &self,
        new_version: &Version,
        pkgset: &PackageSet,
    ) -> anyhow::Result<()> {
        self.set_cargo_version(new_version)?;
        bump_dependents(pkgset, self, new_version)?;
        if self.path().join("Cargo.lock").exists() {
            // Do this AFTER updating dependents!
            self.update_lockfile(new_version)?;
        }
        Ok(())
    }

    pub(crate) fn update_lockfile(&self, v: &Version) -> anyhow::Result<()> {
        LoggedCommand::new("cargo")
            .arg("update")
            .arg("-p")
            .arg(self.name())
            .arg("--precise")
            .arg(v.to_string())
            .current_dir(self.path())
            .status()
            .map_err(Into::into)
    }

    pub(crate) fn set_dependency_version<V: Into<toml_edit::Value> + Clone>(
        &self,
        package: &str,
        req: V,
        create: bool,
    ) -> anyhow::Result<Vec<&'static str>> {
        let manifest = self.manifest();
        let Some(mut doc) = manifest.get()? else {
            bail!("Package lacks Cargo.toml");
        };
        let mut changed = Vec::new();
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
                changed.push(tblname);
            } else if let Some(t) = reqitem.as_table_like_mut() {
                if create || t.contains_key("version") {
                    t.insert("version", toml_edit::value(req.clone()));
                    changed.push(tblname);
                }
            } else {
                bail!("{tblname}.{package} in Cargo.toml is not a string or table");
            }
        }
        manifest.set(doc)?;
        Ok(changed)
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

    pub(crate) fn begin_dev<'a>(&'a self, package_set: &'a PackageSet) -> BeginDev<'a> {
        BeginDev::new(self, package_set)
    }

    pub(crate) fn flavor(&self) -> Flavor {
        Flavor {
            name: Some(self.metadata.name.to_string()),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BeginDev<'a> {
    package: &'a Package,
    pkgset: &'a PackageSet,
    latest_release: Option<(Version, chrono::NaiveDate)>,
    quiet: bool,
    force: bool,
}

impl<'a> BeginDev<'a> {
    fn new(package: &'a Package, package_set: &'a PackageSet) -> Self {
        BeginDev {
            package,
            pkgset: package_set,
            latest_release: None,
            quiet: false,
            force: false,
        }
    }

    pub(crate) fn latest_release(mut self, version: Version, date: chrono::NaiveDate) -> Self {
        self.latest_release = Some((version, date));
        self
    }

    pub(crate) fn quiet(mut self, yes: bool) -> Self {
        self.quiet = yes;
        self
    }

    pub(crate) fn force(mut self, yes: bool) -> Self {
        self.force = yes;
        self
    }

    pub(crate) fn run(self) -> anyhow::Result<()> {
        let current_version = &self.package.metadata().version;
        if !self.force && !current_version.pre.is_empty() {
            if !self.quiet {
                log::info!("Project is already in dev state; not adjusting");
            }
            return Ok(());
        }

        log::info!("Preparing for work on next version ...");
        let latest_version = match self.latest_release {
            Some((ref version, _)) => version.clone(),
            None => current_version.clone(),
        };
        let next_version = bump_version(latest_version, Bump::Minor);
        let mut dev_next = next_version.clone();
        dev_next.pre =
            Prerelease::new("dev").expect("'dev' should be a valid prerelease identifier");

        // Update version in Cargo.toml
        log::info!("Setting next version in Cargo.toml ...");
        self.package
            .set_version_and_bump_dependents(&dev_next, self.pkgset)?;

        // If `self.latest_release` is set, ensure CHANGELOG exists
        let chlog_file = self.package.changelog();
        let mut chlog = chlog_file.get()?;
        if chlog.is_none()
            && let Some((version, date)) = self.latest_release
        {
            chlog = Some(Changelog {
                sections: vec![ChangelogSection {
                    header: ChangelogHeader::Released { version, date },
                    content: "Initial release\n".into(),
                }],
            });
        }
        // If CHANGELOG exists, ensure it contains section for upcoming version
        if let Some(mut chlog) = chlog {
            if chlog
                .sections
                .first()
                .is_none_or(|sect| matches!(sect.header, ChangelogHeader::Released { .. }))
            {
                log::info!("Adding next section to CHANGELOG.md ...");
                chlog.sections.insert(
                    0,
                    ChangelogSection {
                        header: ChangelogHeader::InProgress {
                            version: next_version,
                        },
                        content: String::new(),
                    },
                );
                chlog_file.set(chlog)?;
            }
        } else {
            log::info!("No CHANGELOG.md file to add next section to");
        }
        Ok(())
    }
}

fn bump_dependents(
    pkgset: &PackageSet,
    package: &Package,
    version: &Version,
) -> anyhow::Result<()> {
    let name = package.name();
    for (rname, req) in package.dependents() {
        // When a package `foo`'s version is bumped from `0.3.0-dev` to
        // `0.3.0`, any package `bar` that depends on `foo 0.3.0-dev` should
        // have its version requirement bumped to `0.3.0`, but Cargo's semver
        // rules mean that `^0.3.0-dev` accepts `0.3.0`.  Thus, if `req` using
        // a prelease does not equal `version` being a prerelease, bump.
        if !req.matches(version) || uses_prerelease(req) == version.pre.is_empty() {
            let Some(rpkg) = pkgset.package_by_name(rname) else {
                bail!(
                    "Inconsistent project metadata: {name} is depended on by {rname}, but the latter was not found"
                );
            };
            log::info!("Updating {rname}'s dependency on {name} ...");
            let changed = rpkg.set_dependency_version(name, version.to_string(), false)?;
            if version.pre.is_empty() && changed.contains(&"dependencies") {
                let chlog_file = rpkg.changelog();
                if chlog_file.exists() {
                    rpkg.begin_dev(pkgset).quiet(true).run()?;
                    if let Some(mut chlog) = chlog_file.get()?
                        && let Some(most_recent) = chlog.sections.first_mut()
                    {
                        log::info!("Updating CHANGELOG.md for {rname} ...");
                        let prefix = format!("- Increase `{name}` dependency to ");
                        let mut new_content = String::with_capacity(most_recent.content.len());
                        let mut changed = false;
                        for ln in most_recent.content.lines() {
                            if !changed && ln.starts_with(&prefix) {
                                let _ = writeln!(&mut new_content, "{prefix}`{version}`");
                                changed = true;
                            } else {
                                let _ = writeln!(&mut new_content, "{ln}");
                            }
                        }
                        if !changed {
                            let _ = writeln!(&mut new_content, "{prefix}`{version}`");
                        }
                        most_recent.content = new_content;
                        chlog_file.set(chlog)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn uses_prerelease(req: &VersionReq) -> bool {
    req.comparators
        .iter()
        .any(|c| c.op == Op::Caret && !c.pre.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use assert_fs::{TempDir, fixture::ChildPath, prelude::*};
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
                .set_cargo_version(&Version::new(1, 2, 3))
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
            let tpkg = TestPackage::new(
                "package = { name = \"foobar\", version = \"0.1.0\", edition = \"2021\" }\ndependencies = {}\n",
            );
            tpkg.package
                .set_cargo_version(&Version::new(1, 2, 3))
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
                .set_cargo_version(&Version::new(1, 2, 3))
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
            let changed = tpkg
                .package
                .set_dependency_version("quux", "1.2.3", true)
                .unwrap();
            assert_eq!(changed, ["dependencies"]);
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
            let changed = tpkg
                .package
                .set_dependency_version("glarch", "42.0", true)
                .unwrap();
            assert_eq!(changed, ["dev-dependencies"]);
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
            let changed = tpkg
                .package
                .set_dependency_version("glarch", "42.0", true)
                .unwrap();
            assert_eq!(changed, ["build-dependencies"]);
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
            let changed = tpkg
                .package
                .set_dependency_version("glarch", "42.0", true)
                .unwrap();
            assert_eq!(
                changed,
                ["dependencies", "dev-dependencies", "build-dependencies"]
            );
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
            let changed = tpkg
                .package
                .set_dependency_version("quux", "1.2.3", true)
                .unwrap();
            assert_eq!(changed, ["dependencies"]);
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
        fn inline_table_dep_no_create() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = { version = "0.1.0", default-features = false }
            "#});
            let changed = tpkg
                .package
                .set_dependency_version("quux", "1.2.3", false)
                .unwrap();
            assert_eq!(changed, ["dependencies"]);
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
            let changed = tpkg
                .package
                .set_dependency_version("quux", "1.2.3", true)
                .unwrap();
            assert_eq!(changed, ["dependencies"]);
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
        fn inline_table_dep_no_version_no_create() {
            let tpkg = TestPackage::new(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = { path = "../quux", default-features = false }
            "#});
            let changed = tpkg
                .package
                .set_dependency_version("quux", "1.2.3", false)
                .unwrap();
            assert!(changed.is_empty());
            tpkg.manifest.assert(indoc! {r#"
                [package]
                name = "foobar"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                quux = { path = "../quux", default-features = false }
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
            let changed = tpkg
                .package
                .set_dependency_version("quux", "1.2.3", true)
                .unwrap();
            assert_eq!(changed, ["dependencies"]);
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
            let changed = tpkg
                .package
                .set_dependency_version("quux", "1.2.3", true)
                .unwrap();
            assert_eq!(changed, ["dependencies"]);
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

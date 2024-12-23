use super::package::Package;
use super::util::{locate_project, LocateError};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackageSet {
    packages: Vec<Package>,
}

impl PackageSet {
    pub(crate) fn new(packages: Vec<Package>) -> PackageSet {
        PackageSet { packages }
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, Package> {
        self.packages.iter()
    }

    pub(crate) fn get(&self, name: Option<&str>) -> anyhow::Result<&Package> {
        match name {
            Some(name) => self.package_by_name(name).ok_or_else(|| {
                anyhow::anyhow!("No package named {name:?} found in current project")
            }),
            None => self
                .current_package()?
                .ok_or_else(|| anyhow::anyhow!("Not currently located in a package")),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn root_package(&self) -> Option<&Package> {
        self.packages.iter().find(|p| p.is_root_package())
    }

    pub(crate) fn into_root_package(self) -> Option<Package> {
        self.packages.into_iter().find(Package::is_root_package)
    }

    pub(crate) fn package_by_name(&self, name: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.name() == name)
    }

    #[allow(dead_code)]
    pub(crate) fn into_package_by_name(self, name: &str) -> Option<Package> {
        self.packages.into_iter().find(|p| p.name() == name)
    }

    pub(crate) fn package_by_manifest_path(&self, manifest_path: &Path) -> Option<&Package> {
        self.packages
            .iter()
            .find(|p| p.manifest_path() == manifest_path)
    }

    pub(crate) fn into_package_by_manifest_path(self, manifest_path: &Path) -> Option<Package> {
        self.packages
            .into_iter()
            .find(|p| p.manifest_path() == manifest_path)
    }

    pub(crate) fn current_package(&self) -> Result<Option<&Package>, LocateError> {
        locate_project(false).map(|path| self.package_by_manifest_path(&path))
    }

    pub(crate) fn into_current_package(self) -> Result<Option<Package>, LocateError> {
        locate_project(false).map(|path| self.into_package_by_manifest_path(&path))
    }
}

impl IntoIterator for PackageSet {
    type Item = Package;
    type IntoIter = std::vec::IntoIter<Package>;

    fn into_iter(self) -> Self::IntoIter {
        self.packages.into_iter()
    }
}

impl<'a> IntoIterator for &'a PackageSet {
    type Item = &'a Package;
    type IntoIter = std::slice::Iter<'a, Package>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

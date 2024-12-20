use crate::package::Package;
use crate::util::{locate_project, LocateError};
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

    #[allow(dead_code)]
    pub(crate) fn root_package(&self) -> Option<&Package> {
        self.packages.iter().find(|p| p.is_root_package())
    }

    #[allow(dead_code)]
    pub(crate) fn into_root_package(self) -> Option<Package> {
        self.packages.into_iter().find(Package::is_root_package)
    }

    #[allow(dead_code)]
    pub(crate) fn package_by_name(&self, name: &str) -> Option<&Package> {
        self.packages.iter().find(|p| p.name() == name)
    }

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

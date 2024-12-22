#![cfg(test)]
use fs_err::{read_dir, read_to_string};
use similar::udiff::unified_diff;
use similar::Algorithm;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::FileType;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CmpDirtrees {
    left: PathBuf,
    right: PathBuf,
    exclude: HashSet<OsString>,
}

impl CmpDirtrees {
    pub(crate) fn new<P: AsRef<Path>, Q: AsRef<Path>>(left: P, right: Q) -> Self {
        CmpDirtrees {
            left: left.as_ref().into(),
            right: right.as_ref().into(),
            exclude: HashSet::new(),
        }
    }

    pub(crate) fn exclude<I, S>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.exclude = iter.into_iter().map(Into::into).collect();
        self
    }

    pub(crate) fn assert_eq(self) {
        assert!(
            self.check(&self.left, &self.right).unwrap(),
            "Directory trees {} and {} differ!",
            self.left.display(),
            self.right.display()
        );
    }

    fn left_pathname(&self, path: &Path) -> String {
        match path.strip_prefix(&self.left) {
            Ok(p) => Path::new("left").join(p).to_string_lossy().into_owned(),
            Err(_) => path.to_string_lossy().into_owned(),
        }
    }

    fn right_pathname(&self, path: &Path) -> String {
        match path.strip_prefix(&self.right) {
            Ok(p) => Path::new("right").join(p).to_string_lossy().into_owned(),
            Err(_) => path.to_string_lossy().into_owned(),
        }
    }

    fn check(&self, left: &Path, right: &Path) -> anyhow::Result<bool> {
        let left_entries = self.direntries(left)?;
        let mut right_entries = self.direntries(right)?;
        let mut ok = true;
        for (fname, ftype) in left_entries {
            let left_path = left.join(&fname);
            let right_path = right.join(&fname);
            let left_pathname = self.left_pathname(&left_path);
            let right_pathname = self.right_pathname(&right_path);
            match right_entries.remove(&fname) {
                Some(rt) if ftype == rt => {
                    if ftype.is_file() {
                        let left_text = read_to_string(&left_path)?;
                        let right_text = read_to_string(&right_path)?;
                        if left_text != right_text {
                            eprint!(
                                "{}",
                                unified_diff(
                                    Algorithm::Myers,
                                    &left_text,
                                    &right_text,
                                    3,
                                    Some((&left_pathname, &right_pathname))
                                )
                            );
                            ok = false;
                        }
                    } else if ftype.is_dir() {
                        if !self.check(&left_path, &right_path)? {
                            ok = false;
                        }
                    } else {
                        eprintln!("Path {left_pathname} has unexpected file type {ftype:?}");
                    }
                }
                Some(rt) => {
                    eprintln!(
                        "Type mismatch: {left_pathname} = {ftype:?}; {right_pathname} = {rt:?}"
                    );
                    ok = false;
                }
                None => {
                    eprintln!(
                        "Dir entry \"{}\" exists in {} but not in {}",
                        fname.to_string_lossy(),
                        self.left_pathname(left),
                        self.right_pathname(right),
                    );
                    ok = false;
                }
            }
        }
        for fname in right_entries.into_keys() {
            eprintln!(
                "Dir entry \"{}\" exists in {} but not in {}",
                fname.to_string_lossy(),
                self.right_pathname(right),
                self.left_pathname(left),
            );
            ok = false;
        }
        Ok(ok)
    }

    fn direntries(&self, dirpath: &Path) -> anyhow::Result<HashMap<OsString, FileType>> {
        let mut entries = HashMap::new();
        for entry in read_dir(dirpath)? {
            let entry = entry?;
            let fname = entry.file_name();
            if !self.exclude.contains(&fname) {
                let ftype = entry.file_type()?;
                entries.insert(fname, ftype);
            }
        }
        Ok(entries)
    }
}

pub(crate) fn unzip<P: Into<PathBuf>, Q: AsRef<Path>>(zippath: P, outdir: Q) -> anyhow::Result<()> {
    let fp = fs_err::File::open(zippath)?;
    let mut zip = zip::ZipArchive::new(fp)?;
    zip.extract(outdir)?;
    Ok(())
}

pub(crate) fn opt_subdir<'a>(path: &'a Path, subdir: Option<&str>) -> std::borrow::Cow<'a, Path> {
    match subdir {
        Some(sub) => path.join(sub).into(),
        None => path.into(),
    }
}

use anyhow::Context;
use similar::udiff::unified_diff;
use similar::Algorithm;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{read_dir, read_to_string, FileType};
use std::path::Path;

pub fn assert_dirtrees_eq(left: &Path, right: &Path) {
    if !check_dirtrees(left, right).unwrap() {
        panic!(
            "Directory trees {} and {} differ!",
            left.display(),
            right.display()
        );
    }
}

fn check_dirtrees(left: &Path, right: &Path) -> anyhow::Result<bool> {
    let left_entries = direntries(left)?;
    let mut right_entries = direntries(right)?;
    let mut ok = true;
    for (fname, ftype) in left_entries {
        let left_path = left.join(&fname);
        let right_path = right.join(&fname);
        match right_entries.remove(&fname) {
            Some(rt) if ftype == rt => {
                if ftype.is_file() {
                    let left_text = read_to_string(&left_path).with_context(|| {
                        format!("Failed to read text from {}", left_path.display())
                    })?;
                    let right_text = read_to_string(&right_path).with_context(|| {
                        format!("Failed to read text from {}", right_path.display())
                    })?;
                    if left_text != right_text {
                        // TODO: Display the paths in the diff header as
                        // `left/` and `right/` followed by the paths relative
                        // to the original paths passed to
                        // `assert_dirtrees_eq()`
                        let left_name = left_path.to_string_lossy();
                        let right_name = right_path.to_string_lossy();
                        eprint!(
                            "{}",
                            unified_diff(
                                Algorithm::Myers,
                                &left_text,
                                &right_text,
                                3,
                                Some((left_name.borrow(), right_name.borrow()))
                            )
                        );
                        ok = false;
                    }
                } else if ftype.is_dir() {
                    if !check_dirtrees(&left_path, &right_path)? {
                        ok = false;
                    }
                } else {
                    eprintln!(
                        "Path {} has unexpected file type {ftype:?}",
                        left_path.display()
                    );
                }
            }
            Some(rt) => {
                eprintln!(
                    "Type mismatch: {} = {ftype:?}; {} = {rt:?}",
                    left_path.display(),
                    right_path.display()
                );
                ok = false;
            }
            None => {
                eprintln!(
                    "Dir entry \"{}\" exists in {} but not in {}",
                    fname.to_string_lossy(),
                    left.display(),
                    right.display(),
                );
                ok = false;
            }
        }
    }
    for fname in right_entries.into_keys() {
        eprintln!(
            "Dir entry \"{}\" exists in {} but not in {}",
            fname.to_string_lossy(),
            right.display(),
            left.display(),
        );
        ok = false;
    }
    Ok(ok)
}

fn direntries(dirpath: &Path) -> anyhow::Result<HashMap<OsString, FileType>> {
    let mut entries = HashMap::new();
    for entry in read_dir(dirpath)
        .with_context(|| format!("Failed to read directory {}", dirpath.display()))?
    {
        let entry = entry
            .with_context(|| format!("Error getting directory entry from {}", dirpath.display()))?;
        let fname = entry.file_name();
        let ftype = entry
            .file_type()
            .with_context(|| format!("Error getting file type of {}", entry.path().display()))?;
        entries.insert(fname, ftype);
    }
    Ok(entries)
}

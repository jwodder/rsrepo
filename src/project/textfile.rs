use anyhow::Context;
use fs_err::{File, read_to_string};
use std::io::{ErrorKind, Write};
use std::marker::PhantomData;
use std::path::Path;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextFile<'a, T> {
    dirpath: &'a Path,
    filename: &'static str,
    _type: PhantomData<T>,
}

impl<'a, T> TextFile<'a, T> {
    pub(crate) fn new(dirpath: &'a Path, filename: &'static str) -> Self {
        TextFile {
            dirpath,
            filename,
            _type: PhantomData,
        }
    }

    pub(crate) fn exists(&self) -> bool {
        self.dirpath.join(self.filename).exists()
    }

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

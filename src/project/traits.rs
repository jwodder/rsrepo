use super::textfile::TextFile;
use crate::readme::Readme;

pub(crate) trait HasReadme {
    fn readme(&self) -> TextFile<'_, Readme>;
}

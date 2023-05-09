use chrono::Datelike;
use std::iter::FusedIterator;

pub(crate) fn this_year() -> i32 {
    chrono::Local::now().year()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringLines {
    content: String,
}

impl StringLines {
    pub fn new(content: String) -> StringLines {
        StringLines { content }
    }
}

impl Iterator for StringLines {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        if self.content.is_empty() {
            return None;
        }
        let i = self.content.find('\n').unwrap_or(self.content.len() - 1);
        let mut line = self.content.drain(0..=i).collect::<String>();
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        Some(line)
    }
}

impl FusedIterator for StringLines {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_lines() {
        let mut iter = StringLines::new("foo\r\nbar\n\nbaz\n".into());
        assert_eq!(iter.next().unwrap(), "foo");
        assert_eq!(iter.next().unwrap(), "bar");
        assert_eq!(iter.next().unwrap(), "");
        assert_eq!(iter.next().unwrap(), "baz");
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn string_lines_no_final_newline() {
        let mut iter = StringLines::new("foo\nbar\n\r\nbaz".into());
        assert_eq!(iter.next().unwrap(), "foo");
        assert_eq!(iter.next().unwrap(), "bar");
        assert_eq!(iter.next().unwrap(), "");
        assert_eq!(iter.next().unwrap(), "baz");
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }
}

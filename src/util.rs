use chrono::Datelike;

pub(crate) fn this_year() -> i32 {
    chrono::Local::now().year()
}

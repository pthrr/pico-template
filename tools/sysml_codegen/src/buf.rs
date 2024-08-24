//! Append formatted text to a `String` without `push_str(&format!(...))`.

use std::fmt::Write;

pub fn append(out: &mut String, args: std::fmt::Arguments<'_>) {
    out.write_fmt(args).unwrap();
}

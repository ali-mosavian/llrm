use std::borrow::Cow;

use crate::Dialect;
use crate::front::lines::logical_lines;

fn joined(
    source: &str,
    dialect: Dialect,
) -> Vec<(usize, String)> {
    logical_lines(source, dialect).into_iter().map(|(line, text)| (line, text.into_owned())).collect()
}

#[test]
fn a_continued_line_is_joined_and_numbered_from_its_first_physical_line() {
    let lines = joined("PRINT 1 _ ignored\n+ 2\nPRINT 3\n", Dialect::Pds71);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0, 1);
    assert!(lines[0].1.starts_with("PRINT 1") && lines[0].1.ends_with("+ 2"), "{lines:?}");
    assert!(!lines[0].1.contains("ignored"), "{lines:?}");
    assert_eq!(lines[1], (3, "PRINT 3".to_string()));
}

#[test]
fn a_line_that_continues_nothing_is_borrowed() {
    let lines = logical_lines("PRINT 1\n", Dialect::Pds71);
    assert!(matches!(lines[0].1, Cow::Borrowed("PRINT 1")));
}

#[test]
fn an_underscore_in_a_string_or_a_comment_continues_nothing() {
    let lines = joined("PRINT \"a _\"\n' note _\nPRINT 2\n", Dialect::Pds71);
    assert_eq!(lines.iter().map(|(line, _)| *line).collect::<Vec<_>>(), [1, 2, 3]);
}

#[test]
fn an_underscore_after_a_letter_is_part_of_a_name_in_vbdos_only() {
    let source = "x_\nPRINT 2\n";
    assert_eq!(joined(source, Dialect::VbDos).len(), 2);
    assert_eq!(joined(source, Dialect::Pds71).len(), 1);
}

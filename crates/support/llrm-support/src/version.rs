//! `--version` of a binary: the commit it was compiled from and whether that
//! tree had changes.
//!
//! The line is `NAME SHA` or `NAME SHA+dirty`, read by `tools/stamp.py`. It is
//! printed and checked, never hashed into anything the compiler writes.

/// The line `--version` prints.
pub fn line(
    name: &str,
    sha: &str,
    dirty: &str,
) -> String {
    let flag = if dirty == "1" { "+dirty" } else { "" };
    format!("{name} {sha}{flag}")
}

/// Prints the line and returns true if the arguments are `--version`.
pub fn asked(
    argv: &[String],
    name: &str,
    sha: &str,
    dirty: &str,
) -> bool {
    let asked = argv.len() == 1 && argv[0] == "--version";
    if asked {
        println!("{}", line(name, sha, dirty));
    }
    asked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dirty_tree_shows_in_the_line() {
        assert_eq!(line("llrm-c", "abc", "0"), "llrm-c abc");
        assert_eq!(line("llrm-c", "abc", "1"), "llrm-c abc+dirty");
    }

    #[test]
    fn only_a_lone_version_flag_asks() {
        assert!(asked(&["--version".to_owned()], "llrm-c", "abc", "0"));
        assert!(!asked(&["--version".to_owned(), "x.c".to_owned()], "llrm-c", "abc", "0"));
    }
}

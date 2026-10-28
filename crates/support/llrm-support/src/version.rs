//! `--version` of a binary: the commit it was compiled from and whether that
//! tree had changes.
//!
//! The line is `NAME SHA` with `+dirty` (the tree had changes) and `+incremental`
//! (an incremental build: its instruction counts are not the cost gate's) after
//! it, read by `tools/stamp.py`. It is printed and checked, never hashed into
//! anything the compiler writes.

/// The line `--version` prints.
pub fn line(
    name: &str,
    sha: &str,
    dirty: &str,
    incremental: &str,
) -> String {
    let dirty = if dirty == "1" { "+dirty" } else { "" };
    let incremental = if incremental == "1" { "+incremental" } else { "" };
    format!("{name} {sha}{dirty}{incremental}")
}

/// Prints the line and returns true if the arguments are `--version`.
pub fn asked(
    argv: &[String],
    name: &str,
    sha: &str,
    dirty: &str,
    incremental: &str,
) -> bool {
    let asked = argv.len() == 1 && argv[0] == "--version";
    if asked {
        println!("{}", line(name, sha, dirty, incremental));
    }
    asked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dirty_tree_shows_in_the_line() {
        assert_eq!(line("llrm-c", "abc", "0", "0"), "llrm-c abc");
        assert_eq!(line("llrm-c", "abc", "1", "0"), "llrm-c abc+dirty");
    }

    /// The object writer ran 13% more instructions in an incremental build of the same commit (9.5 -> 10.9 Minstr, #5744): the stamp says which.
    #[test]
    fn an_incremental_build_shows_in_the_line() {
        assert_eq!(line("llrm-llc", "abc", "0", "1"), "llrm-llc abc+incremental");
        assert_eq!(line("llrm-llc", "abc", "1", "1"), "llrm-llc abc+dirty+incremental");
    }

    #[test]
    fn only_a_lone_version_flag_asks() {
        assert!(asked(&["--version".to_owned()], "llrm-c", "abc", "0", "0"));
        assert!(!asked(&["--version".to_owned(), "x.c".to_owned()], "llrm-c", "abc", "0", "0"));
    }
}

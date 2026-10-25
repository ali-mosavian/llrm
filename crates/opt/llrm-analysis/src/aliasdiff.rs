//! The alias stack's answers, recorded and compared: the gate for any change
//! to what the stack is built from.
//!
//! # How it works
//! `alias_pairs` (bin) prints, per function of a module dump, one row per
//! memory access `i` holding the verdict of the stack for the pair `(i, j)`
//! for every later access `j` (`N` no alias, `M` may, `P` partial, `U` must),
//! and one row per call holding, for every access, what the call may do to its
//! location (`0` nothing, `R` read, `W` write, `B` both). A row is keyed by
//! the function and the position of the access or call in the function's
//! walk, which is the same for two builds fed one dump.
//!
//! [`compare`] reads two such recordings. A pair that was `N` and is not, or
//! a call that touched a location and no longer does, is *lost* (precision;
//! counted). A pair that was not `N` and is, or a call that read or wrote a
//! location and now does not, is *gained* (a new proof): it fails, since a
//! change that only moves the analyses' inputs may remove answers, never
//! invent them. A forged segment pointer answered `N` by the new stack is the
//! `b$seg` case this guards.
//!
//! # Example
//! `a_pair_that_becomes_no_alias_is_gained` in `aliasdiff_tests.rs`.
//!
//! # See also
//! the alias-stack plan (stage S0.5).

/// What [`compare`] found.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Report {
    /// Pairs and call locations with the same answer.
    pub same: usize,
    /// Answers that lost a proof.
    pub lost: usize,
    /// Answers that changed otherwise (may to partial or must, read to write).
    pub changed: usize,
    /// Answers that gained a proof: each is a failure.
    pub gained: Vec<String>,
    /// Rows present in one recording only.
    pub unmatched: Vec<String>,
}

impl Report {
    pub fn passes(&self) -> bool {
        self.gained.is_empty() && self.unmatched.is_empty()
    }
}

/// A verdict's strength: higher is more known.
fn proof(
    kind: char,
    verdict: char,
) -> u8 {
    match (kind, verdict) {
        ('A', 'N') | ('C', '0') => 2,
        ('C', 'R' | 'W') => 1,
        _ => 0,
    }
}

/// The two recordings compared row by row.
pub fn compare(
    old: &str,
    new: &str,
) -> Report {
    let rows = |text: &str| -> std::collections::BTreeMap<String, String> {
        text.lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let kind = parts.next()?;
                let key = format!("{kind} {} {}", parts.next()?, parts.next()?);
                Some((key, parts.next().unwrap_or("").to_owned()))
            })
            .collect()
    };
    let (before, after) = (rows(old), rows(new));
    let mut report = Report::default();
    for (key, one) in &before {
        let Some(two) = after.get(key) else {
            report.unmatched.push(format!("{key}: only in the old recording"));
            continue;
        };
        if one.len() != two.len() {
            report.unmatched.push(format!("{key}: {} answers against {}", one.len(), two.len()));
            continue;
        }
        let kind = key.chars().next().unwrap_or('A');
        for (at, (a, b)) in one.chars().zip(two.chars()).enumerate() {
            if a == b {
                report.same += 1;
            } else if proof(kind, b) > proof(kind, a) {
                report.gained.push(format!("{key} [{at}]: {a} -> {b}"));
            } else if proof(kind, b) < proof(kind, a) {
                report.lost += 1;
            } else {
                report.changed += 1;
            }
        }
    }
    for key in after.keys().filter(|key| !before.contains_key(*key)) {
        report.unmatched.push(format!("{key}: only in the new recording"));
    }
    report
}

#[cfg(test)]
#[path = "aliasdiff_tests.rs"]
mod tests;

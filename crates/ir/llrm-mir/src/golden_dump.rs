//! The hash the `tools/llvmport/*_dump.cpp` drivers print: each call of an
//! operation hashes its answers, a line of a golden file is the hash of all
//! the calls of one operation and how many different answers they gave.

use std::collections::BTreeSet;

pub(crate) struct Dump {
    hash: u64,
    current: u64,
    distinct: BTreeSet<u64>,
    pub(crate) lines: Vec<String>,
}

const OFFSET: u64 = 0xcbf29ce484222325;

fn fnv(
    state: &mut u64,
    v: u64,
) {
    for i in 0..8 {
        *state ^= (v >> (8 * i)) & 0xff;
        *state = state.wrapping_mul(0x100000001b3);
    }
}

impl Dump {
    pub(crate) fn new() -> Self {
        Self { hash: OFFSET, current: OFFSET, distinct: BTreeSet::new(), lines: Vec::new() }
    }

    pub(crate) fn begin(&mut self) {
        self.current = OFFSET;
    }

    pub(crate) fn add(
        &mut self,
        v: u64,
    ) {
        fnv(&mut self.current, v);
    }

    /// A string: its length, then its bytes.
    pub(crate) fn add_str(
        &mut self,
        text: &str,
    ) {
        self.add(text.len() as u64);
        for byte in text.bytes() {
            self.add(u64::from(byte));
        }
    }

    pub(crate) fn end(&mut self) {
        self.distinct.insert(self.current);
        fnv(&mut self.hash, self.current);
    }

    /// Ends the operation: its line, `name [format] hash distinct`.
    pub(crate) fn line(
        &mut self,
        name: &str,
        format: Option<&str>,
    ) {
        let format = format.map_or(String::new(), |one| format!(" {one}"));
        self.lines.push(format!("{name}{format} {:016x} {}", self.hash, self.distinct.len()));
        self.hash = OFFSET;
        self.distinct.clear();
    }
}

/// The lines of `ours` and of LLVM's file `golden` agree one by one.
pub(crate) fn assert_agree(
    ours: &[String],
    golden: &str,
) {
    let golden: Vec<&str> = golden.lines().collect();
    assert_eq!(ours.len(), golden.len(), "{} operations here, {} in LLVM's file", ours.len(), golden.len());
    let differing: Vec<String> =
        ours.iter().zip(&golden).filter(|(a, b)| a.as_str() != **b).map(|(a, b)| format!("{a} (LLVM: {b})")).collect();
    assert!(differing.is_empty(), "{} differ:\n{}", differing.len(), differing.join("\n"));
}

/// An oracle that can only say one thing proves nothing: every line of the
/// file but `except` has at least two different answers.
pub(crate) fn assert_many_answers(
    golden: &str,
    except: &[&str],
    least_lines: usize,
) {
    let mut constant = Vec::new();
    let mut count = 0;
    for line in golden.lines() {
        let words: Vec<&str> = line.split(' ').collect();
        let distinct: usize = words[words.len() - 1].parse().expect("a count");
        count += 1;
        if distinct < 2 && !except.contains(&words[0]) {
            constant.push(line);
        }
    }
    assert!(count >= least_lines, "{count} lines");
    assert!(constant.is_empty(), "these always answer the same thing: {constant:?}");
}

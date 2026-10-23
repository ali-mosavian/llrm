//! Which instruction selector runs, and the row each function leaves for
//! `tools/pairs/isel_shadow.py`.
//!
//! # How it works
//! `--isel=old` (the default) is the MIR selector of `isel`; `--isel=dag` is
//! the SelectionDAG translation ([selectiondag-plan], group sdag-d). The DAG
//! path refuses every function until its first end-to-end piece lands, so the
//! harness counts refusals as it counts any other difference.
//!
//! `LLRM_DEBUG=isel` prints one row per function once the route is chosen:
//! `[isel] NAME selector=old instructions=21 copies=3 executed=40 memory=12`.
//! `executed` and `memory` are `executed::executed`'s estimate (`-` where the
//! body has no finite one); `copies` are register-to-register `mov`s.
//! `LLRM_ISEL_REPORT=FILE` appends `NAME dag|old` per function: who selected it.
//! `ISEL_DUMP=1` also prints `=== ISel old` (or `dag`) right after selection,
//! before any machine phase.
//!
//! # Example
//! A function of 3 instructions holding one register copy:
//!
//! ```text
//! [isel] id selector=old instructions=3 copies=1 executed=3 memory=0
//! ```
//!
//! From `LLRM_DEBUG=isel llrm-c -m32 -O2` of `int id(int x) { return x; }`.
//!
//! # Used by
//! `assemble::staged` (the choice and the dump), `assemble::machined` (the
//! row), `driver::flags` (`--isel=`).
//!
//! [selectiondag-plan]:
//! https://github.com/42labs/llrm/blob/main/docs/working/selectiondag-plan.md

use crate::backend::executed;
use crate::model::ir::Loc;
use crate::model::lir::LirBody;

/// The instruction selector a compile uses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Selector {
    /// `isel`: one forward walk over MIR.
    #[default]
    Old,
    /// The SelectionDAG translation, `selectiondag-plan.md`.
    Dag,
    /// The DAG path where it can select, else the old selector, each refusal
    /// reported (`[isel] NAME refused: WHY`): what the harness measures.
    Shadow,
}

impl Selector {
    /// The name `--isel=` takes and the dump prints.
    pub fn name(self) -> &'static str {
        match self {
            Self::Old => "old",
            Self::Dag => "dag",
            Self::Shadow => "shadow",
        }
    }

    /// The selector `--isel=<text>` names.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "old" => Ok(Self::Old),
            "dag" => Ok(Self::Dag),
            "shadow" => Ok(Self::Shadow),
            _ => Err(format!("unknown --isel={text}; choose old, dag or shadow")),
        }
    }
}

/// `LLRM_ISEL_REPORT=FILE`: one line `NAME dag` or `NAME old` appended per
/// function, for the one that selected it (a shadow run's refused function is
/// `old`). A run that reports a selection percentage or an execution count
/// proves with it which selector compiled what: the flag alone does not.
pub fn record(
    name: &str,
    by: Selector,
) {
    if let Some(path) = std::env::var_os("LLRM_ISEL_REPORT") {
        record_to(std::path::Path::new(&path), name, by);
    }
}

fn record_to(
    path: &std::path::Path,
    name: &str,
    by: Selector,
) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{name} {}", by.name());
    }
}

#[cfg(test)]
pub(super) fn recorded_to(
    path: &std::path::Path,
    name: &str,
    by: Selector,
) {
    record_to(path, name, by);
}

/// The `isel` channel's row for `body`, finished by `selector`.
pub fn row(
    body: &LirBody,
    selector: Selector,
) -> String {
    let insns = body.insns();
    let copies = insns.iter().filter(|one| one.what.as_ref().is_some_and(is_copy)).count();
    let (executed, memory) = match executed::executed(body) {
        Some(done) => (format!("{:.0}", done.instructions), format!("{:.0}", done.memory)),
        None => ("-".to_owned(), "-".to_owned()),
    };
    format!(
        "{} selector={} instructions={} copies={copies} executed={executed} memory={memory}",
        body.name,
        selector.name(),
        insns.len()
    )
}

fn is_copy(what: &llrm_lir::Semantics) -> bool {
    let register = |one: &Loc| matches!(one, Loc::Reg(_) | Loc::Held(_));
    what.name.as_deref() == Some("mov") && what.dests.iter().all(register) && what.sources.iter().all(register)
}

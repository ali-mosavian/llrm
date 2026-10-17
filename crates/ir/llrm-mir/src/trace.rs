//! A record of what a compile did, for replaying it against another engine.
//!
//! # How it works
//! `LLRM_TRACE=path` (read once; `llrm-c` calls [`start`] with the source's
//! name and level) appends one JSON object a line to `path` when the compile
//! ends, in one write, so compiles that run side by side do not interleave
//! their lines; every line carries the `file` it was made for
//! (`<source>@<level>`) and a `seq`, one count for the compile, that orders the
//! events of a function (an ask is numbered when it starts and written when it
//! ends, so a compute comes after the asks it made). Off, each call site costs
//! one relaxed load.
//!
//! ```text
//! {"file":"d_faces.c@O2","seq":7,"ev":"run","pass":"fold","stage":"r01-fold","fn":123,"id":4,"insts":812,"blocks":97}
//! {"file":"d_faces.c@O2","seq":8,"ev":"edit","fn":123,"kind":"Rewritten","block":12,"mem":false,"ptr":false,"term":false,"cfg":false,"params":false,"value":true}
//! {"file":"d_faces.c@O2","seq":9,"ev":"ask","pass":"fold","fn":123,"analysis":"through-memory","how":"recomputed","depth":0,"instr":1523411,"insts":812,"blocks":97}
//! {"file":"d_faces.c@O2","seq":10,"ev":"clone","from":123,"to":124}
//! ```
//! - `run`: a function pass ran over a body of `insts` instructions in `blocks`
//!   blocks; `fn` is the body's lineage (the same for its forks), `id` its
//!   global where it has one.
//! - `edit`: one change-log entry of `fn`, written at the next ask or the end
//!   of the run, with its kind, the block it is in, and what it touches: `mem`
//!   a load, store or call, `ptr` the pointer operand of one, `term` a
//!   terminator, `cfg` blocks made or erased, `params` the parameters, `value`
//!   the operands, opcode or flags of an instruction.
//! - `ask`: the manager was asked for `analysis` by the pass running (empty
//!   before any), answered `how`: `hit` (held), `kept` (held past the edits, by
//!   their soundness or by the analysis's own footprint), `updated` (brought up
//!   to date from the log) or `recomputed`; `instr` the instructions it took
//!   (counted through the asks it made, which are numbered after it at `depth`
//!   + 1), `insts` and `blocks` the body's size at the ask.
//! - `clone` and `fork`: a function was copied (`to` is another lineage) or
//!   forked (the same one). `adopt`, `discard`, `checkpoint` and `rollback` are
//!   written by [`lifecycle`], which the code that does them calls.
//!
//! # Used by
//! The pass driver (`llrm-transforms::pipeline`, `passes`) and the manager's
//! lookups (`Analyses::get`).

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::module::{Change, Function, Mark};
use crate::opcode::Opcode;

static ON: AtomicBool = AtomicBool::new(false);
static PATH: OnceLock<String> = OnceLock::new();

static SEQ: AtomicU64 = AtomicU64::new(0);
static WORK: OnceLock<fn() -> u64> = OnceLock::new();

thread_local! {
    static FILE: RefCell<String> = const { RefCell::new(String::new()) };
    static LINES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// The pass running, and the body it runs over.
    static CURRENT: Cell<(&'static str, u64)> = const { Cell::new(("", 0)) };
    /// How many asks are in progress, the outermost 0.
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    /// The change-log position each body was last written to.
    static WRITTEN: RefCell<std::collections::HashMap<u64, Mark>> = RefCell::new(std::collections::HashMap::new());
}

/// Whether the trace is on.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// What counts the work an ask took (`llrm_support::debug::work`, which this
/// crate cannot name).
pub fn count_work_with(work: fn() -> u64) {
    let _ = WORK.set(work);
}

fn work() -> u64 {
    WORK.get().map_or(0, |work| work())
}

/// Starts tracing this compile of `file` into the path `LLRM_TRACE` names, if
/// it names one.
pub fn start(file: &str) {
    let Some(path) = std::env::var_os("LLRM_TRACE") else { return };
    let _ = PATH.set(path.to_string_lossy().into_owned());
    FILE.with(|name| *name.borrow_mut() = file.to_owned());
    ON.store(true, Ordering::Relaxed);
}

fn escaped(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn line(
    seq: u64,
    event: &str,
    rest: String,
) {
    let file = FILE.with(|name| escaped(&name.borrow()));
    LINES.with(|lines| {
        lines.borrow_mut().push(format!("{{\"file\":\"{file}\",\"seq\":{seq},\"ev\":\"{event}\",{rest}}}"))
    });
}

fn next() -> u64 {
    SEQ.fetch_add(1, Ordering::Relaxed)
}

fn size(function: &Function) -> (usize, usize) {
    let blocks = function.layout().len();
    (function.layout().iter().map(|&block| function.block(block).instructions().len()).sum(), blocks)
}

/// A function pass `pass` is about to run over `function`, in `stage`.
pub fn run(
    pass: &'static str,
    stage: &str,
    function: &Function,
    id: Option<u32>,
) {
    if !on() {
        return;
    }
    let fn_id = function.mark().lineage();
    CURRENT.with(|current| current.set((pass, fn_id)));
    WRITTEN.with(|written| written.borrow_mut().insert(fn_id, function.mark()));
    let (insts, blocks) = size(function);
    let id = id.map_or_else(|| "null".to_owned(), |one| one.to_string());
    line(
        next(),
        "run",
        format!(
            "\"pass\":\"{pass}\",\"stage\":\"{}\",\"fn\":{fn_id},\"id\":{id},\"insts\":{insts},\"blocks\":{blocks}",
            escaped(stage)
        ),
    );
}

/// Writes the changes `function` logged since the last write.
pub fn sync(function: &Function) {
    if !on() {
        return;
    }
    let mark = function.mark();
    let fn_id = mark.lineage();
    let from = WRITTEN.with(|written| written.borrow_mut().insert(fn_id, mark));
    let Some(changes) = from.and_then(|from| function.changes_since(from)) else { return };
    for change in changes {
        edit(function, fn_id, change);
    }
}

fn edit(
    function: &Function,
    fn_id: u64,
    change: &Change,
) {
    let inst = match change {
        Change::Inserted { inst, .. }
        | Change::Moved { inst, .. }
        | Change::Erased { inst, .. }
        | Change::Operand { inst, .. }
        | Change::Operands { inst, .. } => Some(*inst),
        Change::Rewritten(inst) | Change::Cloned { to: inst, .. } => Some(*inst),
        _ => None,
    };
    let block = match change {
        Change::Inserted { block, .. } | Change::Moved { block, .. } | Change::Erased { block, .. } => Some(block.0),
        Change::BlockCreated(block) | Change::BlockErased(block) => Some(block.0),
        _ => inst.and_then(|inst| function.parent(inst)).map(|block| block.0),
    };
    let opcode = inst.map(|inst| &function.instruction(inst).opcode);
    let memory = opcode.is_some_and(|op| {
        matches!(
            op,
            Opcode::Load { .. } | Opcode::Store { .. } | Opcode::Call(_) | Opcode::Invoke(_)
        )
    });
    let pointer = match (change, opcode) {
        (Change::Operand { index, .. }, Some(Opcode::Load { .. })) => *index == 0,
        (Change::Operand { index, .. }, Some(Opcode::Store { .. })) => *index == 1,
        (Change::Operands { .. } | Change::Rewritten(_), Some(Opcode::Load { .. } | Opcode::Store { .. })) => true,
        _ => false,
    };
    let terminator = opcode.is_some_and(Opcode::is_terminator);
    let blocks_changed = matches!(change, Change::BlockCreated(_) | Change::BlockErased(_));
    let value = matches!(
        change,
        Change::Rewritten(_) | Change::Operand { .. } | Change::Operands { .. } | Change::Substituted { .. }
    );
    line(
        next(),
        "edit",
        format!(
            "\"fn\":{fn_id},\"kind\":\"{}\",\"block\":{},\"mem\":{memory},\"ptr\":{pointer},\"term\":{terminator},\"cfg\":{blocks_changed},\"params\":{},\"value\":{value}",
            kind(change),
            block.map_or_else(|| "null".to_owned(), |one| one.to_string()),
            matches!(change, Change::Parameters)
        ),
    );
}

/// An ask in progress.
pub struct Ask {
    seq: u64,
    start: u64,
    depth: u32,
}

/// The manager is asked for `analysis` of `function` and has to answer it by
/// more than holding it.
pub fn ask_begin(function: &Function) -> Option<Ask> {
    if !on() {
        return None;
    }
    sync(function);
    let depth = DEPTH.with(|depth| depth.replace(depth.get() + 1));
    Some(Ask { seq: next(), start: work(), depth })
}

/// The ask `ask` was answered `how`.
pub fn ask_end(
    ask: Option<Ask>,
    analysis: &'static str,
    how: &str,
    function: &Function,
) {
    let Some(ask) = ask else { return };
    let instr = work().saturating_sub(ask.start);
    DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    emit_ask(ask.seq, analysis, how, ask.depth, instr, function);
}

/// The manager held `analysis` of `function`.
pub fn ask_held(
    analysis: &'static str,
    function: &Function,
) {
    if !on() {
        return;
    }
    sync(function);
    emit_ask(next(), analysis, "hit", DEPTH.with(Cell::get), 0, function);
}

fn emit_ask(
    seq: u64,
    analysis: &str,
    how: &str,
    depth: u32,
    instr: u64,
    function: &Function,
) {
    let (pass, fn_id) = CURRENT.with(Cell::get);
    let (insts, blocks) = size(function);
    line(
        seq,
        "ask",
        format!(
            "\"pass\":\"{pass}\",\"fn\":{fn_id},\"analysis\":\"{analysis}\",\"how\":\"{how}\",\"depth\":{depth},\"instr\":{instr},\"insts\":{insts},\"blocks\":{blocks}"
        ),
    );
}

/// The run of the pass over `function` is over: its last changes are written.
pub fn end(function: &Function) {
    if !on() {
        return;
    }
    sync(function);
    CURRENT.with(|current| current.set(("", 0)));
}

/// A body's life changed: `kind` is `clone`, `fork`, `adopt`, `discard`,
/// `checkpoint` or `rollback`, `from` and `to` the lineages before and after.
pub fn lifecycle(
    kind: &str,
    from: u64,
    to: u64,
) {
    if !on() {
        return;
    }
    line(next(), kind, format!("\"from\":{from},\"to\":{to}"));
}

/// The variant of `change`, by name.
fn kind(change: &Change) -> String {
    let text = format!("{change:?}");
    text.split(|one: char| !one.is_alphanumeric()).next().unwrap_or_default().to_owned()
}

/// Appends the lines of this compile to the trace, in one write.
pub fn flush() {
    if !on() {
        return;
    }
    let Some(path) = PATH.get() else { return };
    let text = LINES.with(|lines| lines.borrow_mut().drain(..).map(|one| one + "\n").collect::<String>());
    if let Ok(mut out) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = out.write_all(text.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a_change_is_named_by_its_variant() {
        assert_eq!(kind(&Change::Rewritten(crate::module::InstId(3))), "Rewritten");
    }

    #[test]
    fn test_a_name_with_a_quote_does_not_break_the_line() {
        assert_eq!(escaped("a\"b\\c"), "a\\\"b\\\\c");
    }
}

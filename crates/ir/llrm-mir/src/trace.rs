//! A record of what a compile did, for replaying it against another engine.
//!
//! # How it works
//! `LLRM_TRACE=path` (read once; `llrm-c` calls [`start`] with the source's
//! name) appends one JSON object a line to `path` when the compile ends, in one
//! write, so compiles that run side by side do not interleave their lines;
//! every line carries the `file` it was made for. Off, each call site costs one
//! relaxed load.
//!
//! ```text
//! {"file":"d_faces.c","ev":"run","pass":"fold","stage":"r01-fold","fn":123,"id":4,"insts":812,"blocks":97}
//! {"file":"d_faces.c","ev":"ask","pass":"fold","fn":123,"analysis":"through-memory","hit":false}
//! {"file":"d_faces.c","ev":"edits","pass":"fold","fn":123,"kinds":{"Rewritten":12,"Substituted":3}}
//! ```
//! - `run`: a function pass ran over a body of `insts` instructions in `blocks`
//!   blocks; `fn` is the body's lineage (the same for its forks), `id` its
//!   global where it has one.
//! - `ask`: the manager was asked for `analysis` by the pass running (empty
//!   before any); `hit` when it was held.
//! - `edits`: the change log entries the run made, by kind.
//!
//! # Used by
//! The pass driver (`llrm-transforms::pipeline`, `passes`) and the manager's
//! lookups (`counts::analysis`).

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::module::{Change, Function, Mark};

static ON: AtomicBool = AtomicBool::new(false);
static PATH: OnceLock<String> = OnceLock::new();

thread_local! {
    static FILE: RefCell<String> = const { RefCell::new(String::new()) };
    static LINES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// (pass, body lineage) the run in progress is of.
    static CURRENT: Cell<(&'static str, u64)> = const { Cell::new(("", 0)) };
}

/// Whether the trace is on.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
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
    event: &str,
    rest: String,
) {
    let file = FILE.with(|name| escaped(&name.borrow()));
    LINES.with(|lines| lines.borrow_mut().push(format!("{{\"file\":\"{file}\",\"ev\":\"{event}\",{rest}}}")));
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
    let blocks = function.layout().len();
    let insts: usize = function.layout().iter().map(|&block| function.block(block).instructions().len()).sum();
    let id = id.map_or_else(|| "null".to_owned(), |one| one.to_string());
    line(
        "run",
        format!(
            "\"pass\":\"{pass}\",\"stage\":\"{}\",\"fn\":{fn_id},\"id\":{id},\"insts\":{insts},\"blocks\":{blocks}",
            escaped(stage)
        ),
    );
}

/// The manager was asked for `analysis`.
pub fn ask(
    analysis: &'static str,
    hit: bool,
) {
    if !on() {
        return;
    }
    let (pass, fn_id) = CURRENT.with(Cell::get);
    line("ask", format!("\"pass\":\"{pass}\",\"fn\":{fn_id},\"analysis\":\"{analysis}\",\"hit\":{hit}"));
}

/// The run of `pass` over `function` made the changes since `before`.
pub fn edits(
    pass: &'static str,
    function: &Function,
    before: Mark,
) {
    if !on() {
        return;
    }
    let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for change in function.changes_since(before).unwrap_or_default() {
        *kinds.entry(kind(change)).or_default() += 1;
    }
    let kinds = kinds.iter().map(|(name, count)| format!("\"{name}\":{count}")).collect::<Vec<_>>().join(",");
    line("edits", format!("\"pass\":\"{pass}\",\"fn\":{},\"kinds\":{{{kinds}}}", function.mark().lineage()));
    CURRENT.with(|current| current.set(("", 0)));
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

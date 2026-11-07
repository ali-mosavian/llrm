//! A function tuned for size calls `__mulsi3` for `x * 3` (the shifts and an
//! add are longer than the call). When every such call in the module is one of
//! these, the routine itself is the cost the call hides: it is linked once,
//! for them.
//!
//! DEVIATE (6502-MUL-IMAGE): the pin decides each multiply alone, and
//! llvm-mos and gcc expand a lone `x * 3` at -Os without a call (no
//! module-level re-selection). Reason: the call is 18 B shorter
//! than the expansion, beside the routine's 110 B, which a module with no
//! other multiply links for it alone
//! (licm_graph_callee_footprint, 467 B against gcc's 296). Exit: 2026-11-20,
//! with the shared inline-or-call cost model.

use crate::dagisel::function::{Selection, is_sized, select_function_with};
use crate::dagisel::hooks::Cpu;
use crate::driver::{Level, Request};
use crate::timing::estimate;

/// Bytes `routine` adds to an image (`mos_rt.s`): its code up to the next
/// `.export`, and the tail it ends in with a `jmp`.
pub fn library_bytes(routine: &str) -> u32 {
    // the routine's macros are defined elsewhere in the file
    let mut macros = String::new();
    let mut inside = false;
    for line in crate::RUNTIME.lines() {
        let word = line.trim_start();
        inside |= word.starts_with(".macro");
        if inside {
            macros.push_str(line);
            macros.push('\n');
        }
        inside &= !word.starts_with(".endmacro");
    }
    let priced = |text: &str| estimate(Cpu::Mos6502, &format!("{macros}{text}")).unwrap_or_else(|e| panic!("{e}")).bytes;
    fn code<'a>(routine: &str) -> &'a str {
        let marker = format!("\n{routine}:");
        let start = crate::RUNTIME.find(&marker).unwrap_or_else(|| panic!("no `{routine}` in mos_rt.s")) + 1;
        let body = &crate::RUNTIME[start..];
        let end = body.find("\n.export").unwrap_or(body.len());
        &body[..end]
    }
    let text = code(routine);
    let own = priced(text);
    let tail = text.lines().map(|line| line.split(';').next().unwrap_or("").trim()).filter(|line| !line.is_empty()).next_back();
    match tail.and_then(|line| line.strip_prefix("jmp ")) {
        Some(label) if !label.starts_with('@') => {
            let from = crate::RUNTIME.find(&format!("\n{label}:")).unwrap_or_else(|| panic!("no `{label}` in mos_rt.s")) + 1;
            let body = &crate::RUNTIME[from..];
            let end = body.find("\n  rts").unwrap_or_else(|| panic!("`{label}` has no rts")) + "\n  rts".len();
            own + priced(&body[..end])
        }
        _ => own,
    }
}

/// Selects the sized functions that call `__mulsi3` again with every multiply
/// by a constant expanded, and keeps those when no call is left in the module
/// and the code grows by less than the routine.
///
/// `from_module[i]` is whether `selections[i]` was selected from `module`
/// itself (not from a copy of it with loops or paged indexing).
pub fn expand_when_unshared(
    module: &llrm_mir::module::Module,
    names: &[String],
    selections: &mut [Selection],
    from_module: &[bool],
    request: &Request,
) -> Result<(), String> {
    if !matches!(request.level, Level::Os | Level::Oz) || selections.iter().all(|one| one.mul_calls() == 0) {
        return Ok(());
    }
    let mut again = Vec::new();
    let mut left = 0;
    for (at, (name, selection)) in names.iter().zip(selections.iter()).enumerate() {
        let function = module.named(name).and_then(|id| module.global(id).function());
        if selection.mul_calls() == 0 || !from_module[at] || !function.is_some_and(is_sized) {
            left += selection.mul_calls();
            continue;
        }
        let mut other = select_function_with(module, name, request.cpu, true)?;
        other.zero_page = request.platform.zero_page;
        other.tuning = request.level.tuning();
        left += other.mul_calls();
        again.push((at, other));
    }
    if left != 0 {
        return Ok(());
    }
    let bytes = |name: &str, selection: &Selection| -> Result<i64, String> {
        let text = crate::select::function_text(module, name, selection)?;
        Ok(i64::from(estimate(request.cpu, &text)?.bytes))
    };
    let mut growth = 0;
    for (at, other) in &again {
        growth += bytes(&names[*at], other)? - bytes(&names[*at], &selections[*at])?;
    }
    if growth < i64::from(library_bytes("__mulsi3")) {
        for (at, other) in again {
            selections[at] = other;
        }
    }
    Ok(())
}

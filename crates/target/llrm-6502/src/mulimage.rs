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
//! (licm_graph_callee_footprint, 467 B against gcc's 296). Exit: 2026-12-12,
//! with the shared inline-or-call cost model.

use crate::dagisel::function::{Selection, is_sized, select_function_with};
use crate::dagisel::hooks::Cpu;
use crate::driver::{Level, Request};
use crate::timing::estimate;

/// The text of `routine` in `mos_rt.s`: from its label to the next line that
/// starts something at the margin that is not a local label or a comment (an
/// `.export`, a macro, another routine).
fn routine_code(routine: &str) -> &'static str {
    let marker = format!("\n{routine}:");
    let start = crate::RUNTIME.find(&marker).unwrap_or_else(|| panic!("no `{routine}` in mos_rt.s")) + 1;
    let body = &crate::RUNTIME[start..];
    let mut end = body.len();
    let mut offset = 0;
    for (at, line) in body.split_inclusive('\n').enumerate() {
        let margin = line.chars().next().is_some_and(|first| !first.is_whitespace() && first != ';' && first != '@');
        if at > 0 && margin {
            end = offset;
            break;
        }
        offset += line.len();
    }
    &body[..end]
}

/// Bytes the runtime label `routine` adds to an image: its own code, up to the
/// next label.
fn label_bytes(routine: &str) -> u32 {
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
    let text = routine_code(routine);
    estimate(Cpu::Mos6502, &format!("{macros}{text}")).unwrap_or_else(|e| panic!("{e}")).bytes
}

/// Bytes a call of `routine` adds to an image (`mos_rt.s`): the routine and the
/// routines it calls or jumps to, as the link takes them.
pub fn library_bytes(routine: &str) -> u32 {
    let mut found = std::collections::BTreeSet::from([routine.to_owned()]);
    links_callees(&mut found);
    found.iter().map(|one| label_bytes(one)).sum()
}

/// A 32-bit multiply call, in clocks (`__mulsi3`, sim65): what each removed
/// call is worth when the bytes it costs are priced at speed.
const CALL_CLOCKS: i64 = 2200;
/// Clocks one byte is worth at -O2/-O3: the matrix's price.
const CLOCKS_PER_BYTE: i64 = 3;

/// Selects the functions that call `__mulsi3` again with every multiply by a
/// constant expanded, and keeps those when no call is left in the module and
/// the level pays for the growth:
/// - -Os/-Oz (sized functions only): the growth is less than the routine, so
///   the bytes fall;
/// - -O1 (bytes are strict, clocks free): the same, so no byte rises;
/// - -O2/-O3: the same, or the clocks the removed calls save (one execution
///   each, 2,200 a call) are at least 3 per byte the module grows.
///
/// DEVIATE 6502-MUL-SYNTH: see `dagisel::combine::synthesized_mul`; the
/// decision is the module's because the routine is linked once, for all its
/// calls.
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
    let at_size = matches!(request.level, Level::Os | Level::Oz);
    if matches!(request.level, Level::O0) || selections.iter().all(|one| one.mul_calls() == 0) {
        return Ok(());
    }
    let mut again = Vec::new();
    let mut left = 0;
    let mut removed = 0;
    for (at, (name, selection)) in names.iter().zip(selections.iter()).enumerate() {
        let function = module.named(name).and_then(|id| module.global(id).function());
        if selection.mul_calls() == 0 || !from_module[at] || (at_size && !function.is_some_and(is_sized)) {
            left += selection.mul_calls();
            continue;
        }
        let mut other = select_function_with(module, name, request.cpu, true)?;
        other.zero_page = request.platform.zero_page;
        other.tuning = request.level.tuning();
        left += other.mul_calls();
        removed += selection.mul_calls() as i64 - other.mul_calls() as i64;
        again.push((at, other));
    }
    // a call that stays keeps the routine linked: the expansions then only
    // add bytes, which at size the share rule below may still take
    if left != 0 && !at_size {
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
    let net = growth - if left == 0 { i64::from(library_bytes("__mulsi3")) } else { 0 };
    let pays = net < 0
        || (at_size && expansion_pays_by_share(module, names, selections, &again, request)?)
        || (!at_size && request.level != Level::O1 && removed * CALL_CLOCKS >= CLOCKS_PER_BYTE * net);
    if pays {
        for (at, other) in again {
            selections[at] = other;
        }
    }
    Ok(())
}

/// Whether, at size, the module with `again` expanded takes the growth in bytes
/// for the clocks it saves: the share of its clocks saved is at least
/// `[share.size] mul_expand` times the share of its bytes added, and the
/// expansion removes at least `mul_expand_floor` percent of its clocks, both
/// read from the module as written, the routines linked counted. A tie in bytes
/// pays (a module the expansion does not grow).
///
/// DEVIATE 6502-MUL-IMAGE (owner llrm-target-6502, exit 2026-12-12): the pin
/// decides each multiply alone, so the clocks are never traded for bytes;
/// here a module takes them by the rule it takes the speed multiply and
/// divide by (`select::fast_multiply_pays_at_size`). copy_runs and
/// mul_constants -Os ran the constant multiplies as calls, 1.4x llvm-mos's
/// clocks.
fn expansion_pays_by_share(
    module: &llrm_mir::module::Module,
    names: &[String],
    selections: &[Selection],
    again: &[(usize, Selection)],
    request: &Request,
) -> Result<bool, String> {
    let current: Vec<&Selection> = selections.iter().collect();
    let expanded: Vec<&Selection> = current
        .iter()
        .enumerate()
        .map(|(at, one)| again.iter().find(|(to, _)| *to == at).map_or(*one, |(_, other)| other))
        .collect();
    let clocks = |chosen: &[&Selection]| -> Result<u64, String> {
        let parts: Vec<(&str, &Selection)> = names.iter().map(String::as_str).zip(chosen.iter().copied()).collect();
        let pieces = crate::select::module_text(module, &parts)?;
        Ok(crate::select::priced(module, &parts, &pieces)?.0)
    };
    let (before, after) = (clocks(&current)?, clocks(&expanded)?);
    let (kept, tried) =
        (module_bytes(module, names, &current, request)?, module_bytes(module, names, &expanded, request)?);
    let saved = before.saturating_sub(after);
    if tried - kept > 0
        && u128::from(saved) * 100
            < u128::from(crate::routines::size_share_percent("mul_expand_floor")) * u128::from(before)
    {
        return Ok(false);
    }
    Ok(pays_by_share_at(
        crate::routines::size_share_percent("mul_expand"),
        (saved, tried - kept),
        before,
        kept.max(1) as u64,
    ))
}

/// The `mos_rt.s` routines the functions' text calls, each once.
fn linked(
    module: &llrm_mir::module::Module,
    names: &[String],
    selections: &[&Selection],
) -> Result<std::collections::BTreeSet<String>, String> {
    let mut found = std::collections::BTreeSet::new();
    for (name, selection) in names.iter().zip(selections) {
        let text = crate::select::function_text(module, name, selection)?;
        for target in text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("jsr ").or_else(|| line.trim().strip_prefix("jmp ")))
            .map(str::trim)
        {
            if names.iter().all(|one| one != target) && crate::RUNTIME.contains(&format!("\n{target}:")) {
                found.insert(target.to_owned());
            }
        }
    }
    links_callees(&mut found);
    Ok(found)
}

/// `found` and the routines they call, in turn.
fn links_callees(found: &mut std::collections::BTreeSet<String>) {
    let mut pending: Vec<String> = found.iter().cloned().collect();
    while let Some(routine) = pending.pop() {
        for line in routine_code(&routine).lines() {
            let code = line.split(';').next().unwrap_or("").trim();
            // a routine that branches into another one (`udivmod16`: `bne
            // udivmod16c`) links it too
            let callee = code
                .strip_prefix("jsr ")
                .or_else(|| code.strip_prefix("jmp "))
                .or_else(|| {
                    ["bne ", "beq ", "bcc ", "bcs ", "bmi ", "bpl ", "bvc ", "bvs "]
                        .iter()
                        .find_map(|op| code.strip_prefix(op))
                })
                .map(str::trim);
            if let Some(callee) =
                callee.filter(|one| !one.starts_with('@') && crate::RUNTIME.contains(&format!("\n{one}:")))
            {
                if found.insert(callee.to_owned()) {
                    pending.push(callee.to_owned());
                }
            }
        }
    }
}

/// A product of two words widened to 32 bits is one `__umulhisi3` (16 x 16)
/// where `__mulsi3` is 32 x 32: shorter arguments, a third of the clocks, and a
/// second routine in the image. At -Os the routine is paid for by the call
/// sites or it is not linked: the module is selected again with every such
/// product widened, and kept when its bytes, the routines linked and the
/// routines no longer called counted, do not rise (idioms and int64_division
/// each grew with a per-product rule: +93 B and +49 B for a routine four sites
/// do not pay).
///
/// DEVIATE 6502-WIDEN-IMAGE (owner llrm-target-6502, exit 2026-12-05): the pin
/// decides each product alone and links no routine by count. Exit: with the
/// shared inline-or-call cost model.
///
/// `from_module[i]` is whether `selections[i]` was selected from `module`
/// itself.
pub fn widen_when_it_pays(
    module: &llrm_mir::module::Module,
    names: &[String],
    selections: &mut [Selection],
    from_module: &[bool],
    request: &Request,
) -> Result<(), String> {
    use crate::dagisel::function::{Widen, set_widen_products};
    if !matches!(request.level, Level::Os | Level::Oz) {
        return Ok(());
    }
    set_widen_products(Widen::Always);
    let mut again = Vec::new();
    for (at, (name, selection)) in names.iter().zip(selections.iter()).enumerate() {
        if selection.mul_calls() == 0 || !from_module[at] {
            continue;
        }
        let mut other = select_function_with(module, name, request.cpu, false);
        if let Ok(other) = other.as_mut() {
            other.zero_page = request.platform.zero_page;
            other.tuning = request.level.tuning();
        }
        match other {
            Ok(other) if other.mul_calls() < selection.mul_calls() => again.push((at, other)),
            Ok(_) => {}
            Err(why) => {
                set_widen_products(Widen::Off);
                return Err(why);
            }
        }
    }
    set_widen_products(Widen::Off);
    if again.is_empty() {
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
    let current: Vec<&Selection> = selections.iter().collect();
    let widened: Vec<&Selection> = current
        .iter()
        .enumerate()
        .map(|(at, one)| again.iter().find(|(to, _)| *to == at).map_or(*one, |(_, other)| other))
        .collect();
    let library = |routines: &std::collections::BTreeSet<String>| -> i64 {
        routines.iter().map(|one| i64::from(label_bytes(one))).sum()
    };
    let net = growth + library(&linked(module, names, &widened)?) - library(&linked(module, names, &current)?);
    if net <= 0 {
        for (at, other) in again {
            selections[at] = other;
        }
    }
    Ok(())
}

/// Whether a module, of `clocks` and `bytes` by the static estimate, takes a
/// speed routine at size that saves `saved` clocks for `growth` bytes: when the
/// share of its clocks it saves is at least `[share] percent_per_percent` times
/// the share of its bytes it adds.
pub fn pays_by_share(
    (saved, growth): (u64, i64),
    clocks: u64,
    bytes: u64,
) -> bool {
    pays_by_share_at(crate::routines::share_percent(), (saved, growth), clocks, bytes)
}

/// [`pays_by_share`] at `percent` clocks per percent of bytes.
pub fn pays_by_share_at(
    percent: u64,
    (saved, growth): (u64, i64),
    clocks: u64,
    bytes: u64,
) -> bool {
    let percent = u128::from(percent);
    growth <= 0 || u128::from(saved) * u128::from(bytes) >= percent * growth as u128 * u128::from(clocks)
}

/// The `mos_rt.s` routines the function `texts` call or jump to, with the ones
/// those call.
pub fn image_of_texts(
    names: &[String],
    texts: &[String],
) -> std::collections::BTreeSet<String> {
    let mut found = std::collections::BTreeSet::new();
    for text in texts {
        for line in text.lines() {
            let code = line.trim();
            let Some(target) = code.strip_prefix("jsr ").or_else(|| code.strip_prefix("jmp ")).map(str::trim) else {
                continue;
            };
            if names.iter().all(|one| one != target) && crate::RUNTIME.contains(&format!("\n{target}:")) {
                found.insert(target.to_owned());
            }
        }
    }
    links_callees(&mut found);
    found
}

/// The bytes the routines add to an image.
pub fn image_bytes(routines: &std::collections::BTreeSet<String>) -> i64 {
    routines.iter().map(|one| i64::from(label_bytes(one))).sum()
}

/// The bytes of the module as `selections` write it, the runtime routines it
/// links counted: what a multiply as a call is priced by, since the routine is
/// linked once, for all of them.
fn module_bytes(
    module: &llrm_mir::module::Module,
    names: &[String],
    selections: &[&Selection],
    request: &Request,
) -> Result<i64, String> {
    let parts: Vec<(&str, &Selection)> = names.iter().map(String::as_str).zip(selections.iter().copied()).collect();
    let pieces = crate::select::module_text(module, &parts)?;
    let mut bytes = 0;
    for piece in &pieces {
        bytes += i64::from(estimate(request.cpu, piece)?.bytes);
    }
    let routines = image_of_texts(names, &pieces);
    if std::env::var_os("LLRM_DEBUG_TRIAL").is_some() {
        eprintln!("trial mul call: functions {bytes} B, routines {} B {routines:?}", image_bytes(&routines));
    }
    Ok(bytes + image_bytes(&routines))
}

/// A multiply of 32 bits by a constant outside every loop that a function not
/// tuned for size expands in line is a call of `__mulsi3` when the module is
/// written in fewer bytes so: the routine is linked once, for every call
/// (compound_assignment_types at -O2: twelve expansions of `check * 7` of
/// 22 B against twelve calls of 8 B and one routine). Bytes first at -O1
/// (user ruling 2026-11-14); -O2 and -O3 at the judge's rate: a call is some
/// 2,200 clocks a multiply, so the bytes it saves must repay them at
/// `CLOCKS_PER_BYTE`; a place in a loop keeps the expansion.
///
/// DEVIATE 6502-MUL-CALL-TRIAL (owner 6502, exit 2026-11-24, with
/// 6502-MUL-IMAGE): translation of the repo's module-level re-selections
/// (`expand_when_unshared`, `widen_when_it_pays`, and `OS-INLINE-TRIAL`'s
/// compile-twice-keep-the-fewer-bytes); the pin decides each multiply alone
/// ([`DAGCombiner::visitMUL`]), and llvm-mos and gcc expand it. The module is
/// selected again for the functions that have such a multiply, only when one
/// does, and kept when its bytes, the routines linked (the speed and compact
/// variants of `module_text` included) counted, fall.
///
/// `from_module[i]` is whether `selections[i]` was selected from `module`
/// itself (not from a copy of it with loops or paged indexing).
pub fn call_when_it_pays(
    module: &llrm_mir::module::Module,
    names: &[String],
    selections: &mut [Selection],
    from_module: &[bool],
    request: &Request,
) -> Result<(), String> {
    use crate::dagisel::function::with_cold_multiply_calls;
    if !matches!(request.level, Level::O1 | Level::O2 | Level::O3)
        || std::env::var_os("LLRM_6502_NO_MUL_CALL").is_some()
    {
        return Ok(());
    }
    // static call weight, as `hotmultiply` counts it: ten a level of loop
    let calls = crate::staticlocals::call_weights(module);
    let loop_called: Vec<String> = names
        .iter()
        .filter(|name| {
            module.named(name).is_some_and(|id| calls.get(&(id.0 as usize)).is_some_and(|weight| *weight >= 10.0))
        })
        .cloned()
        .collect();
    let mut again = Vec::new();
    // The clocks the calls add, a call at `CALL_CLOCKS` for each time the
    // function is called (`call_weights`; one where nothing calls it).
    let mut added_clocks = 0;
    for (at, (name, selection)) in names.iter().zip(selections.iter()).enumerate() {
        let function = module.named(name).and_then(|id| module.global(id).function());
        if selection.call_candidates == 0 || !from_module[at] || function.is_some_and(is_sized) {
            continue;
        }
        let mut other =
            with_cold_multiply_calls(loop_called.clone(), || select_function_with(module, name, request.cpu, false))?;
        other.zero_page = request.platform.zero_page;
        other.tuning = request.level.tuning();
        if other.mul_calls() > selection.mul_calls() {
            let runs = module.named(name).and_then(|id| calls.get(&(id.0 as usize))).copied().unwrap_or(1.0).max(1.0);
            added_clocks += ((other.mul_calls() - selection.mul_calls()) as f64 * runs * CALL_CLOCKS as f64) as i64;
            again.push((at, other));
        }
    }
    if again.is_empty() {
        return Ok(());
    }
    let current: Vec<&Selection> = selections.iter().collect();
    let called: Vec<&Selection> = current
        .iter()
        .enumerate()
        .map(|(at, one)| again.iter().find(|(to, _)| *to == at).map_or(*one, |(_, other)| other))
        .collect();
    let (kept, tried) =
        (module_bytes(module, names, &current, request)?, module_bytes(module, names, &called, request)?);
    if std::env::var_os("LLRM_DEBUG_TRIAL").is_some() {
        eprintln!("trial mul call: {} functions, {tried} B against {kept} B kept", again.len());
    }
    // -O1: bytes strict, clocks free. -O2/-O3: the judge's rate: the bytes the
    // calls save must repay the clocks they add (`expand_when_unshared`'s
    // rule, seen from the other side).
    let pays = match request.level {
        Level::O1 => tried < kept,
        _ => tried < kept && (kept - tried) * CLOCKS_PER_BYTE >= added_clocks,
    };
    if pays {
        for (at, other) in again {
            selections[at] = other;
        }
    }
    Ok(())
}

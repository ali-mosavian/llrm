//! LIR as text: the one printer of a body, used by every dump, check message
//! and test. `ISEL_DUMP=1` prints it after each machine phase;
//! `ISEL_DUMP=verbose` adds what the compact form leaves out.
//!
//! # How it works
//! Per body: a banner, a `fn` line (the registers its arguments arrive in, the
//! registers it returns in), a `frame` line naming the frame's slots, then
//! each block. A block is a header (`B1  ← B0 B2  → B2 B3  loop  ×8.0`) and
//! its instructions, set in fixed columns: its label (`#12`), the
//! destination, `=`, the opcode with its width, the operands, then `; notes`.
//! Blocks are named by position and every target says its block. An
//! instruction's label is given once, in emit order, and kept by every pass
//! that keeps the instruction (`Insn::label`, `Phi::label`), so two dumps of
//! one body differ only where the instructions do.
//! A block is indented four spaces for each loop around it, so a loop is a
//! shape on the page; its header says `loop` and, for an innermost loop whose
//! trip count is known, the count. Operand and instruction text is
//! `llrm_lir::text`.
//!
//! A value is called by its name where MIR gave it one (`lsr.iv`), else `t12`;
//! a copy of a named value keeps the name (`LirBody::names`).
//!
//! A note is printed only where it says something: `spill` and `reload` for
//! frame traffic, `copy` for a phi's parallel copy, `remat`, `frame` for a
//! frame adjustment, `N% taken` on a branch whose odds isel stated, `elided`
//! for an operation selection removed that still carries a value (`v5 =
//! elided v2`). A call lists its clobbers only as a difference from the
//! body's most common set, which the `fn` line states. Verbose adds what an
//! instruction needs and gives in registers, the source bytes it covers, its
//! line, and each block's live-in and live-out values.
//!
//! # Example
//! The first blocks of a Fibonacci loop (`ISEL_DUMP=1 llrm-c -m16 -O2`), after
//! the allocator. `#20` is an operation selection removed, numbered when it
//! first appeared in a dump:
//!
//! ```lir
//! ═══ _f@3 · RegAlloc · m16 ══════════════════════════════════════════════════════
//! fn _f@3(t5@ax) -> @eax
//!
//! B0  entry  → B1                                                   ×1.0
//!     #24  t1         = elided      t5
//!      #2  bx         = mov.i2      1
//!      #3  cx         = mov.i2      0
//!      #4               jmp         B1
//!
//!     B1  ← B0 B3  → B3 B2  loop                                        ×32.0
//!          #8  ax         = sub.i2      ax, 1
//!          #9               cmp.i2      ax, -1
//!         #10               jne         B3 else B2                            ; 97% taken
//! ```
//!
//! # Used by
//! `assemble` after each machine phase when `ISEL_DUMP` is set, the checks of
//! `verify` to name a place, and the allocator's tests.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::text::{self, Rendered, Syntax};
use llrm_omf::module::{Addr, Space};

use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBlock, LirBody, ValueNames};
use crate::support::hash::IndexMap;

/// How much a dump says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Compact,
    Verbose,
}

/// Colour only where the output is a terminal; the caller knows which.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Style {
    Plain,
    Color,
}

/// What a call is to, by the call's instruction address: not in the LIR, which
/// holds the call as an operation with no operand.
#[derive(Clone, Debug, Default)]
pub struct Calls {
    pub names: IndexMap<i64, String>,
    pub far: BTreeSet<i64>,
}

/// Values by their names: a source name, else `t12`.
struct Namer {
    names: ValueNames,
    defined: std::collections::BTreeMap<u32, u32>,
}

impl Namer {
    fn of(body: &LirBody) -> Self {
        Namer { names: body.with_inherited_names(), defined: body.definitions() }
    }

    /// A name a copy gave holds while the value is defined once; defined again
    /// (after the phis are gone) it is another version, and has none.
    fn name(
        &self,
        value: u32,
    ) -> String {
        let kept = self.names.get(value).filter(|(_, inherited)| !inherited || self.defined.get(&value) == Some(&1));
        kept.map_or_else(|| format!("t{value}"), |(name, _)| name.to_owned())
    }
}

/// Columns, in characters from a block's indent: fixed, so a line's place does
/// not depend on what the other lines say.
const DEST_WIDTH: usize = 10;
const OP_WIDTH: usize = 12;
const NOTE_COLUMN: usize = 58;
const BANNER_WIDTH: usize = 80;
const HEADER_WIDTH: usize = 66;

impl Style {
    fn paint(
        self,
        code: &str,
        text: &str,
    ) -> String {
        match self {
            Style::Plain => text.to_owned(),
            Style::Color => format!("\x1b[{code}m{text}\x1b[0m"),
        }
    }
}

/// `lir_stage_with`, compact and plain, for a call not told what its calls
/// are to.
pub fn lir_stage(
    stage: &str,
    bodies: &[(String, LirBody)],
) -> String {
    lir_stage_with(stage, bodies, &Calls::default(), Mode::Compact, Style::Plain)
}

/// Each body under a banner naming it and the stage.
pub fn lir_stage_with(
    stage: &str,
    bodies: &[(String, LirBody)],
    calls: &Calls,
    mode: Mode,
    style: Style,
) -> String {
    let mut out = String::new();
    for (_, one) in bodies {
        let title = format!("\u{2550}\u{2550}\u{2550} {} \u{b7} {stage} \u{b7} m{} ", one.name, one.bits);
        let fill = BANNER_WIDTH.saturating_sub(title.chars().count());
        out += &style.paint("1", &format!("{title}{}", "\u{2550}".repeat(fill)));
        out.push('\n');
        out += &lir_body(one, calls, mode, style);
        out.push('\n');
    }
    out
}

/// The name of a block: its position in the body.
fn names(body: &LirBody) -> BTreeMap<i64, usize> {
    body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect()
}

fn named(
    position: &BTreeMap<i64, usize>,
    at: i64,
) -> String {
    position.get(&at).map_or_else(|| format!("B?{at}"), |index| format!("B{index}"))
}

/// What a block prints: the phis, then each instruction but the arrival, which
/// the `fn` line holds.
fn shown(block: &LirBlock) -> impl Iterator<Item = &Arc<Insn>> {
    block.insns.iter().filter(|insn| !insn.arrival())
}

/// Where an instruction is: `B3.2`, its block and its line in that block, for
/// a check to name and a reader to find in a dump.
pub fn place(
    body: &LirBody,
    at: i64,
) -> String {
    body.blocks
        .iter()
        .enumerate()
        .find_map(|(block, one)| {
            shown(one).position(|insn| insn.at == at).map(|index| format!("B{block}.{}", one.phis.len() + index))
        })
        .unwrap_or_else(|| format!("B?.{at}"))
}

/// One instruction on its own, for a message that has no body to name a place
/// in: its operation and operands, blocks as `B@address`.
pub fn insn_text(insn: &Insn) -> String {
    insn.what.as_ref().map_or_else(|| "carried".to_owned(), |what| text::render(what, &Syntax::plain()).to_string())
}

/// `block`'s name, for a check message.
pub fn block_name(
    body: &LirBody,
    at: i64,
) -> String {
    named(&names(body), at)
}

/// The clobber set most calls of the body have, the largest where they tie.
fn usual_clobbers(body: &LirBody) -> Option<BTreeSet<RegId>> {
    let mut counted: BTreeMap<BTreeSet<RegId>, usize> = BTreeMap::new();
    for insn in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        if insn.what.as_ref().is_some_and(|what| what.op == Operation::Call) {
            *counted.entry(insn.clobbers.clone()).or_default() += 1;
        }
    }
    counted.into_iter().max_by_key(|(set, count)| (*count, set.len())).map(|(set, _)| set)
}

fn registers(set: &BTreeSet<RegId>) -> String {
    set.iter().map(|one| text::register(*one)).collect::<Vec<_>>().join(" ")
}

/// The frame cells a body names, as slots: a cell lies in the slot its
/// address is tagged with, else in one of its own. Locals count down from the
/// frame register, arguments up.
struct Frame {
    /// Slot start (a displacement from the frame register), name, bytes.
    slots: Vec<(i64, String, i64)>,
}

impl Frame {
    fn of(body: &LirBody) -> Self {
        let mut extent: BTreeMap<i64, i64> = BTreeMap::new();
        let cells = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter_map(|insn| insn.what.as_ref())
            .flat_map(|what| what.dests.iter().chain(&what.sources));
        for cell in cells {
            let Loc::Mem(cell) = cell else { continue };
            let Some(addr) = cell.addr.as_ref().filter(|addr| addr.space == Space::Frame) else { continue };
            let home = addr.slot_home().unwrap_or(addr.disp);
            let end = addr.disp - home + i64::from(cell.width);
            let held = extent.entry(home).or_insert(0);
            *held = (*held).max(end);
        }
        let (mut locals, arguments): (Vec<_>, Vec<_>) = extent.into_iter().partition(|(home, _)| *home < 0);
        locals.reverse();
        let slots = locals
            .into_iter()
            .enumerate()
            .map(|(index, (home, size))| (home, format!("slot{index}"), size))
            .chain(arguments.into_iter().enumerate().map(|(index, (home, size))| (home, format!("arg{index}"), size)))
            .collect();
        Frame { slots }
    }

    fn name(
        &self,
        addr: &Addr,
    ) -> Option<String> {
        let home = addr.slot_home().unwrap_or(addr.disp);
        let (_, name, _) = self.slots.iter().find(|(start, ..)| *start == home)?;
        let inside = addr.disp - home;
        Some(if inside == 0 { name.clone() } else { format!("{name}{inside:+}") })
    }

    fn line(&self) -> Option<String> {
        if self.slots.is_empty() {
            return None;
        }
        let each = self.slots.iter().map(|(home, name, size)| format!("{name} [bp{home:+}] {size} B"));
        Some(format!("frame  {}", each.collect::<Vec<_>>().join("  ")))
    }
}

/// One body: its `fn` line, then its blocks.
pub fn lir_body(
    body: &LirBody,
    calls: &Calls,
    mode: Mode,
    style: Style,
) -> String {
    body.number_insns();
    let namer = Namer::of(body);
    let position = names(body);
    let usual = usual_clobbers(body);
    let arguments: Vec<String> = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|insn| insn.arrival())
        .flat_map(|insn| insn.delivers.iter().map(|(held, at)| text::held(*held, *at, &|value| namer.name(value))))
        .collect();
    let returns: BTreeSet<RegId> = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .filter(|insn| insn.what.as_ref().is_some_and(|what| what.op == Operation::Return))
        .flat_map(|insn| insn.requires.iter().map(|(_, at)| *at))
        .collect();
    let mut out = format!("fn {}({})", body.name, arguments.join(", "));
    if !returns.is_empty() {
        let _ = write!(
            out,
            " -> {}",
            returns.iter().map(|at| format!("@{}", text::register(*at))).collect::<Vec<_>>().join(" ")
        );
    }
    if let Some(usual) = usual.as_ref().filter(|set| !set.is_empty()) {
        let _ = write!(out, "   ; calls clobber {}", registers(usual));
    }
    out.push('\n');
    let frame = Frame::of(body);
    if let Some(line) = frame.line() {
        out += &line;
        out.push('\n');
    }
    let predecessors = body
        .blocks
        .iter()
        .fold(
            BTreeMap::<i64, Vec<i64>>::new(),
            |mut found, block| {
                block.succ.iter().for_each(|to| found.entry(*to).or_default().push(block.at));
                found
            },
        );
    let forest = crate::analysis::loops::forest_of(&body.blocks, Some(body.entry));
    let live = (mode == Mode::Verbose).then(|| crate::backend::allocate::live(body));
    let frame_name = |addr: &Addr| frame.name(addr);
    for block in &body.blocks {
        let depth = forest.depth(block.at) as usize;
        let indent = " ".repeat(4 * depth);
        let header = _header(body, &namer, block, &position, &predecessors, &forest, live.as_ref());
        out.push('\n');
        out += &indent;
        out += &style.paint("1", &header);
        out.push('\n');
        out += &_block(body, &namer, block, &position, calls, usual.as_ref(), &frame_name, mode, style, &indent);
    }
    out
}

fn _list(
    label: &str,
    names: Vec<String>,
) -> String {
    if names.is_empty() { String::new() } else { format!("{label} {}", names.join(" ")) }
}

fn _header(
    body: &LirBody,
    namer: &Namer,
    block: &LirBlock,
    position: &BTreeMap<i64, usize>,
    predecessors: &BTreeMap<i64, Vec<i64>>,
    forest: &llrm_mir::forest::Forest,
    live: Option<&(crate::backend::allocate::Live, crate::backend::allocate::Live)>,
) -> String {
    let name = |at: &i64| named(position, *at);
    let mut parts = vec![named(position, block.at)];
    if block.at == body.entry {
        parts.push("entry".to_owned());
    }
    parts.push(_list(
        "\u{2190}",
        predecessors.get(&block.at).map(|found| found.iter().map(name).collect()).unwrap_or_default(),
    ));
    parts.push(if block.succ.is_empty() {
        "\u{2192} exit".to_owned()
    } else {
        _list("\u{2192}", block.succ.iter().map(name).collect())
    });
    if let Some(found) = forest.loops.iter().find(|one| one.header == block.at) {
        let trips =
            body.loop_trip_counts.iter().find(|(header, _)| *header == block.at).filter(|_| found.children.is_empty());
        parts.push(trips.map_or_else(|| "loop".to_owned(), |(_, count)| format!("loop {count} trips")));
    }
    if block.cold {
        parts.push("cold".to_owned());
    }
    if let Some((into, out_of)) = live {
        let values = |sets: &crate::backend::allocate::Live| {
            sets.get(&block.at).map(|set| set.iter().map(|value| namer.name(*value)).collect()).unwrap_or_default()
        };
        parts.push(_list("live-in", values(into)));
        parts.push(_list("live-out", values(out_of)));
    }
    let mut out = parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join("  ");
    if let Some(frequency) = body.frequencies.as_ref().and_then(|found| found.0.get(&block.at)) {
        let pad = HEADER_WIDTH.saturating_sub(out.chars().count()).max(2);
        let _ = write!(out, "{}\u{d7}{frequency:.1}", " ".repeat(pad));
    }
    out
}

/// One printed instruction: its parts, and its notes.
struct Line {
    /// `#12`.
    label: String,
    dest: String,
    op: String,
    operands: String,
    notes: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
fn _block(
    body: &LirBody,
    namer: &Namer,
    block: &LirBlock,
    position: &BTreeMap<i64, usize>,
    calls: &Calls,
    usual: Option<&BTreeSet<RegId>>,
    frame: &dyn Fn(&Addr) -> Option<String>,
    mode: Mode,
    style: Style,
    indent: &str,
) -> String {
    let mut lines: Vec<Line> = block
        .phis
        .iter()
        .map(|phi| {
            // By block, whatever order the pass built them in: the same text
            // for the same phi.
            let mut incoming: Vec<_> = phi.incoming.iter().collect();
            incoming.sort_by_key(|(from, _)| position.get(from).copied().unwrap_or(usize::MAX));
            let arms = incoming
                .iter()
                .map(|(from, value)| format!("{}: {}", named(position, *from), namer.name(*value)))
                .collect::<Vec<_>>()
                .join(", ");
            Line {
                label: format!("#{}", phi.label.get()),
                dest: namer.name(phi.result),
                op: "phi".to_owned(),
                operands: arms,
                notes: Vec::new(),
            }
        })
        .collect();
    lines.extend(shown(block).map(|insn| _line(frame, body, namer, block, insn, position, calls, usual, mode)));
    let mut out = String::new();
    for line in &lines {
        let equals = if line.dest.is_empty() { "  " } else { "= " };
        let mut said =
            format!("{indent}  {:>5}  {:<DEST_WIDTH$} {equals}{:<OP_WIDTH$}", line.label, line.dest, line.op);
        said += &line.operands;
        let said = said.trim_end().to_owned();
        let mut said = said;
        if !line.notes.is_empty() {
            let column = indent.len() + NOTE_COLUMN + 14;
            let gap = column.saturating_sub(said.chars().count()).max(2);
            said += &" ".repeat(gap);
            said += &style.paint("2", &format!("; {}", line.notes.join("; ")));
        }
        out += &said;
        out.push('\n');
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn _line(
    frame: &dyn Fn(&Addr) -> Option<String>,
    body: &LirBody,
    namer: &Namer,
    block: &LirBlock,
    insn: &Insn,
    position: &BTreeMap<i64, usize>,
    calls: &Calls,
    usual: Option<&BTreeSet<RegId>>,
    mode: Mode,
) -> Line {
    let verbose = mode == Mode::Verbose;
    let mut notes: Vec<String> = Vec::new();
    let Some(what) = &insn.what else {
        if let Some((from, to)) = insn.covers {
            notes.push(format!("covers {from}..{to}"));
        }
        return Line {
            label: format!("#{}", insn.label.get()),
            dest: String::new(),
            op: "carried".to_owned(),
            operands: String::new(),
            notes,
        };
    };
    let held = |pairs: &[(llrm_lir::Held, RegId)]| {
        pairs.iter().map(|(value, at)| text::held(*value, *at, &|number| namer.name(number))).collect::<Vec<_>>()
    };
    let arguments = held(&insn.requires);
    let block_name = |at| named(position, at);
    let otherwise = (what.op == Operation::Branch)
        .then(|| block.succ.iter().copied().find(|to| Some(*to) != what.target).map(block_name))
        .flatten();
    let spelled = calls.names.get(&insn.at).map(|name| inline_code(name));
    let callee = spelled.as_deref().map(|name| (name, calls.far.contains(&insn.at)));
    let value_name = |number| namer.name(number);
    let syntax = Syntax { block: &block_name, value: &value_name, frame, callee, arguments: &arguments, otherwise };
    let Rendered { mut dest, mut op, mut operands } = text::render(what, &syntax);
    // Values the instruction defines or reads that no operand names: all an
    // elided operation is, so they are what it prints.
    fn operand_values(locs: &[Loc]) -> impl Iterator<Item = u32> + '_ {
        locs.iter().flat_map(|one| llrm_lir::values(one)).map(|held| held.value)
    }
    let named_defs: BTreeSet<u32> = what
        .dests
        .iter()
        .filter_map(|one| if let Loc::Held(held) = one { Some(held.value) } else { None })
        .chain(insn.delivers.iter().map(|(held, _)| held.value))
        .collect();
    let named_uses: BTreeSet<u32> = operand_values(&what.sources)
        .chain(operand_values(&what.dests).filter(|value| !named_defs.contains(value)))
        .chain(insn.requires.iter().map(|(held, _)| held.value))
        .collect();
    let extra_defs: Vec<u32> = insn.defines.iter().copied().filter(|value| !named_defs.contains(value)).collect();
    let extra_uses: Vec<u32> = insn.uses.iter().copied().filter(|value| !named_uses.contains(value)).collect();
    let value_list = |values: &[u32]| values.iter().map(|value| namer.name(*value)).collect::<Vec<_>>().join(", ");
    let delivered = |pairs: &[(llrm_lir::Held, RegId)]| held(pairs).join(", ");
    match what.op {
        Operation::Nothing if what.name.as_deref().is_none_or(str::is_empty) && arguments.is_empty() => {
            dest = value_list(&extra_defs);
            op = "elided".to_owned();
            operands = value_list(&extra_uses);
        }
        Operation::Nothing if what.name.as_deref().is_none_or(str::is_empty) => {
            op = "use".to_owned();
            operands = arguments.join(", ");
        }
        Operation::Call => dest = delivered(&insn.delivers),
        _ => {
            if verbose && !insn.requires.is_empty() && what.op != Operation::Return {
                notes.push(format!("needs {}", arguments.join(", ")));
            }
            if verbose && !insn.delivers.is_empty() {
                notes.push(format!("gives {}", delivered(&insn.delivers)));
            }
            if verbose && !extra_defs.is_empty() {
                notes.push(format!("also defines {}", value_list(&extra_defs)));
            }
            if verbose && !extra_uses.is_empty() {
                notes.push(format!("also uses {}", value_list(&extra_uses)));
            }
        }
    }
    if insn.spill_store {
        notes.push("spill".to_owned());
    }
    if insn.spill_reload {
        notes.push("reload".to_owned());
    }
    if insn.rematerialized {
        notes.push("remat".to_owned());
    }
    if insn.group.is_some() {
        notes.push("copy".to_owned());
    }
    if insn.frame_adjust {
        notes.push("frame".to_owned());
    }
    if insn.volatile {
        notes.push("volatile".to_owned());
    }
    if what.op == Operation::Branch {
        let target = what.target.unwrap_or_default();
        if let Some(chance) = body.odds.probability(block.at, target) {
            notes.push(format!("{:.0}% taken", chance * 100.0));
        }
    }
    let clobbers = _clobbers(&insn.clobbers, usual.filter(|_| what.op == Operation::Call));
    if !clobbers.is_empty() {
        notes.push(clobbers);
    }
    if verbose {
        if let Some((from, to)) = insn.covers.filter(|(from, to)| from != to) {
            notes.push(format!("covers {from}..{to}"));
        }
        if let Some(line) = insn.line {
            notes.push(format!("line {line}"));
        }
    }
    Line { label: format!("#{}", insn.label.get()), dest, op, operands, notes }
}

/// A callee's name, where the name is inline code or assembly: its bytes and
/// what they take and give, not the mangled name that carries them.
fn inline_code(name: &str) -> String {
    use llrm_mir::intrinsics;
    let bytes = |code: &[u8]| code.iter().map(|one| format!("{one:02x}")).collect::<Vec<_>>().join(" ");
    if let Some((code, places)) = intrinsics::code(name) {
        let places = places.iter().map(|(at, addend)| format!("  arg@{at}{addend:+}")).collect::<String>();
        return format!("asm{{{}{places}}}", bytes(&code));
    }
    if let Some(block) = intrinsics::asm(name) {
        let list = |label: &str, names: &[String]| {
            if names.is_empty() { String::new() } else { format!("  {label} {}", names.join(" ")) }
        };
        let memory = if block.memory { "  memory" } else { "" };
        return format!(
            "asm{{{}{}{}{}{memory}}}",
            bytes(&block.code),
            list("in", &block.inputs),
            list("out", &block.outputs),
            list("clobbers", &block.clobbers)
        );
    }
    name.to_owned()
}

/// A clobber set: whole, or as the difference from `usual`.
fn _clobbers(
    set: &BTreeSet<RegId>,
    usual: Option<&BTreeSet<RegId>>,
) -> String {
    let Some(usual) = usual else {
        return if set.is_empty() { String::new() } else { format!("clobbers {}", registers(set)) };
    };
    let more = set.difference(usual).map(|one| text::register(*one)).collect::<Vec<_>>();
    let fewer = usual.difference(set).map(|one| text::register(*one)).collect::<Vec<_>>();
    let mut said = Vec::new();
    if !more.is_empty() {
        said.push(format!("also clobbers {}", more.join(" ")));
    }
    if !fewer.is_empty() {
        said.push(format!("keeps {}", fewer.join(" ")));
    }
    said.join("; ")
}

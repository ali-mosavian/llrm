//! Port of `qbopt/frontend/blocks.py`: which bytes of a module are code,
//! and where control goes.
//!
//! Reachability decides it: start from the entry points the records name,
//! follow control flow, and every byte reached is code by construction.
//! `B$OGTA` does not return to the byte after its call -- `ON GOTO` lays a
//! count byte and that many offset16 words there -- and the only thing that
//! says so is the EXTDEF the fixup names.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use iced_x86::{Code, FlowControl, Register};

use crate::frontend::declen::{Insn, decode};
use crate::frontend::fppatches;
use crate::objectfile::module::{Module, Space, defines, family};
use crate::objectfile::omf::{self, Fixup};
use crate::support::hash::IndexMap;

// The one routine that reads a table laid inline after its own call site.
// Established from the shipped libraries rather than inferred -- gosub.asm
// in BCOM45.LIB, BCL71ENR.LIB and VBDCL10E.LIB, read with tools/libdump.py.
// The implementations differ; the QB form below shows the table protocol:
//
//   lds  si,[bp+2]     si = the return address, which IS the table
//   lodsb              al = the count, si now on the entries
//   mov  dl,al / shl dx,1 / add dx,si    dx = past the table
//   mov  cx,[bx+si]    cx = entry[index-1]
//   cmp  al,bl / jbe   out of range falls through to dx
//   push bx / push dx  and a far return goes to whichever was chosen
//
// So an entry is a two-byte offset into this same segment, the count is one
// byte in front of them. Selectors 0 or count+1..255 fall through; values
// above 255 enter B$FrameFC instead. These are normal CFG successors only.
pub static INLINE_TABLE: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| BTreeSet::from(["B$OGTA"]));

pub const PAD: u8 = 0x90;

/// How a block ends.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Ends {
    FallsThrough,
    Conditional,
    Jump,
    Indirect,
    Return,
    Leaves,
    Table,
}

impl Ends {
    /// The `StrEnum` value.
    pub fn value(self) -> &'static str {
        match self {
            Ends::FallsThrough => "falls-through",
            Ends::Conditional => "conditional",
            Ends::Jump => "jump",
            Ends::Indirect => "indirect",
            Ends::Return => "return",
            Ends::Leaves => "leaves",
            Ends::Table => "table",
        }
    }
}

// What iced calls it, and what it means for a block. The one that matters is
// that a call is NEXT-like: it comes back, and treating it as an end halved what
// liveness could see -- 49 of 102 blocks.
fn ends_of(flow: FlowControl) -> Option<Ends> {
    Some(match flow {
        FlowControl::Next | FlowControl::Call | FlowControl::Interrupt | FlowControl::IndirectCall => {
            Ends::FallsThrough
        }
        FlowControl::ConditionalBranch => Ends::Conditional,
        FlowControl::UnconditionalBranch => Ends::Jump,
        FlowControl::IndirectBranch => Ends::Indirect,
        FlowControl::Return => Ends::Return,
        FlowControl::Exception | FlowControl::XbeginXabortXend => Ends::Leaves,
    })
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodeMap {
    pub starts: BTreeSet<usize>,
    pub leaders: BTreeSet<usize>,
    pub tables: Vec<(usize, usize)>,
    pub unreached: Vec<(usize, usize)>,
    pub procedures: BTreeSet<usize>,
}

/// What this instruction does to control flow.
pub fn terminator(insn: &Insn, module: Option<&Module>) -> Ends {
    if let Some(module) = module {
        if module.calls.get(&(insn.at as i64)).map(String::as_str) == Some("B$RETA")
            && matches!(family(&module.records).value(), "pds71" | "vbdos")
            && !defines(&module.records, module.seg).contains("B$RETA")
        {
            // gosub.asm discards this call's return IP, then returns to the
            // GOSUB continuation or tail-jumps through B$EVTRET for an event.
            return Ends::Leaves;
        }
    }
    let ends = ends_of(insn.flow()).unwrap_or(Ends::Leaves);
    // a far jump goes somewhere this module cannot follow
    if ends == Ends::Jump && insn.target().is_none() {
        return Ends::Leaves;
    }
    ends
}

/// The extent of the table after a call, and the offsets it holds.
pub fn inline_table(module: &Module, insn: &Insn) -> Option<(usize, usize, Vec<usize>)> {
    if !module.calls.get(&(insn.at as i64)).is_some_and(|name| INLINE_TABLE.contains(name.as_str())) {
        return None;
    }
    let count = module.code[insn.end()] as usize;
    let (lo, hi) = (insn.end(), insn.end() + 1 + count * 2);
    let entries: Vec<usize> = module
        .operands
        .keys()
        .map(|&at| at as usize)
        .filter(|&at| lo < at && at < hi)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Some((lo, hi, entries))
}

pub fn dispatch_targets(module: &Module, insn: &Insn) -> Option<Vec<i64>> {
    let limit = (module.end as usize).min(module.code.len());
    if insn.end() >= limit {
        return None;
    }
    let (lo, hi, fields) = inline_table(module, insn)?;
    if hi > limit || fields != (lo + 1..hi).step_by(2).collect::<Vec<usize>>() {
        return None;
    }
    let entries: Vec<_> = fields.iter().map(|&at| module.operands[&(at as i64)]).collect();
    if entries.iter().any(|entry| {
        entry.space != Space::Segment
            || entry.index != module.seg
            || !(module.start <= entry.disp && entry.disp < module.end)
    }) {
        return None;
    }
    Some(entries.iter().map(|entry| entry.disp).collect())
}

// MODULE_CODE in the QuickBASIC 4.5 runtime's addr.inc: a signature word, an
// eight-byte module name and nineteen more words, 48 bytes. The runtime calls
// the offset past it O_ENT, and rtinit.asm says the beginning of the user's
// code is at that fixed offset from the module header.
pub const SIGNATURES: [&[u8]; 3] = [b"bl", b"bm", b"br"];
pub const ENTRY: usize = 0x30;

// U_FLAG, the header's last word, records the switches BC was given. Under /V
// or /W the module opens with a jump over a sixteen-byte event-poll routine
// that only the runtime enters, at a fixed offset the same way the code is --
// so nothing falls into it and reachability cannot find it unaided.
pub const U_FLAG: usize = 0x2E;
pub const EVENTS: u16 = 0x400 | 0x800; // u_sw_v, u_sw_w

pub fn has_header(module: &Module) -> bool {
    SIGNATURES.contains(&slice(&module.code, 0, 2))
}

/// The compiler requested event checks, whether or not it emitted a stub.
pub fn event_enabled(module: &Module) -> bool {
    has_header(module)
        && module.code.len() >= U_FLAG + 2
        && u16::from_le_bytes([module.code[U_FLAG], module.code[U_FLAG + 1]]) & EVENTS != 0
}

// CMP word [b$EVTFLG],0 / JNE past / RET / POP AX / PUSH CS / PUSH AX / JMP FAR B$EVK1
pub const EVENT_ADAPTER: [&[Code]; 7] = [
    &[Code::Cmp_rm16_imm8, Code::Cmp_rm16_imm16],
    &[Code::Jne_rel8_16, Code::Jne_rel16],
    &[Code::Retnw],
    &[Code::Pop_r16],
    &[Code::Pushw_CS],
    &[Code::Push_r16],
    &[Code::Jmp_ptr1616],
];

/// Where the event-poll routine starts, in a module that carries one.
///
/// Identified by its instructions and relocations, not its place or
/// encoding: BC puts it just past the entry jump, a rebuilt segment may lay
/// it out anywhere and re-encode its compare.
pub fn event_stub(module: &Module) -> Option<usize> {
    if !event_enabled(module) {
        return None;
    }
    let names = omf::externals(&module.records);
    let mut fields: IndexMap<i64, Fixup> = IndexMap::default();
    for fixup in omf::fixups(&module.records) {
        if fixup.seg == Some(module.seg) {
            fields.insert(fixup.offset, fixup);
        }
    }

    let named = |at: i64, loc: i64, name: &str| -> bool {
        fields.get(&at).is_some_and(|fixup| {
            fixup.target == "external"
                && fixup.disp == 0
                && !fixup.selfrel
                && fixup.loc == loc
                && names[fixup.index as usize] == name
        })
    };

    let flags: BTreeSet<i64> = fields.keys().copied().filter(|&at| named(at, omf::LOC_OFF16, "b$EVTFLG")).collect();
    for flag in flags {
        let mut insns: Vec<Insn> = Vec::new();
        let mut at = usize::try_from(flag - 2).expect("a b$EVTFLG field inside the module header");
        let mut complete = true;
        for codes in EVENT_ADAPTER {
            let one = decode(&module.code, at);
            let Some(one) = one.filter(|one| codes.contains(&one.insn.code())) else {
                complete = false;
                break;
            };
            at = one.end();
            insns.push(one);
        }
        if complete {
            let [compare, skip, _ret, pop, _cs, push, far] = &insns[..] else { unreachable!() };
            if compare.insn.immediate(1) == 0
                && skip.insn.near_branch_target() == pop.at as u64
                && pop.insn.op0_register() == push.insn.op0_register()
                && push.insn.op0_register() == Register::AX
                && named(far.at as i64 + 1, omf::LOC_PTR32, "B$EVK1")
                && fields.keys().filter(|&&site| insns[0].at as i64 <= site && site < far.end() as i64).count() == 2
            {
                return Some(insns[0].at);
            }
        }
    }
    None
}

pub fn local_call_target(module: &Module, insn: &Insn) -> Option<usize> {
    let target = insn.target();
    if insn.flow() != FlowControl::Call
        || target.is_none()
        || module.sites.iter().any(|&site| insn.at as i64 <= site && site < insn.end() as i64)
    {
        return None;
    }
    let target = target? as i64;
    (module.start <= target && target < module.end).then_some(target as usize)
}

/// Every byte reachable as an instruction, from the entry points on.
pub fn walk(module: &Module, entry: usize, extra_entries: &BTreeSet<usize>) -> Result<CodeMap, String> {
    let mut seeds: BTreeSet<usize> = BTreeSet::from([entry]);
    seeds.extend(module.targets.iter().map(|&at| at as usize));
    seeds.extend(module.publics.iter().map(|&at| at as usize));
    seeds.extend(extra_entries.iter().copied());
    if let Some(stub) = event_stub(module) {
        seeds.insert(stub);
    }
    let entries: Vec<usize> = seeds.into_iter().collect();

    let mut starts: BTreeSet<usize> = BTreeSet::new();
    let mut leaders: BTreeSet<usize> = entries.iter().copied().collect();
    let declared = statement_table(module);
    let mut tables: Vec<(usize, usize)> = declared.into_iter().collect();
    let mut pending: Vec<usize> = entries;
    let (start, end) = (module.start as usize, module.end as usize);

    while let Some(popped) = pending.pop() {
        let mut at = popped;
        while start <= at && at < end && !starts.contains(&at) {
            if let Some((lo, hi)) = declared {
                if lo <= at && at < hi {
                    break;
                }
            }
            let Some(insn) = decode(&module.code, at) else {
                return Err(format!("the decoder gave up at {at:#x}, reached from an entry point"));
            };
            starts.insert(at);

            // A reachable private near call identifies code, but its callee
            // is not a CFG successor: execution also continues after the call.
            if let Some(target) = local_call_target(module, &insn) {
                leaders.insert(target);
                pending.push(target);
            }

            if let Some((lo, hi, held)) = inline_table(module, &insn) {
                if held.len() * 2 + 1 != hi - lo {
                    return Err(format!("the table after the call at {:#x} does not match its count", insn.at));
                }
                tables.push((lo, hi));
                // It comes back past the table, which is ON GOTO's out-of-range
                // fall-through: ON 0 GOTO ... runs the next statement.
                leaders.insert(hi);
                at = hi;
                continue;
            }

            let ends = terminator(&insn, Some(module));
            if matches!(ends, Ends::Conditional | Ends::Jump) {
                let target = insn.target().map(|target| target as usize);
                let Some(target) = target.filter(|&target| start <= target && target < end) else {
                    return Err(format!("a branch at {:#x} leaves the module", insn.at));
                };
                leaders.insert(target);
                pending.push(target);
            }
            if matches!(ends, Ends::Jump | Ends::Return | Ends::Leaves) {
                break;
            }
            if ends == Ends::Indirect {
                return Err(format!("an indirect jump at {:#x} has no computable target", insn.at));
            }
            if ends == Ends::Conditional {
                leaders.insert(insn.end());
            }
            at = insn.end();
        }
    }

    let unreached = gaps(module, &starts, &tables);
    tables.sort();
    Ok(CodeMap { starts, leaders, tables, unreached, procedures: extra_entries.clone() })
}

/// A disconnected, frame-based C procedure that exactly explains a gap.
///
/// Stripped Borland objects do not name an unreferenced static procedure in
/// PUBDEF, LINNUM or a debug segment. Its ABI entry still does: PUSH BP;
/// MOV BP,SP. Accept it only when a walk from that entry stays inside this
/// one previously unexplained range, every path is closed by a return, and
/// anything it leaves behind is inert.
pub fn native_gap_entry(
    module: &Module,
    found: &CodeMap,
    gap: (usize, usize),
    entry: usize,
    extra_entries: &BTreeSet<usize>,
) -> Option<usize> {
    let (lo, hi) = gap;
    let push = decode(&module.code, lo);
    let establish = push.as_ref().and_then(|push| decode(&module.code, push.end()));
    let (Some(push), Some(establish)) = (push, establish) else {
        return None;
    };
    if !(push.code() == Code::Push_r16
        && push.insn.op0_register() == Register::BP
        && establish.code() == Code::Mov_r16_rm16
        && establish.insn.op0_register() == Register::BP
        && establish.insn.op1_register() == Register::SP
        && push.end() == establish.at)
    {
        return None;
    }

    let mut entries = extra_entries.clone();
    entries.insert(lo);
    let candidate = walk(module, entry, &entries).ok()?;
    let added: BTreeSet<usize> = candidate.starts.difference(&found.starts).copied().collect();
    if added.is_empty() || added.iter().any(|&at| !(lo <= at && at < hi)) {
        return None;
    }

    let candidate_blocks: Vec<Block> =
        partition(module, &candidate).into_iter().filter(|block| added.contains(&block.at)).collect();
    if candidate_blocks.is_empty()
        || candidate_blocks.iter().any(|block| {
            block.succ.iter().any(|successor| !added.contains(successor))
                || (block.succ.is_empty() && block.ends != Ends::Return)
        })
    {
        return None;
    }
    if candidate
        .unreached
        .iter()
        .filter(|remaining| remaining.0 < hi && remaining.1 > lo)
        .any(|remaining| benign(module, (lo.max(remaining.0), hi.min(remaining.1))).is_none())
    {
        return None;
    }
    Some(lo)
}

/// Ranges that reachability never explained, so nothing may be moved across them.
pub fn gaps(module: &Module, starts: &BTreeSet<usize>, tables: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let end = module.end as usize;
    // bytearray slice assignment past the end only grows it, which no index
    // below `end` can see
    let mut covered = vec![false; end];
    let mut cover = |lo: usize, hi: usize| {
        for one in covered.iter_mut().take(hi.min(end)).skip(lo) {
            *one = true;
        }
    };
    for &at in starts {
        if let Some(insn) = decode(&module.code, at) {
            cover(at, insn.end());
        }
    }
    for &(lo, hi) in tables {
        cover(lo, hi);
    }

    let (mut out, mut run_from) = (Vec::new(), None);
    for at in module.start as usize..end {
        if !covered[at] && run_from.is_none() {
            run_from = Some(at);
        } else if covered[at] {
            if let Some(from) = run_from.take() {
                out.push((from, at));
            }
        }
    }
    if let Some(from) = run_from {
        out.push((from, end));
    }
    out
}

// How far in to look for where the header stops and code starts. Measured: the
// header's own fields carry fixups up to 0x20 and the earliest operand of an
// instruction is at 0x31, so the boundary is inside this.
pub const HEADER_SEARCH: usize = 0x40;

// A table has at least this many entries. Three is what ON GOTO's smallest form
// has, and a run that short at a constant stride does not happen by accident.
pub const SHORTEST_TABLE: usize = 3;

/// OF_STA points to an address/line table terminated by a zero word.
pub fn statement_table(module: &Module) -> Option<(usize, usize)> {
    if !has_header(module) {
        return None;
    }
    let reference = module.operands.get(&0x0A)?;
    if reference.space != Space::Segment
        || reference.index != module.seg
        || slice(&module.code, 0x0A, 0x0C) != b"\x00\x00"
    {
        return None;
    }
    let start = reference.disp;
    let mut at = start;
    if at < ENTRY as i64 {
        return None;
    }
    let inside = |lo: i64, hi: i64| module.sites.iter().any(|&site| lo <= site && site < hi);
    while at + 2 <= module.end {
        let code = |lo: i64, hi: i64| slice(&module.code, lo as usize, hi as usize);
        let Some(address) = module.operands.get(&at) else {
            if code(at, at + 2) == b"\x00\x00" && !inside(at, at + 2) {
                return Some((start as usize, (at + 2) as usize));
            }
            return None;
        };
        if address.space != Space::Segment
            || address.index != module.seg
            || at + 4 > module.end
            || code(at, at + 2) != b"\x00\x00"
            || inside(at + 1, at + 4)
        {
            return None;
        }
        at += 4;
    }
    None
}

/// Runs of relocations no instruction accounts for, which are a table.
///
/// Under /X, a map from statement to code offset so RESUME can find its way
/// back sits past the end of the code and the walk falls into it. What finds
/// it is the fixups: a run of them at a constant stride that no operand field
/// explains. Misalignment does not produce that.
pub fn unexplained_tables(module: &Module, fields: &BTreeSet<usize>, entry: usize) -> Vec<(usize, usize)> {
    // the header's own fields are below the entry and already exempt
    let missing: Vec<usize> = module
        .sites
        .iter()
        .map(|&site| site as usize)
        .filter(|site| *site >= entry && !fields.contains(site))
        .collect();
    let (mut found, mut start) = (Vec::new(), 0);
    while start < missing.len() {
        let (mut stop, mut stride) = (start + 1, None);
        while stop < missing.len() {
            let step = missing[stop] - missing[stop - 1];
            match stride {
                None => stride = Some(step),
                Some(stride) if step != stride => break,
                Some(_) => {}
            }
            stop += 1;
        }
        if let Some(stride) = stride {
            if stop - start >= SHORTEST_TABLE {
                found.push((missing[start], missing[stop - 1] + stride));
            }
        }
        start = stop;
    }
    found
}

/// Whether a byte range nothing reaches can be left where it is.
///
/// PDS under /Ot pads between procedure bodies with nops, and /V /W leaves
/// event-polling calls the optimiser jumps straight over. The test here is
/// only that the whole span is instruction-shaped, rather than accepting
/// arbitrary data because it happens to be unreachable.
pub fn benign(module: &Module, gap: (usize, usize)) -> Option<Vec<Insn>> {
    let (lo, hi) = gap;
    let bytes = slice(&module.code, lo, hi);
    if !bytes.is_empty() && bytes.iter().all(|&byte| byte == PAD) {
        return Some(Vec::new());
    }
    let (mut dead, mut at) = (Vec::new(), lo);
    while at < hi {
        let insn = decode(&module.code, at).filter(|insn| insn.end() <= hi)?;
        at = insn.end();
        dead.push(insn);
    }
    (at == hi).then_some(dead)
}

/// Whether every relocated field sits inside an operand of a reached instruction.
///
/// The fixups are BC's own map of where operand fields are, so they are
/// what says the alignment is right.
pub fn operand_fields(module: &Module, found: &CodeMap, dead: &[Insn]) -> Option<BTreeSet<usize>> {
    let mut fields = BTreeSet::new();
    let reached: Vec<Insn> = found.starts.iter().filter_map(|&at| decode(&module.code, at)).collect();
    if reached.len() != found.starts.len() {
        return None;
    }
    // A real instruction stream tiles. A decode that started inside the header
    // drifts into the middle of the first instruction and reports fragments that
    // overlap it, which is what an entry one or two bytes off looks like.
    if reached.windows(2).any(|pair| pair[0].end() > pair[1].at) {
        return None;
    }
    for insn in reached.iter().chain(dead) {
        if let Some(disp_at) = insn.disp_at {
            fields.extend(disp_at..disp_at + insn.disp_len);
        }
        if let Some(imm_at) = insn.imm_at {
            fields.extend(imm_at..imm_at + insn.imm_len);
        }
    }
    for &(lo, hi) in &found.tables {
        fields.extend(lo..hi);
    }
    Some(fields)
}

/// Where the code is: after the header, whose shape the runtime defines.
///
/// MODULE_CODE in QuickBASIC 4.5's runtime/inc/addr.inc is a fixed structure
/// whose fields sum to 48, and its first field is a signature word, so the
/// layout is checked rather than assumed. The search is still here for a
/// module with no signature, where there is nothing else to lean on.
pub fn code_map(module: &Module) -> Result<CodeMap, String> {
    let mut why = "no offset gives a decode the fixups agree with".to_owned();
    let mut best: Option<(usize, usize, CodeMap)> = None;

    let candidates: Vec<usize> = if has_header(module) { vec![ENTRY] } else { (0..HEADER_SEARCH).collect() };
    for entry in candidates {
        let native = !has_header(module) && omf::code_segment(&module.records).is_some();
        let mut extra_entries: BTreeSet<usize> = BTreeSet::new();
        let found = loop {
            let found = match walk(module, entry, &extra_entries) {
                Ok(found) => found,
                Err(error) => break Err(error),
            };
            let stranded_gap =
                found.unreached.iter().copied().find(|&gap| gap.0 >= entry && benign(module, gap).is_none());
            let Some(stranded_gap) = stranded_gap else {
                break Ok(found);
            };
            let discovered = if native {
                native_gap_entry(module, &found, stranded_gap, entry, &extra_entries)
            } else {
                None
            };
            let Some(discovered) = discovered else {
                why = format!("{:#x}..{:#x} is neither reached nor inert", stranded_gap.0, stranded_gap.1);
                break Err("unexplained code".to_owned());
            };
            extra_entries.insert(discovered);
        };
        let Ok(mut found) = found else {
            continue;
        };
        let dead: Vec<Insn> = found
            .unreached
            .iter()
            .filter(|gap| gap.0 >= entry)
            .flat_map(|&gap| benign(module, gap).unwrap_or_default())
            .collect();
        let Some(mut fields) = operand_fields(module, &found, &dead) else {
            continue;
        };
        let patches = fppatches::sites(module, &found.starts);
        let tables = unexplained_tables(module, &fields.union(&patches).copied().collect(), entry);
        for &(lo, hi) in &tables {
            fields.extend(lo..hi);
        }
        let accounted = |site: usize| fields.contains(&site) || patches.contains(&site);
        if !module.sites.iter().map(|&site| site as usize).filter(|&site| site >= entry).all(accounted) {
            continue;
        }
        if !tables.is_empty() {
            let covered: BTreeSet<usize> = tables.iter().flat_map(|&(lo, hi)| lo..hi).collect();
            found.starts = found.starts.difference(&covered).copied().collect();
            found.tables.extend(tables.iter().copied());
            found.tables.sort();
        }
        // Score by how many relocated fields the decode accounts for. An entry
        // too early reads header bytes as code; one too late skips real code and
        // leaves its operands unexplained. The count is what separates them, and
        // the largest entry breaks the tie, so the least data gets decoded.
        let explained = module.sites.iter().filter(|&&site| accounted(site as usize)).count();
        if best.as_ref().is_none_or(|best| (explained, entry) > (best.0, best.1)) {
            best = Some((explained, entry, found));
        }
    }

    if let Some(best) = best {
        return Ok(best.2);
    }

    // /V /W puts an event stub in the header region that no record names -- the
    // runtime finds it at a fixed offset -- so those four modules land here.
    Err(format!("no entry point explains the whole segment: {why}"))
}

/// Decode with the FP emulator's segment protocol restored.
pub fn decoded_instruction(module: &Module, at: usize) -> Option<Insn> {
    let insn = decode(&module.code, at)?;
    if slice(&module.code, at, at + 2) == b"\xcd\x3c"
        && insn.insn.code() != Code::Int_imm8
        && omf::externals(&module.records).iter().any(|name| name == "FIDRQQ")
    {
        // The emulator patches CD 3C D9 07 into 90 26 D9 07: ES, not DS.
        // Read out of QuickBASIC 4.5's own deedlines at 0824:A3F2, so the
        // protocol is the emulator's and not one dialect's.
        // Keep file offsets and length; only the virtual instruction changes.
        let mut native = insn.insn;
        native.set_segment_prefix(Register::ES);
        return Some(Insn { insn: native, ..insn });
    }
    Some(insn)
}

/// Every instruction the module actually reaches, in address order.
pub fn instructions(module: &Module) -> Result<Vec<Insn>, String> {
    let mapped = code_map(module)?;
    Ok(mapped.starts.iter().filter_map(|&at| decoded_instruction(module, at)).collect())
}

/// Port of `qbopt/frontend/blocks.py:Block`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub at: usize,
    pub end: usize,
    pub insns: Vec<Insn>,
    pub ends: Ends,
    pub succ: Vec<usize>,
}

impl Block {
    /// Whether control goes somewhere this cannot see.
    #[must_use]
    pub fn leaves(&self) -> bool {
        matches!(self.ends, Ends::Return | Ends::Leaves | Ends::Indirect) || self.succ.is_empty()
    }
}

/// The reached instructions cut into basic blocks, with their successors.
pub fn partition(module: &Module, mapped: &CodeMap) -> Vec<Block> {
    let reached: Vec<Insn> = mapped.starts.iter().filter_map(|&at| decoded_instruction(module, at)).collect();
    let mut out: Vec<Block> = Vec::new();
    let mut run: Vec<Insn> = Vec::new();

    for insn in reached {
        let at = insn.at;
        if !run.is_empty() && (mapped.leaders.contains(&at) || run.last().unwrap().end() != at) {
            out.push(_close(module, &run, mapped));
            run = Vec::new();
        }
        let closes = terminator(&insn, Some(module)) != Ends::FallsThrough || _table_at(module, mapped, &insn).is_some();
        run.push(insn);
        if closes {
            out.push(_close(module, &run, mapped));
            run = Vec::new();
        }
    }
    if !run.is_empty() {
        out.push(_close(module, &run, mapped));
    }
    out
}

pub fn _table_at(_module: &Module, mapped: &CodeMap, insn: &Insn) -> Option<(usize, usize)> {
    mapped.tables.iter().copied().find(|table| table.0 == insn.end())
}

pub fn _close(module: &Module, run: &[Insn], mapped: &CodeMap) -> Block {
    let last = run.last().unwrap();
    let mut ends = terminator(last, Some(module));
    let mut succ: Vec<i64> = Vec::new();

    if let Some(table) = _table_at(module, mapped, last) {
        let targets = dispatch_targets(module, last);
        succ = targets.unwrap_or_else(|| module.targets.iter().copied().collect());
        succ.push(table.1 as i64);
        ends = Ends::Table;
    } else if matches!(ends, Ends::Conditional | Ends::Jump) {
        succ = last.target().map(|target| target as i64).into_iter().collect();
        if ends == Ends::Conditional {
            succ.push(last.end() as i64);
        }
    } else if ends == Ends::FallsThrough {
        succ = vec![last.end() as i64];
    }

    let inside: Vec<i64> = succ.iter().copied().filter(|&at| module.start <= at && at < module.end).collect();
    let succ = if inside.len() == succ.len() {
        inside.into_iter().map(|at| at as usize).collect::<BTreeSet<usize>>().into_iter().collect()
    } else {
        Vec::new()
    };
    Block { at: run[0].at, end: last.end(), insns: run.to_vec(), ends, succ }
}

pub fn block_at(blocks: &[Block], offset: usize) -> Option<&Block> {
    blocks.iter().find(|block| block.at <= offset && offset < block.end)
}

/// `b[lo:hi]` for non-negative bounds: clamped, and empty where `hi < lo`.
fn slice(b: &[u8], lo: usize, hi: usize) -> &[u8] {
    let hi = hi.min(b.len());
    if lo >= hi { &[] } else { &b[lo..hi] }
}

#[cfg(test)]
#[path = "blocks_tests.rs"]
mod tests;

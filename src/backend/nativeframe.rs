//! Port of `qbopt/backend/nativeframe.py`.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::{Code, FlowControl, Register};
use indexmap::IndexMap;

use crate::frontend::blocks::Block;
use crate::frontend::declen::{Insn, WRITES, instruction_info_factory};
use crate::frontend::stack::touches_sp;
use crate::model::ir::{self, Loc, Space};
use crate::model::lir::LirBody;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub floor: i64,
    pub reserve_at: i64,
    pub saved: Vec<Register>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    pub entry: Entry,
    pub releases: BTreeSet<i64>,
    pub registers: Vec<(i64, Register)>,
    pub outgoing: BTreeSet<(i64, i64, u32)>,
    pub framed: bool,
    pub return_depth: i64,
}

impl Plan {
    /// The dataclass constructor with every later field at its default.
    #[must_use]
    pub fn new(entry: Entry, releases: BTreeSet<i64>, registers: Vec<(i64, Register)>) -> Self {
        Self {
            entry,
            releases,
            registers,
            outgoing: BTreeSet::new(),
            framed: true,
            return_depth: 2,
        }
    }
}

fn used_registers(insn: &Insn) -> Vec<(Register, iced_x86::OpAccess)> {
    instruction_info_factory()
        .info(&insn.insn)
        .used_registers()
        .iter()
        .map(|used| (used.register(), used.access()))
        .collect()
}

#[must_use]
pub fn bound(body: &LirBody, layout: &Plan) -> LirBody {
    let operand = |at: i64, where_: &Loc| -> Loc {
        if let Loc::Mem(memory) = where_ {
            if let Some(addr) = &memory.addr {
                if addr.space == Space::Frame && layout.outgoing.contains(&(at, addr.disp, memory.width)) {
                    let mut memory = memory.clone();
                    memory.stack_argument = true;
                    return Loc::Mem(memory);
                }
            }
        }
        where_.clone()
    };
    let mut body = body.clone();
    for block in &mut body.blocks {
        block.insns = block
            .insns
            .iter()
            .map(|one| match &one.what {
                Some(what) => {
                    let mut what = what.clone();
                    what.dests = what.dests.iter().map(|where_| operand(one.at, where_)).collect();
                    what.sources = what.sources.iter().map(|where_| operand(one.at, where_)).collect();
                    let mut replaced = (**one).clone();
                    replaced.what = Some(what);
                    Arc::new(replaced)
                }
                None => Arc::clone(one),
            })
            .collect();
    }
    body
}

#[must_use]
pub fn pins(body: &LirBody, layout: &Plan) -> IndexMap<u32, Register> {
    let registers: IndexMap<i64, Register> = layout.registers.iter().copied().collect();
    let mut result = IndexMap::new();
    for one in body.insns() {
        let Some(what) = &one.what else { continue };
        let Some(register) = registers.get(&one.at) else { continue };
        for operand in what.sources.iter().chain(&what.dests) {
            if let Loc::Held(held) = operand {
                result.insert(held.value, *register);
            }
        }
    }
    result
}

#[must_use]
pub fn balanced(blocks: &[Block], layout: &Plan, cleanup: &IndexMap<usize, i64>) -> bool {
    checked(blocks, layout, cleanup).is_some()
}

#[must_use]
pub fn checked(blocks: &[Block], layout: &Plan, cleanup: &IndexMap<usize, i64>) -> Option<Plan> {
    let by_start: IndexMap<usize, &Block> = blocks.iter().map(|block| (block.at, block)).collect();
    let first = blocks
        .iter()
        .find(|block| block.insns.iter().any(|insn| insn.at as i64 == layout.entry.reserve_at))?;
    let mut outgoing: BTreeSet<(i64, i64, u32)> = BTreeSet::new();
    let mut depths: IndexMap<usize, i64> = IndexMap::from([(first.at, layout.entry.floor)]);
    let mut pending = vec![first.at];
    while let Some(at) = pending.pop() {
        let block = by_start[&at];
        let mut depth = depths[&block.at];
        let mut active = !std::ptr::eq(block, first);
        let mut returned = false;
        for insn in &block.insns {
            active |= insn.at as i64 == layout.entry.reserve_at;
            if !active {
                continue;
            }
            if layout.releases.contains(&(insn.at as i64)) && depth != layout.entry.floor {
                return None;
            }
            let machine = &insn.insn;
            if machine.memory_base() == Register::BP {
                let displacement = (machine.memory_displacement64() & 0xFFFF) as i64;
                let displacement = if displacement >= 0x8000 { displacement - 0x10000 } else { displacement };
                if displacement < layout.entry.floor {
                    let width = machine.memory_size().size() as i64;
                    if machine.memory_index() != Register::None
                        || width == 0
                        || displacement < depth
                        || displacement + width > layout.entry.floor
                    {
                        return None;
                    }
                    outgoing.insert((insn.at as i64, displacement, width as u32));
                }
            }
            if insn.flow() == FlowControl::Return {
                if depth != layout.return_depth {
                    return None;
                }
                returned = true;
                break;
            }
            if matches!(insn.flow(), FlowControl::Call | FlowControl::IndirectCall) {
                match cleanup.get(&insn.at) {
                    Some(&amount) if amount >= 0 => depth += amount,
                    _ => return None,
                }
            } else if insn.code() == Code::Leavew {
                depth = 2;
            } else if matches!(
                insn.code(),
                Code::Add_rm16_imm8 | Code::Add_rm16_imm16 | Code::Sub_rm16_imm8 | Code::Sub_rm16_imm16
            ) && insn.insn.op0_register() == Register::SP
            {
                let amount = (insn.insn.immediate(1) & 0xFFFF) as i64;
                let amount = if amount >= 0x8000 { amount - 0x10000 } else { amount };
                depth += if matches!(insn.code(), Code::Add_rm16_imm8 | Code::Add_rm16_imm16) {
                    amount
                } else {
                    -amount
                };
            } else if insn.insn.stack_pointer_increment() != 0 {
                depth += i64::from(insn.insn.stack_pointer_increment());
            } else if touches_sp(insn)
                || used_registers(insn)
                    .iter()
                    .any(|(register, _)| matches!(register, Register::SP | Register::ESP))
            {
                return None;
            }
            if matches!(
                insn.flow(),
                FlowControl::Interrupt | FlowControl::IndirectBranch | FlowControl::Exception
            ) {
                return None;
            }
            if !matches!(insn.code(), Code::Leavew | Code::Pop_r16)
                && used_registers(insn).iter().any(|(register, access)| {
                    matches!(register, Register::BP | Register::EBP) && WRITES.contains(access)
                })
            {
                return None;
            }
        }
        if returned {
            continue;
        }
        if block.succ.is_empty() {
            return None;
        }
        for &successor in &block.succ {
            if !by_start.contains_key(&successor) {
                return None;
            }
            if let Some(&known) = depths.get(&successor) {
                if known != depth || successor == first.at {
                    return None;
                }
            } else {
                depths.insert(successor, depth);
                pending.push(successor);
            }
        }
    }
    Some(Plan {
        outgoing,
        ..layout.clone()
    })
}

#[must_use]
pub fn plan(blocks: &[Block], start: usize) -> Option<Plan> {
    let first = blocks.iter().find(|block| block.at == start)?;
    let Some(setup) = entry(&first.insns) else {
        return frameless(blocks, first);
    };
    let mut releases: BTreeSet<i64> = BTreeSet::new();
    let mut registers: Vec<(i64, Register)> = first
        .insns
        .iter()
        .filter(|insn| (insn.at as i64) < setup.reserve_at && insn.code() == Code::Push_r16)
        .map(|insn| (insn.at as i64, insn.insn.op0_register()))
        .collect();
    for block in blocks {
        for (index, insn) in block.insns.iter().enumerate() {
            if insn.flow() != FlowControl::Return {
                continue;
            }
            if index == 0 {
                return None;
            }
            let teardown = &block.insns[index - 1];
            if !(teardown.code() == Code::Leavew
                || (setup.floor == -2 * setup.saved.len() as i64
                    && teardown.code() == Code::Pop_r16
                    && teardown.insn.op0_register() == Register::BP))
            {
                return None;
            }
            let mut release = teardown.at as i64;
            if teardown.code() == Code::Pop_r16 {
                registers.push((teardown.at as i64, Register::BP));
            }
            let mut before = index as i64 - 2;
            for &register in &setup.saved {
                while before >= 0 && block.insns[before as usize].code() != Code::Pop_r16 {
                    let candidate = &block.insns[before as usize];
                    if candidate.flow() != FlowControl::Next
                        || used_registers(candidate).iter().any(|(used, _)| {
                            matches!(used, Register::SP | Register::ESP | Register::BP | Register::EBP)
                        })
                    {
                        return None;
                    }
                    before -= 1;
                }
                if before < 0 || block.insns[before as usize].insn.op0_register() != register {
                    return None;
                }
                release = block.insns[before as usize].at as i64;
                registers.push((release, register));
                before -= 1;
            }
            releases.insert(release);
        }
    }
    if releases.is_empty() {
        None
    } else {
        Some(Plan::new(setup, releases, registers))
    }
}

/// A call-free native leaf that uses neither BP nor an adjustable stack.
///
/// Such a body can be rebuilt but cannot acquire spill slots: BP still
/// belongs to its caller. `frame::Frame::slot` enforces that.
#[must_use]
pub fn frameless(blocks: &[Block], first: &Block) -> Option<Plan> {
    if first.insns.is_empty() {
        return None;
    }
    let mut saved: Vec<(Register, i64)> = Vec::new();
    let mut entry_pushes: BTreeSet<i64> = BTreeSet::new();
    let mut entry_registers: Vec<(i64, Register)> = Vec::new();
    let mut index = 0;
    let push_widths = |code: Code| match code {
        Code::Push_r16 => Some(2),
        Code::Push_r32 => Some(4),
        _ => None,
    };
    while index < first.insns.len() && push_widths(first.insns[index].code()).is_some() {
        let width = push_widths(first.insns[index].code()).expect("checked above");
        let register = first.insns[index].insn.op0_register();
        if !matches!(ir::root(register), Register::EBX | Register::ESI | Register::EDI)
            || saved
                .iter()
                .any(|(saved_register, _)| ir::root(*saved_register) == ir::root(register))
        {
            return None;
        }
        saved.push((register, width));
        entry_pushes.insert(first.insns[index].at as i64);
        entry_registers.push((first.insns[index].at as i64, register));
        index += 1;
    }
    if index == first.insns.len() {
        return None;
    }

    let mut returns: BTreeSet<i64> = BTreeSet::new();
    let mut releases: BTreeSet<i64> = BTreeSet::new();
    let mut restores: BTreeSet<i64> = BTreeSet::new();
    let mut registers = entry_registers.clone();
    for block in blocks {
        if block.succ.is_empty() && block.insns[block.insns.len() - 1].flow() != FlowControl::Return {
            return None;
        }
        for (at, insn) in block.insns.iter().enumerate() {
            if insn.flow() == FlowControl::Return {
                returns.insert(insn.at as i64);
                let mut before = at as i64 - 1;
                let mut release = insn.at as i64;
                for &(register, width) in &saved {
                    let pop_code = if width == 4 { Code::Pop_r32 } else { Code::Pop_r16 };
                    while before >= 0 && block.insns[before as usize].code() != pop_code {
                        let candidate = &block.insns[before as usize];
                        if candidate.flow() != FlowControl::Next || touches_sp(candidate) {
                            return None;
                        }
                        before -= 1;
                    }
                    if before < 0 || block.insns[before as usize].insn.op0_register() != register {
                        return None;
                    }
                    release = block.insns[before as usize].at as i64;
                    restores.insert(release);
                    registers.push((release, register));
                    before -= 1;
                }
                releases.insert(release);
            }
        }
    }
    for block in blocks {
        for insn in &block.insns {
            let at = insn.at as i64;
            if entry_pushes.contains(&at) || restores.contains(&at) || returns.contains(&at) {
                continue;
            }
            if matches!(insn.flow(), FlowControl::Call | FlowControl::IndirectCall) || touches_sp(insn) {
                return None;
            }
            if used_registers(insn).iter().any(|(register, _)| {
                matches!(register, Register::BP | Register::EBP | Register::SP | Register::ESP)
            }) {
                return None;
            }
        }
    }
    if returns.is_empty() {
        return None;
    }
    Some(Plan {
        framed: false,
        return_depth: 0,
        ..Plan::new(
            Entry {
                floor: -saved.iter().map(|(_, width)| width).sum::<i64>(),
                reserve_at: first.insns[index].at as i64,
                saved: saved.iter().map(|(register, _)| *register).collect(),
            },
            releases,
            registers,
        )
    })
}

#[must_use]
pub fn entry(insns: &[Insn]) -> Option<Entry> {
    if insns.len() < 3 {
        return None;
    }
    let (push, establish) = (&insns[0], &insns[1]);
    if push.code() != Code::Push_r16
        || push.insn.op0_register() != Register::BP
        || establish.code() != Code::Mov_r16_rm16
        || establish.insn.op0_register() != Register::BP
        || establish.insn.op1_register() != Register::SP
        || push.end() != establish.at
    {
        return None;
    }
    let (mut index, mut floor) = (2, 0_i64);
    let allocation = &insns[index];
    if matches!(allocation.code(), Code::Sub_rm16_imm8 | Code::Sub_rm16_imm16)
        && allocation.insn.op0_register() == Register::SP
    {
        let size = allocation.insn.immediate(1);
        if size > 0x7FFF || size % 2 != 0 {
            return None;
        }
        floor -= size as i64;
        index += 1;
    }
    let mut saved: Vec<Register> = Vec::new();
    while index < insns.len() && insns[index].code() == Code::Push_r16 {
        let register = insns[index].insn.op0_register();
        if !matches!(register, Register::BX | Register::SI | Register::DI) || saved.contains(&register) {
            return None;
        }
        saved.push(register);
        floor -= 2;
        index += 1;
    }
    if index == insns.len() || insns[..index].iter().zip(&insns[1..=index]).any(|(a, b)| a.end() != b.at) {
        return None;
    }
    Some(Entry {
        floor,
        reserve_at: insns[index].at as i64,
        saved,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::FlowControl;
    use indexmap::IndexMap;

    use super::{balanced, entry, plan};
    use crate::backend::{frame, prologue};
    use crate::frontend::blocks::{Block, Ends};
    use crate::frontend::declen::{self, Insn};
    use crate::model::lir::{self, LirBlock, LirBody};
    use crate::objectfile::omf;

    // r_walk-borland.obj's blocks, printed by Python's blocks.partition.
    const BLOCKS: &[(usize, usize, Ends, &[usize])] = &[
        (0, 60, Ends::FallsThrough, &[60]), (60, 74, Ends::FallsThrough, &[74]),
        (74, 99, Ends::Conditional, &[99, 102]), (99, 102, Ends::Jump, &[371]),
        (102, 126, Ends::Conditional, &[126, 129]), (126, 128, Ends::Jump, &[250]),
        (129, 154, Ends::Conditional, &[154, 202]), (154, 202, Ends::Jump, &[651]),
        (202, 250, Ends::Jump, &[651]), (250, 275, Ends::Conditional, &[275, 323]),
        (275, 323, Ends::Jump, &[651]), (323, 371, Ends::Jump, &[651]),
        (371, 398, Ends::Conditional, &[398, 401]), (398, 401, Ends::Jump, &[528]),
        (401, 426, Ends::Conditional, &[426, 477]), (426, 477, Ends::Jump, &[651]),
        (477, 527, Ends::Jump, &[651]), (528, 553, Ends::Conditional, &[553, 603]),
        (553, 603, Ends::Jump, &[651]), (603, 651, Ends::FallsThrough, &[651]),
        (651, 718, Ends::Conditional, &[718, 727]), (718, 727, Ends::Return, &[]),
        (727, 745, Ends::Conditional, &[745, 748]), (745, 748, Ends::Jump, &[74]), (748, 758, Ends::Return, &[]),
        (758, 820, Ends::Return, &[]), (820, 909, Ends::Conditional, &[909, 912]),
        (909, 912, Ends::Jump, &[1143]), (912, 923, Ends::Conditional, &[923, 941]),
        (923, 938, Ends::Conditional, &[938, 941]), (938, 941, Ends::Jump, &[1132]),
        (941, 974, Ends::Conditional, &[974, 977]), (974, 977, Ends::Jump, &[1132]),
        (977, 1021, Ends::Conditional, &[1021, 1056]), (1021, 1056, Ends::Conditional, &[1021, 1056]),
        (1056, 1066, Ends::Conditional, &[1066, 1121]), (1066, 1121, Ends::FallsThrough, &[1121]),
        (1121, 1132, Ends::Return, &[]), (1132, 1143, Ends::Return, &[]),
        (1143, 1177, Ends::Conditional, &[1177, 1180]), (1177, 1180, Ends::Jump, &[1536]),
        (1180, 1241, Ends::Conditional, &[1241, 1246]), (1241, 1246, Ends::Jump, &[1248]),
        (1246, 1248, Ends::FallsThrough, &[1248]), (1248, 1257, Ends::Conditional, &[1257, 1260]),
        (1257, 1260, Ends::Jump, &[1400]), (1260, 1294, Ends::Conditional, &[1294, 1349]),
        (1294, 1349, Ends::FallsThrough, &[1349]), (1349, 1400, Ends::Return, &[]),
        (1400, 1434, Ends::Conditional, &[1434, 1489]), (1434, 1489, Ends::FallsThrough, &[1489]),
        (1489, 1536, Ends::FallsThrough, &[1536]), (1536, 1540, Ends::Return, &[]),
        (1540, 1739, Ends::Return, &[]), (1739, 1752, Ends::Conditional, &[1752, 1757]),
        (1752, 1757, Ends::Jump, &[1759]), (1757, 1759, Ends::FallsThrough, &[1759]),
        (1759, 1763, Ends::Return, &[]),
    ];

    // extent.partition's bodies: seed and ranges.
    const BODIES: &[(usize, &[(usize, usize)])] = &[
        (1540, &[(1540, 1739)]),
        (1739, &[(1739, 1763)]),
        (0, &[(0, 128), (129, 527), (528, 758)]),
        (758, &[(758, 820)]),
        (820, &[(820, 1540)]),
    ];

    /// `blocks.partition(module, code_map(module))` for r_walk, decoded here.
    fn partition() -> Vec<Block> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/regressions/r_walk-borland.obj");
        let records = omf::read(path).unwrap();
        let (seg, _, size) = omf::code_segment(&records).unwrap();
        let code = omf::segment_image(&records, seg, size);
        assert_eq!(code.len(), 1763);
        BLOCKS
            .iter()
            .map(|&(at, end, ends, succ)| {
                let (insns, gave_up) = declen::run(&code, at, end);
                assert_eq!(gave_up, None);
                Block { at, end, insns, ends, succ: succ.to_vec() }
            })
            .collect()
    }

    fn instructions() -> Vec<Insn> {
        partition().into_iter().flat_map(|block| block.insns).collect()
    }

    fn owned(parts: &[Block], seed: usize) -> Vec<Block> {
        let ranges = BODIES.iter().find(|(start, _)| *start == seed).unwrap().1;
        parts
            .iter()
            .filter(|block| ranges.iter().any(|&(start, end)| start <= block.at && block.at < end))
            .cloned()
            .collect()
    }

    fn without(parts: &[Block], at: usize) -> Vec<Block> {
        parts
            .iter()
            .map(|block| Block { insns: block.insns.iter().filter(|insn| insn.at != at).cloned().collect(), ..block.clone() })
            .collect()
    }

    #[test]
    fn test_native_spills_start_below_locals_and_saved_registers() {
        let instructions = instructions();
        assert_eq!(instructions.len(), 757);
        for (start, floor, reserve) in [(0, -30, 8), (0x334, -50, 0x33C), (0x604, -76, 0x60C), (0x6CB, 0, 0x6CE)] {
            let from: Vec<Insn> = instructions.iter().filter(|insn| insn.at >= start).cloned().collect();
            let found = entry(&from).unwrap();
            assert_eq!(found.floor, floor);
            assert_eq!(found.reserve_at, reserve);
        }
    }

    #[test]
    fn test_native_frame_does_not_guess_missing_setup() {
        assert_eq!(entry(&instructions()[1..]), None);
    }

    #[test]
    fn test_native_spills_reserve_after_saves_and_release_before_pops() {
        let parts = owned(&partition(), 0x604);
        let plan = plan(&parts, 0x604).unwrap();
        assert_eq!(plan.releases, [0x6C5].into());
        let low = LirBody::new(
            "native",
            0x604,
            parts
                .iter()
                .map(|block| LirBlock {
                    succ: block.succ.iter().map(|&at| at as i64).collect(),
                    ..LirBlock::new(
                        block.at as i64,
                        block
                            .insns
                            .iter()
                            .map(|insn| {
                                Arc::new(lir::Insn::new(insn.at as i64, Some((insn.at as i64, insn.end() as i64)), None, vec![], vec![]))
                            })
                            .collect(),
                    )
                })
                .collect(),
            IndexMap::new(),
            IndexMap::new(),
        );
        let mut slots = frame::of(&low, None, "", Some(plan.clone())).unwrap();
        assert_eq!(slots.slot(1_i64, 4), Ok(-80));
        let changed = prologue::reserved(&low, &slots, None).unwrap();
        let adjustments: Vec<(i64, Option<String>)> = changed
            .insns()
            .iter()
            .filter(|one| one.frame_adjust)
            .map(|one| (one.at, one.what.as_ref().and_then(|what| what.name.clone())))
            .collect();
        assert_eq!(adjustments, [(0x60C, Some("sub".to_owned())), (0x6C5, Some("add".to_owned()))]);
        assert_eq!(super::plan(&without(&parts, 0x6C5), 0x604), None);
        let mut missing_anchor = low.clone();
        for block in &mut missing_anchor.blocks {
            block.insns.retain(|one| one.at != 0x6C5);
        }
        let Err(prologue::Refused(message)) = prologue::reserved(&missing_anchor, &slots, None) else {
            panic!("a lost release anchor is refused");
        };
        assert!(message.contains("release anchor"));
        assert!(balanced(&parts, &plan, &IndexMap::from([(0x6BF, 0)])));
        // Address folding changed allocation: POP's spill store shared its address
        // and the frame release gate refused a unique real restore as duplicated.
        let spill = Arc::new(lir::Insn::new(0x6C5, Some((0x6C5, 0x6C5)), None, vec![], vec![]));
        let mut expanded = low.clone();
        for block in &mut expanded.blocks {
            block.insns = block
                .insns
                .iter()
                .flat_map(|one| if one.at == 0x6C5 { vec![Arc::clone(one), Arc::clone(&spill)] } else { vec![Arc::clone(one)] })
                .collect();
        }
        let released = prologue::reserved(&expanded, &slots, None).unwrap();
        assert_eq!(released.insns().iter().filter(|one| one.frame_adjust).count(), 2);
        assert!(!balanced(&parts, &plan, &IndexMap::new()));
        assert!(!balanced(&parts, &plan, &IndexMap::from([(0x6BF, 2)])));
        assert!(!balanced(&without(&parts, 0x6C2), &plan, &IndexMap::from([(0x6BF, 0)])));
    }

    #[test]
    fn test_native_leaf_stack_balances_on_all_exits() {
        let all = partition();
        for start in [0, 0x2F6, 0x6CB] {
            let parts = owned(&all, start);
            let plan = plan(&parts, start).unwrap();
            assert!(parts.iter().all(|block| block.insns.iter().all(|insn| insn.flow() != FlowControl::Call)));
            assert!(balanced(&parts, &plan, &IndexMap::new()));
        }
    }
}

//! Port of `qbopt/backend/prologue.py`: reserving the frame the spiller
//! asked for, and giving it back.
//!
//! For a runtime-framed procedure, reserve after B$ENRA establishes BP and
//! release before B$EXSA tears it down. Refused where the body's exits
//! cannot all be found.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::backend::frame::{self as frames, Frame};
use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::{Exception, LIRTransform};

/// The frame cannot be grown safely on this body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

fn _native_anchor(block: &LirBlock, at: i64, role: &str) -> Result<usize, Refused> {
    let anchors: Vec<usize> = block
        .insns
        .iter()
        .enumerate()
        .filter(|(_, one)| one.at == at)
        .map(|(index, _)| index)
        .collect();
    let covered: Vec<usize> = anchors
        .iter()
        .copied()
        .filter(|&index| block.insns[index].covers.is_some_and(|covers| covers.0 < covers.1))
        .collect();
    if anchors.is_empty()
        || anchors != (anchors[0]..=anchors[anchors.len() - 1]).collect::<Vec<_>>()
        || covered.len() != 1
    {
        return Err(Refused(format!("native frame {role} anchor was lost or duplicated")));
    }
    Ok(anchors[0])
}

/// Python holds the one mutable frame every machine phase shares.
pub struct Prologue {
    pub frame: Rc<RefCell<Frame>>,
    pub calls: IndexMap<i64, String>,
}

impl Prologue {
    #[must_use]
    pub fn new(frame: Rc<RefCell<Frame>>, calls: Option<IndexMap<i64, String>>) -> Self {
        Self {
            frame,
            calls: calls.unwrap_or_default(),
        }
    }
}

impl LIRTransform for Prologue {
    fn class_name(&self) -> &'static str {
        "Prologue"
    }

    fn name(&self) -> &str {
        "prologue"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        reserved(&body, &self.frame.borrow(), Some(&self.calls)).map_err(|refused| refused.0)
    }

    fn transform_raising(&mut self, body: LirBody) -> Result<LirBody, Exception> {
        reserved(&body, &self.frame.borrow(), Some(&self.calls)).map_err(|refused| Exception::defined_in("qbopt.backend.prologue", "Refused", refused.0))
    }
}

fn _upper(calls: &IndexMap<i64, String>, at: i64) -> String {
    calls.get(&at).map_or_else(String::new, |name| name.to_uppercase())
}

/// `body` with sp lowered by what the frame grew, and put back.
pub fn reserved(
    body: &LirBody,
    frame: &Frame,
    calls: Option<&IndexMap<i64, String>>,
) -> Result<LirBody, Refused> {
    if frame.size() == 0 {
        return Ok(body.clone());
    }
    let arguments;
    let body = if frame.native.is_some() {
        arguments = _arguments(body, frame.size())?;
        &arguments
    } else {
        body
    };
    let empty = IndexMap::default();
    let calls = calls.unwrap_or(&empty);
    let entry = body.blocks.iter().find(|block| block.at == body.entry);
    let Some(entry) = entry.filter(|entry| !entry.insns.is_empty()) else {
        return Err(Refused(
            "the entry block has no instruction to put the prologue in front of".to_owned(),
        ));
    };
    let runtime_entry = entry.insns.iter().position(|one| {
        _upper(calls, one.at) == frames::ENTER
            && one.what.as_ref().is_none_or(|what| what.op == Operation::Call)
    });
    let mut leaves: Vec<(&LirBlock, usize)> = body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .insns
                .iter()
                .enumerate()
                .filter(|(_, one)| {
                    (_upper(calls, one.at) == frames::LEAVE
                        && one.what.as_ref().is_none_or(|what| what.op == Operation::Call))
                        || (runtime_entry.is_none()
                            && one.what.as_ref().is_some_and(|what| what.op == Operation::Return))
                })
                .map(move |(index, _)| (block, index))
        })
        .collect();
    let mut entry_index = runtime_entry.map_or(0, |index| index + 1);
    if let Some(native) = &frame.native {
        if runtime_entry.is_some() {
            return Err(Refused("native and runtime frame plans cannot be combined".to_owned()));
        }
        entry_index = _native_anchor(entry, native.entry.reserve_at, "reservation")?;
        leaves = Vec::new();
        for &at in &native.releases {
            for block in &body.blocks {
                if block.insns.iter().any(|one| one.at == at) {
                    leaves.push((block, _native_anchor(block, at, "release")?));
                }
            }
        }
        if leaves.len() != native.releases.len() {
            return Err(Refused("native frame release anchor was lost or duplicated".to_owned()));
        }
    }
    if leaves.is_empty() && !body.noreturn && !_ends_the_program(body, calls) {
        return Err(Refused(format!(
            "{} bytes of frame are wanted and this body has no return to give them back at",
            frame.size()
        )));
    }

    let take = _adjust(&entry.insns[0], -frame.size());
    let give: IndexMap<(i64, usize), Arc<Insn>> = leaves
        .iter()
        .map(|&(block, index)| ((block.at, index), _adjust(&block.insns[index], frame.size())))
        .collect();
    let mut out = body.clone();
    for block in &mut out.blocks {
        block.insns = _woven(
            block,
            if block.at == body.entry { Some(&take) } else { None },
            &give,
            entry_index,
        );
    }
    Ok(out)
}

fn _arguments(body: &LirBody, size: i64) -> Result<LirBody, Refused> {
    let operand = |where_: &Loc| -> Result<Loc, Refused> {
        if let Loc::Mem(memory) = where_ {
            if memory.stack_argument {
                if let Some(addr) = &memory.addr {
                    let displacement = addr.disp - size;
                    if displacement < -0x8000 {
                        return Err(Refused(
                            "outgoing argument exceeds the native frame displacement range".to_owned(),
                        ));
                    }
                    let mut memory = memory.clone();
                    memory.addr = Some(crate::model::ir::Addr {
                        disp: displacement,
                        ..addr.clone()
                    });
                    return Ok(Loc::Mem(memory));
                }
            }
        }
        Ok(where_.clone())
    };
    let mut out = body.clone();
    for block in &mut out.blocks {
        let mut insns = Vec::with_capacity(block.insns.len());
        for one in &block.insns {
            let Some(what) = &one.what else {
                insns.push(Arc::clone(one));
                continue;
            };
            let mut what = what.clone();
            what.dests = what.dests.iter().map(operand).collect::<Result<_, _>>()?;
            what.sources = what.sources.iter().map(operand).collect::<Result<_, _>>()?;
            let mut replaced = (**one).clone();
            replaced.what = Some(what);
            insns.push(Arc::new(replaced));
        }
        block.insns = insns;
    }
    Ok(out)
}

/// The block with the prologue in front and an epilogue before each return.
fn _woven(
    block: &LirBlock,
    take: Option<&Arc<Insn>>,
    give: &IndexMap<(i64, usize), Arc<Insn>>,
    entry_index: usize,
) -> Vec<Arc<Insn>> {
    let mut out = Vec::new();
    for (index, one) in block.insns.iter().enumerate() {
        if let Some(take) = take {
            if index == entry_index {
                let mut placed = (**take).clone();
                placed.at = one.at;
                placed.covers = Some((one.at, one.at));
                placed.op = one.op.clone();
                out.push(Arc::new(placed));
            }
        }
        if let Some(found) = give.get(&(block.at, index)) {
            out.push(Arc::clone(found));
        }
        out.push(Arc::clone(one));
    }
    out
}

/// `sub sp,N` or `add sp,N`, claiming none of the original bytes.
fn _adjust(beside: &Insn, by: i64) -> Arc<Insn> {
    let at = beside.covers.map_or(beside.at, |covers| covers.0);
    let name = if by > 0 { "add" } else { "sub" };
    let sp = Loc::Reg(Reg { register: Register::SP, width: 2 });
    let mut one = Insn::new(
        beside.at,
        Some((at, at)),
        Some(Semantics {
            name: Some(name.to_owned()),
            dests: vec![sp.clone()],
            sources: vec![sp, Loc::Imm(Imm { value: by.abs(), width: 2, address: None })],
            ..Semantics::new(Operation::Binary)
        }),
        Vec::new(),
        Vec::new(),
    );
    one.op = beside.op.clone();
    one.frame_adjust = true;
    Arc::new(one)
}

// The runtime call that ends the program. A body whose last instruction is
// this one never returns to anybody, so sp may be lowered and never put back.
pub static ENDS: LazyLock<BTreeSet<&'static str>> = LazyLock::new(|| BTreeSet::from(["B$CEND", "B$CENP"]));

/// Whether this body hands control to the runtime's exit and never returns.
///
/// Asked of the whole body: BC pads a code segment with zeros, and the last
/// instruction is routinely not a terminator at all.
fn _ends_the_program(body: &LirBody, calls: &IndexMap<i64, String>) -> bool {
    body.blocks
        .iter()
        .any(|block| block.insns.iter().any(|one| ENDS.contains(_upper(calls, one.at).as_str())))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;
    use crate::support::hash::IndexMap;

    use super::reserved;
    use crate::backend::frame::{self, Frame};
    use crate::model::ir::{Held, Imm, Loc, Operation, Semantics};
    use crate::model::lir::{Insn, LirBlock, LirBody};

    fn instruction(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Insn {
        Insn::new(
            at,
            Some((at, at + 1)),
            Some(Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }),
            vec![],
            vec![],
        )
    }

    fn procedure() -> LirBody {
        let held = Loc::Held(Held { value: 1, width: 2 });
        let mut entry = instruction(1, Operation::Call, "call", vec![], vec![held.clone()]);
        entry.requires = vec![(Held { value: 1, width: 2 }, Register::CX)];
        let insns = vec![
            instruction(0, Operation::Move, "mov", vec![held], vec![Loc::Imm(Imm { value: 6, width: 2, address: None })]),
            entry,
            instruction(2, Operation::Call, "call", vec![], vec![]),
            instruction(3, Operation::Return, "retf", vec![], vec![]),
        ];
        let block = LirBlock::new(0, insns.into_iter().map(Arc::new).collect());
        LirBody::new("procedure", 0, vec![block], IndexMap::default(), IndexMap::default())
    }

    fn runtime() -> IndexMap<i64, String> {
        IndexMap::from_iter([(1, "B$ENRA".to_owned()), (2, "B$EXSA".to_owned())])
    }

    #[test]
    fn test_spill_reservation_is_inside_the_runtime_frame() {
        let body = procedure();
        let mut slots = Frame::new(-16);
        slots.slot(1_i64, 2).unwrap();
        let result = reserved(&body, &slots, Some(&runtime())).unwrap();
        let names: Vec<_> = result.blocks[0]
            .insns
            .iter()
            .map(|one| one.what.as_ref().unwrap().name.clone().unwrap())
            .collect();
        assert_eq!(names, ["mov", "call", "sub", "add", "call", "retf"]);
    }

    #[test]
    fn test_slots_are_below_runtime_metadata_and_declared_locals() {
        let mut slots = frame::of(&procedure(), Some(&runtime()), "", None).unwrap();
        assert_eq!(slots.slot(9_i64, 2), Ok(-18));
    }

    #[test]
    fn test_explicit_end_needs_no_spill_frame_return() {
        // EVTRAP main refused four spill bytes after its END edge was corrected.
        let mut body = procedure();
        body.blocks[0].insns.truncate(3);
        let mut slots = Frame::new(-16);
        slots.slot(1_i64, 4).unwrap();
        let result = reserved(&body, &slots, Some(&IndexMap::from_iter([(2, "B$CEND".to_owned())]))).unwrap();
        assert_eq!(result.blocks[0].insns[0].what.as_ref().unwrap().name.as_deref(), Some("sub"));
    }
}

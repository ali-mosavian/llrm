//! Which 64-bit and counting operations a size-built module calls the runtime
//! for at the 68020 and later, by the bytes of the whole linked program.
//!
//! # How it works
//! [`chosen`] counts the sites of each [`Kind`] in the functions built for
//! size, asks [`llrm_codegen::backend::libcall_choice::choose`] with the
//! costs in `libcalls.tsv`, and [`apply`] sets `LibCall` (and `Expand`, for the
//! shift parts) for the kinds it returns. One use of a 64-bit count is
//! cheaper inline; a second makes the call cheaper, the routine being paid
//! once. A function built for speed never calls: the library is slower
//! (bitcount_u64 -O2: +8% clocks at the 68020, +18% at the 68060).
//!
//! A function with more kinds of site than [`MEASURED_KINDS`], or more
//! instructions than [`MEASURED_INSTRUCTIONS`], is not measured and stays
//! inline.
//!
//! A unit that defines a routine named in `libcalls.tsv` never calls it (the
//! body would recurse).
//!
//! DEVIATE M68K-SIZE-LIBCALL: see [`llrm_codegen::backend::libcall_choice`].
//! The 68000 and 68010 call the counts always (BC1, `dagisel/lowering.rs`);
//! their variable shifts are priced like the 68020's. Owner m68k.
//! Review: 2026-12-20.

use std::collections::HashMap;
use std::sync::LazyLock;

/// DEVIATE M68K-SIZE-LIBCALL-CAP: a function is built `1 + kinds` times to
/// price its sites (the three shifts count as one kind, see `grouped`), so the
/// work stays linear in the size of the functions with a site: one with more
/// kinds than this, or more instructions than the next constant, is not
/// measured and stays inline (the pin's choice). Owner m68k.
/// Review: 2026-12-20.
const MEASURED_KINDS: usize = 2;
const MEASURED_INSTRUCTIONS: usize = 1000;

use llrm_codegen::backend::libcall_choice::{Candidate, Costs, Member, choose};
use llrm_codegen::backend::selectiondag::actions::{Action, TargetLowering};
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::MVT;
use llrm_mir::context::ConstantKind;
use llrm_mir::intrinsics::{BitCount, Intrinsic};
use llrm_mir::module::{Function, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

use crate::dagisel::hooks::Cpu;

/// How a build chooses: by bytes (the default), or one way for everything
/// (`Options::size_libcalls`, which the size table is measured with).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Auto,
    Inline,
    Call,
}

impl Mode {
    pub fn parse(text: &str) -> Option<Mode> {
        match text {
            "auto" => Some(Mode::Auto),
            "inline" => Some(Mode::Inline),
            "call" => Some(Mode::Call),
            _ => None,
        }
    }
}

/// An operation that can be inline or a call.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Ctz64,
    Clz64,
    Pop32,
    Pop64,
    Shl64,
    Shr64,
    Sar64,
    Mul64,
}

const KINDS: [Kind; 8] =
    [Kind::Ctz64, Kind::Clz64, Kind::Pop32, Kind::Pop64, Kind::Shl64, Kind::Shr64, Kind::Sar64, Kind::Mul64];

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Ctz64 => "Ctz64",
            Kind::Clz64 => "Clz64",
            Kind::Pop32 => "Pop32",
            Kind::Pop64 => "Pop64",
            Kind::Shl64 => "Shl64",
            Kind::Shr64 => "Shr64",
            Kind::Sar64 => "Sar64",
            Kind::Mul64 => "Mul64",
        }
    }

    /// The actions that make this kind a call: the count's own, or the shift
    /// parts left to the legalizer, which then calls the routine (a
    /// shift by a constant stays inline).
    fn actions(self) -> &'static [(ISD, MVT, Action)] {
        match self {
            Kind::Ctz64 => &[(ISD::CtTzZeroPoison, MVT::I64, Action::LibCall)],
            Kind::Clz64 => &[(ISD::CtLzZeroPoison, MVT::I64, Action::LibCall)],
            Kind::Pop32 => &[(ISD::CtPop, MVT::I32, Action::LibCall)],
            Kind::Pop64 => &[(ISD::CtPop, MVT::I64, Action::LibCall)],
            Kind::Shl64 => &[(ISD::ShlParts, MVT::I32, Action::Expand)],
            Kind::Shr64 => &[(ISD::SrlParts, MVT::I32, Action::Expand)],
            Kind::Sar64 => &[(ISD::SraParts, MVT::I32, Action::Expand)],
            // a name, not an action: see [`apply`]
            Kind::Mul64 => &[],
        }
    }
}

static COSTS: LazyLock<Costs<Kind>> = LazyLock::new(|| parse(include_str!("libcalls.tsv")));

/// The table: `candidate KEY ROUTINE` and `member ROUTINE BYTES [NEEDS,..]`
/// lines, `#` comments.
pub fn parse(text: &str) -> Costs<Kind> {
    let mut costs = Costs { candidates: Vec::new(), members: HashMap::new() };
    for line in text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields[..] {
            ["candidate", key, routine] => costs.candidates.push(Candidate {
                key: KINDS
                    .into_iter()
                    .find(|kind| kind.name() == key)
                    .unwrap_or_else(|| panic!("libcalls.tsv: kind {key}")),
                routine: routine.to_owned(),
            }),
            ["member", routine, bytes] => {
                costs
                    .members
                    .insert(
                        routine.to_owned(),
                        Member { bytes: bytes.parse().expect("member bytes"), needs: Vec::new() },
                    );
            }
            ["member", routine, bytes, needs] => {
                costs
                    .members
                    .insert(
                        routine.to_owned(),
                        Member {
                            bytes: bytes.parse().expect("member bytes"),
                            needs: needs.split(',').map(str::to_owned).collect(),
                        },
                    );
            }
            _ => panic!("libcalls.tsv: {line}"),
        }
    }
    costs
}

/// The table's costs.
pub fn costs() -> &'static Costs<Kind> {
    &COSTS
}

/// The kinds with a site in `function`, each with its count of sites.
fn present(
    module: &Module,
    function: &Function,
) -> Vec<(Kind, usize)> {
    let mut found: Vec<(Kind, usize)> = Vec::new();
    let types = &module.context.types;
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        let bits = |operand: Operand| function.operand_type(&module.context, operand).and_then(|ty| types.int_bits(ty));
        let kind = match &instruction.opcode {
            Opcode::Call(_) => {
                let Some(Operand::Constant(id)) = instruction.operands.last().copied() else { continue };
                let ConstantKind::Global(global) = module.context.get(id).kind else { continue };
                match module.global(global).intrinsic() {
                    Some(Intrinsic::BitCount(count)) => match (count, bits(instruction.operands[0])) {
                        (BitCount::TrailingZeros, Some(64)) => Kind::Ctz64,
                        (BitCount::LeadingZeros, Some(64)) => Kind::Clz64,
                        (BitCount::Population, Some(32)) => Kind::Pop32,
                        (BitCount::Population, Some(64)) => Kind::Pop64,
                        _ => continue,
                    },
                    // the wide product the legalizer makes of these is an `i64`
                    // multiply
                    Some(Intrinsic::WithOverflow { op: BinaryOp::Mul, .. })
                    | Some(Intrinsic::Fixed { divide: false })
                    | Some(Intrinsic::FixedMul { .. }) => Kind::Mul64,
                    _ => continue,
                }
            }
            Opcode::Binary(BinaryOp::Mul) if bits(instruction.operands[0]) == Some(64) => Kind::Mul64,
            // a shift by a constant is expanded in line whatever the choice
            Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr))
                if bits(instruction.operands[0]) == Some(64)
                    && !matches!(instruction.operands[1], Operand::Constant(_)) =>
            {
                match op {
                    BinaryOp::Shl => Kind::Shl64,
                    BinaryOp::LShr => Kind::Shr64,
                    _ => Kind::Sar64,
                }
            }
            _ => continue,
        };
        match found.iter_mut().find(|(one, _)| *one == kind) {
            Some((_, count)) => *count += 1,
            None => found.push((kind, 1)),
        }
    }
    found
}

/// The kinds priced by one build: the three shifts share one (a function
/// with all of them is built twice, not four times), their gain split by
/// sites; every other kind is its own.
fn grouped(here: Vec<(Kind, usize)>) -> Vec<Vec<(Kind, usize)>> {
    let (shifts, others): (Vec<_>, Vec<_>) =
        here.into_iter().partition(|(kind, _)| matches!(kind, Kind::Shl64 | Kind::Shr64 | Kind::Sar64));
    let mut groups: Vec<Vec<(Kind, usize)>> = others.into_iter().map(|one| vec![one]).collect();
    if !shifts.is_empty() {
        groups.push(shifts);
    }
    groups
}

/// Whether `kind` is a choice at `cpu`. The counts are the 68020's (the 68000
/// and 68010 always call them, M68K-BC1); a variable shift is a choice of every
/// CPU (the expansion is the same code, and the 68000 and 68010 inline it
/// too); the product of two `i64`s is the choice of every CPU without a
/// `mulu.l` that gives 64 bits (68000, 68010, 68060): the 68020 to the 68040
/// take it before any call.
pub fn decided_at(
    kind: Kind,
    cpu: Cpu,
) -> bool {
    match kind {
        Kind::Mul64 => !cpu.has_wide_multiply(),
        Kind::Shl64 | Kind::Shr64 | Kind::Sar64 => true,
        _ => cpu.at_least(Cpu::M68020),
    }
}

/// The kinds to call in this module's size-built functions (`built_for_size`
/// says which), in `mode`. `bytes_of(name, kinds)` is the bytes of the
/// function built with `kinds` as calls: the saving of a kind is what the
/// functions with a site of it lose by it, summed, as measured.
pub fn chosen(
    module: &Module,
    cpu: Cpu,
    mode: Mode,
    built_for_size: impl Fn(&Function) -> bool,
    bytes_of: impl Fn(&str, &[Kind]) -> Result<usize, String>,
) -> Vec<Kind> {
    let mut kinds = match mode {
        Mode::Auto => {
            let mut saved: HashMap<Kind, i64> = HashMap::new();
            for (_, global, function) in module.functions().filter(|(_, _, one)| !one.is_declaration()) {
                let (Some(name), true) = (global.name.as_deref(), built_for_size(function)) else { continue };
                let here =
                    grouped(present(module, function).into_iter().filter(|&(kind, _)| decided_at(kind, cpu)).collect());
                if here.is_empty() || here.len() > MEASURED_KINDS || function.walk().count() > MEASURED_INSTRUCTIONS {
                    continue;
                }
                let Ok(inline) = bytes_of(name, &[]) else { continue };
                for group in here {
                    let kinds: Vec<Kind> = group.iter().map(|&(kind, _)| kind).collect();
                    let Ok(called) = bytes_of(name, &kinds) else { continue };
                    let gain = inline as i64 - called as i64;
                    let sites = group.iter().map(|&(_, count)| count as i64).sum::<i64>();
                    for (kind, count) in group {
                        *saved.entry(kind).or_default() += gain * count as i64 / sites;
                    }
                }
            }
            choose(&COSTS, &saved)
        }
        Mode::Inline => Vec::new(),
        Mode::Call => KINDS.into_iter().filter(|&kind| decided_at(kind, cpu)).collect(),
    };
    // the unit that defines a routine is its body: a call would recurse
    kinds.retain(|kind| {
        let routine = COSTS.candidates.iter().find(|one| one.key == *kind).map(|one| one.routine.as_str());
        !routine
            .and_then(|name| module.named(name))
            .is_some_and(
                |id| matches!(
                    &module.global(id).kind,
                    llrm_mir::GlobalKind::Function(function) if !function.is_declaration()
                ),
            )
    });
    kinds
}

/// Makes `kinds` calls in `tli`.
pub fn apply(
    tli: &mut TargetLowering,
    kinds: &[Kind],
) {
    for kind in kinds {
        for &(op, vt, action) in kind.actions() {
            tli.set_operation_action(op, vt, action);
        }
        if *kind == Kind::Mul64 {
            // the product of two `i64`s past a `mulu.l` (68000, 68010, 68060):
            // `expand_mul` calls it in a function built for size
            tli.set_size_only_libcall_name(ISD::Mul, MVT::I64, "__muldi3");
        }
    }
}

/// `kinds` as a bit set, for [`crate::compile::Options`] (which is `Copy`).
pub fn mask(kinds: &[Kind]) -> u8 {
    kinds.iter().fold(0, |bits, kind| bits | 1 << KINDS.iter().position(|one| one == kind).expect("a kind"))
}

/// The kinds of a [`mask`].
pub fn from_mask(bits: u8) -> Vec<Kind> {
    KINDS.into_iter().enumerate().filter(|(at, _)| bits >> at & 1 == 1).map(|(_, kind)| kind).collect()
}

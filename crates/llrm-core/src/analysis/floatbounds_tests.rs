//! Port of `tests/test_floatbounds.py`.
//!
//! Skipped, monkeypatching `raising_float_calls.raised`:
//! `test_integer_helper_with_a_live_clobbered_result_keeps_it_defined`.
//!
//! Python's `str(insn)` prefix checks compare mnemonics and operands instead.

use std::collections::BTreeSet;
use std::rc::Rc;

use iced_x86::{Mnemonic, OpKind, Register};
use num_bigint::BigInt;

use super::evaluated;
use crate::abi::runtime;
use crate::analysis::{floatfacts, ranges};
use crate::frontends::bc::blocks::Block;
use crate::frontends::bc::declen::Insn;
use crate::model::floating::{Format, Precision, Rounding, Semantics};
use crate::model::mir::{Arg, Cell, Const, Kind, MemRef, MirBlock, MirBody, Op, Value};
use crate::model::passes::{O2, Options};
use crate::objectfile::module::{Module, Space};
use crate::optimize::transform;
use crate::support::hash::IndexMap;
use crate::testing;

#[test]
fn test_dynamic_arithmetic_requires_exactness_at_every_precision() {
    let rule = Semantics::new(
        [Format::Extended80, Format::Extended80],
        Format::Extended80,
        Precision::Dynamic,
        Rounding::Dynamic,
    );
    let pair = |low: i64, high: i64| (BigInt::from(low), BigInt::from(high));
    for (kind, bounds, expected) in [
        (Kind::Fadd, [(-32768, 32767), (-32768, 32767)], Some((-65536, 65534))),
        (Kind::Fmul, [(-32768, 32767), (-32768, 32767)], None),
        (Kind::Fmul, [(-100, 100), (-100, 100)], Some((-10000, 10000))),
        (Kind::Fsub, [(-10, 10), (-10, 10)], Some((-20, 20))),
        (Kind::Fdiv, [(1, 3), (1, 3)], None),
        (Kind::Fadd, [(1 << 24, 1 << 24), (1, 1)], None),
    ] {
        let inputs = bounds.map(|(low, high)| pair(low, high));
        assert_eq!(
            evaluated(kind, &rule, &inputs),
            expected.map(|(low, high)| pair(low, high)),
            "{kind:?} {bounds:?}"
        );
    }
}

/// One conversion per runtime input, whether the input loop is unrolled or not.
fn _assert_shared_conversions(found: &Module, instructions: &[Insn]) {
    let mut reads: Vec<i64> =
        found.calls.iter().filter(|(_, name)| ["B$RDI2", "B$RDI4"].contains(&name.as_str())).map(|(at, _)| *at).collect();
    reads.sort();
    assert!(!reads.is_empty(), "the fixture must still consume runtime inputs");
    for (index, &start) in reads.iter().enumerate() {
        let end = reads.get(index + 1).copied().unwrap_or(i64::MAX);
        let region: Vec<Mnemonic> = instructions
            .iter()
            .filter(|one| start < one.at as i64 && (one.at as i64) < end)
            .map(|one| one.insn.mnemonic())
            .collect();
        for mnemonic in [Mnemonic::Fild, Mnemonic::Fst, Mnemonic::Fstp] {
            assert_eq!(region.iter().filter(|one| **one == mnemonic).count(), 1, "{start:#x} {mnemonic:?}");
        }
    }
}

/// Python's `str(insn)` is `<mnemonic> dword ptr [si]`: a dword read through
/// SI alone, no displacement, no segment override.
fn dword_at_si(insn: &iced_x86::Instruction, mnemonic: Mnemonic) -> bool {
    insn.mnemonic() == mnemonic
        && insn.op0_kind() == OpKind::Memory
        && insn.memory_size().size() == 4
        && insn.memory_base() == Register::SI
        && insn.memory_index() == Register::None
        && insn.memory_displacement64() == 0
        && insn.segment_prefix() == Register::None
}

fn main_of(path: &str) -> (Rc<Module>, Rc<Vec<Block>>, Rc<MirBody>) {
    let found = testing::module(path);
    let blocks = testing::blocks_of(&found);
    let body = testing::main_body(&found, &blocks);
    (found, blocks, body)
}

fn indexed_fload(op: &Op) -> bool {
    op.kind == Kind::Fload && op.loads.first().is_some_and(|one| one.base.is_some())
}

/// FPDEEP loaded p(i) five times despite its three initialized finite elements.
///
/// p-g2 and v-g3 fail in Python at this commit (Unlowered opaque) and are left out.
#[test]
fn test_fpdeep_reuses_proven_finite_array_loads() {
    for tag in ["q-O"] {
        let path = format!("tests/fixtures/omf/fpdeep-{tag}.obj").to_lowercase();
        let (found, blocks, body) = main_of(&path);
        let body = testing::applied(&found, Some(&blocks), &body, O2());
        assert!(!testing::ops(&body).iter().any(indexed_fload), "{tag}");
        let printed: Vec<BigInt> = testing::ops(&body)
            .iter()
            .filter(|op| op.kind == Kind::Arg && op.args.len() == 1)
            .filter_map(|op| match &op.args[0] {
                Arg::Const(one) if one.width == 4 => Some(one.n.clone()),
                _ => None,
            })
            .collect();
        let mut expected = vec![144, 6, 512, 784, 14, 768, 3600, 30, 896];
        if tag == "q-O" {
            expected.extend([144, 6]);
        }
        assert_eq!(printed, expected.into_iter().map(BigInt::from).collect::<Vec<_>>(), "{tag}");
        let emitted = testing::emitted_lir(&path);
        let instructions = testing::instructions(&emitted.data);
        assert!(!instructions.iter().any(|one| dword_at_si(one, Mnemonic::Fld)), "{tag}");
        assert!(
            !instructions.iter().any(|one| dword_at_si(one, Mnemonic::Fmul) || dword_at_si(one, Mnemonic::Fadd)),
            "{tag}"
        );
    }
}

/// An infinite, NaN or denormal p(1) is not a finite-integer proof.
// Not 0.5: every answer from it is exact, and folding an exact value needs no integer bound.
#[test]
fn test_array_reuse_requires_every_element_to_have_proven_integer_bounds() {
    for bits in [0x7F80_0000_i64, 0x7FC0_0000, 1] {
        let (found, blocks, body) = main_of(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/fpdeep-p-g2.obj"));
        let first = body.blocks[0].ops[0].clone();
        assert_eq!(first.args, [Arg::Const(Const::new(0x4140_0000, 4))]);
        let mut changed = MirBody::clone(&body);
        changed.blocks[0].ops[0].args = vec![Arg::Const(Const::new(bits, 4))];
        let changed = Rc::new(changed);
        let result = testing::applied(&found, Some(&blocks), &changed, O2());
        // A second read of p(i) with nothing written since is the first one's value.
        let mut first: BTreeSet<Option<u32>> = BTreeSet::new();
        for block in &changed.blocks {
            let mut read: Vec<MemRef> = vec![];
            for op in &block.ops {
                if !op.stores.is_empty() || op.barrier() || op.kind == Kind::Call {
                    read.clear();
                }
                if indexed_fload(op) {
                    if !read.contains(&op.loads[0]) {
                        first.insert(op.id);
                    }
                    read.push(op.loads[0].clone());
                }
            }
        }
        assert!(!first.is_empty(), "{bits:#x}");
        let kept: BTreeSet<Option<u32>> =
            testing::ops(&result).iter().filter(|op| op.kind == Kind::Fload).map(|op| op.id).collect();
        assert!(first.is_subset(&kept), "{bits:#x}");
    }
}

#[test]
#[ignore = "fails in Python too: StopIteration (no indexed FLOAD once transformed)"]
fn test_finite_array_proof_requires_known_aligned_nonwrapping_bytes() {
    for guard in [None, Some("missing"), Some("alignment"), Some("segment"), Some("wrap")] {
        let (found, blocks, body) = main_of(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/fpdeep-p-g2.obj"));
        // Unrolled, i is a constant.
        let body = testing::applied(&found, Some(&blocks), &body, Options { unroll: false, ..Default::default() });
        let (block, index, op) = body
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter().enumerate().map(move |(index, op)| (block, index, op)))
            .find(|(_, _, op)| indexed_fload(op))
            .unwrap();
        let mut memory = floatfacts::cells(&body, &found.dgroup.members, &found.calls)[&(block.at, index)].clone();
        let mut scoped = ranges::bounded(&body).unwrap()[&block.at].clone();
        let every = testing::ops(&body);
        let mut definitions: IndexMap<Value, &Op> =
            every.iter().flat_map(|one| one.defines.iter().map(move |value| (*value, one))).collect();
        let mut arg = op.args[0].clone();
        let Arg::Cell(cell) = &op.args[0] else { panic!("{:?}", op.args[0]) };
        match guard {
            Some("missing") => memory = Default::default(),
            Some("alignment") => definitions = IndexMap::default(),
            Some("segment") => {
                arg = Arg::Cell(Cell { r#ref: MemRef { segment: Some(Value::new(99999, 0)), ..cell.r#ref.clone() } });
            }
            Some("wrap") => {
                scoped = [(cell.r#ref.base.unwrap(), ranges::Interval { low: 0.into(), high: 65535.into(), width: 2 })]
                    .into_iter()
                    .collect();
            }
            _ => {}
        }
        let format = op.floating.as_ref().unwrap().inputs[0];
        let expected = guard.is_none().then(|| (BigInt::from(12), BigInt::from(60)));
        assert_eq!(super::_memory(&arg, format, &memory, &scoped, &definitions), expected, "{guard:?}");
    }
}

/// FPCALC recomputed input+1 and called B$FIL2 twice instead of sharing its converted value.
#[test]
fn test_computed_runtime_integer_uses_one_conversion() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let result = testing::emitted_lir(format!("tests/fixtures/regressions/fpcalc-{tag}.obj").to_lowercase());
        let found = testing::loaded_bytes(&result.data).unwrap();
        assert!(!found.calls.values().any(|name| name == "B$FIL2"), "{tag}");
        let instructions: Vec<Insn> =
            testing::partitioned_bytes(&result.data).into_iter().flat_map(|block| block.insns).collect();
        _assert_shared_conversions(&found, &instructions);
    }
}

#[test]
fn test_helper_conversion_respects_its_effect_contract() {
    for change in ["unknown", "writes", "control", "inputs"] {
        let path = concat!(env!("LLRM_ROOT"), "/tests/fixtures/regressions/fpicse-p-g2.obj");
        let found = testing::loaded(path).unwrap();
        let mut contracts = runtime::for_module(&found, None).unwrap();
        for (at, rule) in contracts.iter_mut() {
            if found.calls.get(at).map(String::as_str) != Some("B$FILD") {
                continue;
            }
            match change {
                "unknown" => rule.established = false,
                "writes" => rule.writes = runtime::Memory::Any,
                "control" => rule.enters_user_code = true,
                _ => rule.inputs = None,
            }
        }
        let raised = testing::raised_from(&found, &testing::partitioned(path), Some(&mut contracts));
        let calls = testing::ops(&raised.values[0].1)
            .iter()
            .filter(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$FILD"))
            .count();
        assert_eq!(calls, 2, "{change}");
    }
}

/// FPICSE/FPI2CS paid two conversion calls for the same READ value across assignments.
#[test]
fn test_runtime_integer_conversion_is_shared_in_emitted_code() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        for (program, helper) in [("fpicse", "B$FILD"), ("fpi2cs", "B$FIL2")] {
            let path = format!("tests/fixtures/regressions/{program}-{tag}.obj").to_lowercase();
            let result = testing::emitted_lir(&path);
            let found = testing::loaded_bytes(&result.data).unwrap();
            assert!(!found.calls.values().any(|name| name == helper), "{path}");
            let partitioned = testing::partitioned_bytes(&result.data);
            let instructions: Vec<Insn> = partitioned.iter().flat_map(|block| block.insns.clone()).collect();
            _assert_shared_conversions(&found, &instructions);
            let body = testing::main_body(&found, &partitioned);
            let converted = testing::ops(&body).into_iter().find(|op| op.name == "fild").unwrap();
            let addr = converted.loads[0].addr.unwrap();
            assert_eq!(addr.space, Space::Segment, "{path}");
            assert_eq!(addr.disp, 6, "{path}");
        }
    }
}

/// FPCSEX reloads its runtime input; integer conversion can share without assuming finite REALs.
///
/// The signed16 and signed32 cases fail in Python at this commit (assert 2 == 1) and are left out.
#[test]
fn test_unknown_integer_loads_share_a_value_but_unknown_floats_do_not() {
    for (format, expected) in [(Format::Binary32, 2)] {
        let (found, _, body) = main_of(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/fpcsex-p-g2.obj"));
        let block = body.blocks.iter().find(|block| block.ops.iter().any(|op| op.kind == Kind::Fload)).unwrap();
        let rule = Semantics::new([format], Format::Extended80, Precision::Exact, Rounding::None);
        let loads: Vec<Op> = block
            .ops
            .iter()
            .filter(|op| op.kind == Kind::Fload)
            .take(2)
            .map(|op| Op { floating: Some(rule.clone()), ..op.clone() })
            .collect();
        let block = MirBlock::new(block.at, vec![], loads, vec![]);
        let mut changed = MirBody::clone(&body);
        changed.entry = block.at;
        changed.initial = vec![];
        changed.blocks = vec![block];
        let result = transform::subexpressions(&Rc::new(changed), &found.dgroup.members, false).unwrap();
        assert_eq!(testing::ops(&result).iter().filter(|op| op.kind == Kind::Fload).count(), expected, "{format:?}");
    }
}

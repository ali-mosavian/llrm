//! Ports of the `tests/test_hir.py` cases that exercise `qbopt/hir` alone.
//!
//! Skipped: every case that parses BASIC, compiles, physicalizes or dumps
//! stages (`frontend/qb` and `tools/qbstages.py`, owned by the QB driver
//! port). `test_hir_lowers_whole_pointer_indirect_memory_without_machine_registers`
//! stops before its `physicalize` half; the modref case calls
//! `callmemory::annotated` directly, which `_alias_annotated` wraps.

use std::collections::BTreeSet;

use crate::support::hash::IndexMap;

use super::*;
use crate::backend::lower as lower_mir;
use crate::model::floating;
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg};
use crate::objectfile::module::Space;

fn int_type(id: i64, name: &str, width: i64) -> Type {
    Type { signed: Some(true), ..Type::new(id, name, TypeKind::Integer, width) }
}

fn float_type(id: i64, name: &str, width: i64) -> Type {
    Type { evaluation: FloatEvaluation::Extended80, ..Type::new(id, name, TypeKind::Float, width) }
}

fn value(id: i64, r#type: i64) -> Value {
    Value { id, r#type }
}

fn values(pairs: &[(i64, i64)]) -> Vec<Value> {
    pairs.iter().map(|(id, r#type)| value(*id, *r#type)).collect()
}

fn instruction(id: i64, op: Op, results: &[i64], operands: Vec<model::Operand>) -> Instruction {
    Instruction::new(id, op, results.to_vec(), operands)
}

fn returning(id: i64, instructions: Vec<Instruction>) -> Block {
    Block::new(id, instructions, Terminator::new(TerminatorKind::Return, vec![], vec![]))
}

fn with_parameters(function: Function, parameters: &[i64]) -> Function {
    Function { parameters: parameters.to_vec(), ..function }
}

fn vbdos(modules: Vec<Module>) -> Program {
    Program::new(Dialect::Vbdos, RuntimeProfile::Vbdos, modules)
}

fn indirect(base: i64, offset: i64, r#type: i64) -> model::Operand {
    model::Operand::IndirectPlace(IndirectPlace { base, offset, r#type, volatile: false, inbounds: false })
}

fn lowered_insns(name: &str, body: &mir::MirBody) -> usize {
    let occurrences = IndexMap::default();
    lower_mir::lowered(
        name,
        body,
        Some(&IndexMap::default()),
        BTreeSet::new(),
        Some(&IndexMap::default()),
        "386",
        lower_mir::Lowered { occurrences: Some(&occurrences), ..Default::default() },
    )
    .expect("lowers")
    .insns()
    .len()
}

fn program() -> Program {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let long = int_type(1, "long", 4);
    let single = float_type(2, "single", 4);
    let local = Place { extent: Some(4), ..Place::new(1, "total", 1, Storage::Local, -4) };
    let entry = Block::new(
        10,
        vec![
            instruction(1, Op::Copy, &[1], vec![model::Operand::constant(1, 40)]),
            instruction(2, Op::Copy, &[2], vec![model::Operand::constant(1, 2)]),
            instruction(3, Op::Add, &[3], vec![model::Operand::value_ref(1), model::Operand::value_ref(2)]),
            instruction(4, Op::Store, &[], vec![model::Operand::place_ref(1), model::Operand::value_ref(3)]),
            instruction(5, Op::Fadd, &[5], vec![model::Operand::value_ref(4), model::Operand::value_ref(4)]),
        ],
        Terminator::new(TerminatorKind::Branch, vec![model::Operand::value_ref(3)], vec![20, 30]),
    );
    let yes = Block::new(20, vec![], Terminator::new(TerminatorKind::Return, vec![model::Operand::value_ref(3)], vec![]));
    let no = Block::new(30, vec![], Terminator::new(TerminatorKind::Return, vec![model::Operand::constant(1, 0)], vec![]));
    let function = with_parameters(
        Function::new(1, "step", 1, values(&[(1, 1), (2, 1), (3, 1), (4, 2), (5, 2)]), vec![local], vec![entry, yes, no], 10),
        &[4],
    );
    vbdos(vec![Module::new(1, "render", vec![void, long, single], vec![function])])
}

fn ops(body: &mir::MirBody) -> Vec<&mir::Op> {
    body.blocks.iter().flat_map(|block| &block.ops).collect()
}

fn kinds(operations: &[&mir::Op]) -> Vec<mir::Kind> {
    operations.iter().map(|one| one.kind).collect()
}

#[test]
fn test_hir_json_is_deterministic_strict_and_replayable() {
    let text = encode(&program(), None).unwrap();
    assert_eq!(text, encode(&decode(&text).unwrap(), None).unwrap());
    assert!(text.contains("\"schema\":1"));
    let error = decode(&text.replace("\"schema\":1", "\"register\":\"eax\",\"schema\":1")).unwrap_err();
    assert!(error.0.contains("unknown fields"), "{error}");
    let error = decode(&text.replace("\"op\":\"add\"", "\"op\":\"adc\"")).unwrap_err();
    assert!(error.0.contains("unknown Op"), "{error}");
    let projection = mir_text(&lower(&decode(&text).unwrap()).unwrap()[0]);
    assert!(projection.contains("v3 <- v1:4 add v2:4"), "{projection}");
    assert!(projection.contains("branch -> b2"), "{projection}");
    assert!(projection.contains("[extended80,extended80->extended80;dynamic/dynamic]"), "{projection}");
}

#[test]
fn test_hir_verifier_rejects_incomplete_float_and_bad_cfg() {
    let source = program();
    let module = &source.modules[0];
    let bad_float = Type { evaluation: FloatEvaluation::None, ..module.types[2].clone() };
    let mut types = module.types[..2].to_vec();
    types.push(bad_float);
    let broken = Program { modules: vec![Module { types, ..module.clone() }], ..source.clone() };
    assert!(verify(&broken).unwrap_err().0.contains("has no evaluation format"));
    let function = &module.functions[0];
    let entry = Block {
        terminator: Terminator::new(TerminatorKind::Jump, vec![], vec![999]),
        ..function.blocks[0].clone()
    };
    let mut blocks = vec![entry];
    blocks.extend(function.blocks[1..].iter().cloned());
    let broken = Function { blocks, ..function.clone() };
    let broken = Program { modules: vec![Module { functions: vec![broken], ..module.clone() }], ..source.clone() };
    assert!(verify(&broken).unwrap_err().0.contains("unknown target"));
}

#[test]
fn test_hir_verifier_refuses_unsigned_division_over_signed_values() {
    // Unsigned source division once reached MIR as signed DIVMOD, changing
    // values above INT_MAX.
    let source = program();
    let module = &source.modules[0];
    let function = &module.functions[0];
    let mut entry = function.blocks[0].clone();
    entry.instructions[2].op = Op::Udiv;
    let mut blocks = vec![entry];
    blocks.extend(function.blocks[1..].iter().cloned());
    let broken = Function { blocks, ..function.clone() };
    let broken = Program { modules: vec![Module { functions: vec![broken], ..module.clone() }], ..source.clone() };
    assert!(verify(&broken).unwrap_err().0.contains("requires unsigned integer operands"));
}

#[test]
fn test_hir_verifier_rejects_a_store_with_the_wrong_value_type() {
    let source = program();
    let module = &source.modules[0];
    let function = &module.functions[0];
    let mut entry = function.blocks[0].clone();
    entry.instructions[3].operands = vec![model::Operand::place_ref(1), model::Operand::value_ref(5)];
    let mut blocks = vec![entry];
    blocks.extend(function.blocks[1..].iter().cloned());
    let broken = Function { blocks, ..function.clone() };
    let broken = Program { modules: vec![Module { functions: vec![broken], ..module.clone() }], ..source.clone() };
    assert!(verify(&broken).unwrap_err().0.contains("store value type"));
}

#[test]
fn test_hir_data_relocations_are_typed_and_bounded() {
    let source = program();
    let module = &source.modules[0];
    let literal = DataObject {
        readonly: true,
        relocations: vec![DataRelocation { at: 2, target: 7, addend: 4, address: AddressKind::Near }],
        ..DataObject::new(7, "$string7", vec![3, 0, 0, 0, 97, 98, 99])
    };
    let with = |object_: DataObject| Program {
        modules: vec![Module { data: vec![object_], ..module.clone() }],
        ..source.clone()
    };
    verify(&with(literal.clone())).unwrap();
    let bad = DataObject {
        relocations: vec![DataRelocation { at: 6, target: 7, addend: 0, address: AddressKind::Near }],
        ..literal
    };
    assert!(verify(&with(bad)).unwrap_err().0.contains("relocation exceeds initializer"));
}

#[test]
fn test_hir_lowers_long_float_memory_and_control_to_existing_mir() {
    let lowered = lower(&program()).unwrap().remove(0);
    assert_eq!(mir::verify(&lowered.body), Vec::<String>::new());
    let operations = ops(&lowered.body);
    assert_eq!(
        kinds(&operations[..5]),
        [mir::Kind::Copy, mir::Kind::Copy, mir::Kind::Add, mir::Kind::Store, mir::Kind::Fadd]
    );
    assert_eq!(
        operations[2].args,
        [
            Arg::Held(mir::Held { value: lowered.values[&1], width: 4 }),
            Arg::Held(mir::Held { value: lowered.values[&2], width: 4 })
        ]
    );
    assert_eq!(operations[3].args.len(), 1);
    assert!(matches!(operations[3].results[0], Arg::Cell(_)));
    let occurrences = IndexMap::default();
    let machine = lower_mir::lowered(
        "store",
        &lowered.body,
        Some(&IndexMap::default()),
        BTreeSet::new(),
        Some(&IndexMap::default()),
        "386",
        lower_mir::Lowered { occurrences: Some(&occurrences), ..Default::default() },
    )
    .unwrap();
    let insns = machine.insns();
    let store = insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .find(|what| {
            what.op == Operation::Move && matches!(what.dests.first(), Some(crate::model::ir::Loc::Mem(_)))
        });
    assert!(store.is_some());
    assert_eq!(operations[3].stores[0].width, 4);
    assert!(operations[3].stores[0].provenance.is_some());
    assert_eq!(operations[4].floating.as_ref().unwrap().result.as_str(), "extended80");
    assert_eq!(lowered.body.blocks[0].succ, [20, 30]);
    let entry_ops = &lowered.body.blocks[0].ops;
    assert_eq!(entry_ops[entry_ops.len() - 2].kind, mir::Kind::Sub);
    assert_eq!(entry_ops[entry_ops.len() - 1].test, Some(mir::Kind::Ne));
}

#[test]
fn test_hir_lowers_typed_array_index_to_whole_offset_arithmetic() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let long = int_type(1, "long", 4);
    let array =
        Type { element: Some(1), rank: 1, bounds: vec![(1, 10)], ..Type::new(2, "longs", TypeKind::Array, 40) };
    let items = Place { symbol: 7, extent: Some(40), ..Place::new(1, "items", 2, Storage::Module, 0) };
    let element = model::Operand::ArrayElement(ArrayElement { place: 1, indices: vec![model::Operand::value_ref(1)] });
    let block = returning(1, vec![instruction(1, Op::Load, &[2], vec![element])]);
    let function =
        with_parameters(Function::new(1, "lookup", 0, values(&[(1, 1), (2, 1)]), vec![items], vec![block], 1), &[1]);
    let module = Module {
        data: vec![DataObject::new(7, "$data", vec![0; 40])],
        ..Module::new(1, "array", vec![void, long, array], vec![function])
    };
    let source = vbdos(vec![module]);

    // The Rust frontend exposed this codec defect first: ArrayElement.indices
    // is a nested tagged union, which must survive the serialized HIR boundary.
    let source = decode(&encode(&source, None).unwrap()).unwrap();
    let lowered = lower(&source).unwrap().remove(0);
    let operations = &lowered.body.blocks[0].ops;
    assert_eq!(
        operations[..3].iter().map(|one| one.kind).collect::<Vec<_>>(),
        [mir::Kind::Sub, mir::Kind::Mul, mir::Kind::Load]
    );
    assert_eq!(operations[2].loads[0].base, Some(lowered.values[&4]));
    assert!(operations[2].loads[0].provenance.is_some());
    assert_eq!(lowered.name, "array.lookup");
    assert!(lowered_insns(&lowered.name, &lowered.body) > 0);
}

#[test]
fn test_canonical_mir_dump_keeps_call_identity() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let call_instruction = Instruction { callee: Some("TWICE&".to_owned()), ..instruction(1, Op::Call, &[], vec![]) };
    let block = returning(1, vec![call_instruction]);
    let call = CallAbi { instruction: 1, order: vec![], cleanup: StackCleanup::Callee, distance: CallDistance::Far, callee: None };
    let function = Function { calls: vec![call], ..Function::new(1, "caller", 0, vec![], vec![], vec![block], 1) };
    let source = vbdos(vec![Module::new(1, "calls", vec![void], vec![function])]);
    assert!(mir_text(&lower(&source).unwrap()[0]).contains("call TWICE&()"));
}

#[test]
fn test_hir_lowers_whole_pointer_indirect_memory_without_machine_registers() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let long = int_type(1, "long", 4);
    let pointer =
        Type { element: Some(1), address: AddressKind::Huge, ..Type::new(2, "huge*long", TypeKind::Pointer, 4) };
    let block = returning(1, vec![instruction(1, Op::Load, &[2], vec![indirect(1, 0, 1)])]);
    let function = with_parameters(Function::new(1, "read", 0, values(&[(1, 2), (2, 1)]), vec![], vec![block], 1), &[1]);
    let source = vbdos(vec![Module::new(1, "pointer", vec![void, long, pointer], vec![function])]);
    let semantic = lower(&decode(&encode(&source, None).unwrap()).unwrap()).unwrap().remove(0);
    let operation = &semantic.body.blocks[0].ops[0];
    assert_eq!(operation.kind, mir::Kind::Load);
    assert!(operation.loads[0].pointer);
    assert_eq!(operation.loads[0].base_width, 4);
    assert_eq!(operation.loads[0].addr, None);
}

#[test]
fn test_hir_lowers_whole_pointer_field_offset_before_memory_access() {
    // QBSP expanded each far UDT field into a complete huge-pointer
    // correction. A FAR pointer advances only its offset word.
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let aggregate = Type::new(2, "pair", TypeKind::Opaque, 4);
    let pointer =
        Type { element: Some(2), address: AddressKind::Far, ..Type::new(3, "far*pair", TypeKind::Pointer, 4) };
    let block = returning(1, vec![instruction(1, Op::Load, &[2], vec![indirect(1, 2, 1)])]);
    let function = with_parameters(Function::new(1, "field", 0, values(&[(1, 3), (2, 1)]), vec![], vec![block], 1), &[1]);
    let source = vbdos(vec![Module::new(1, "pointer", vec![void, integer, aggregate, pointer], vec![function])]);
    let body = lower(&source).unwrap().remove(0).body;
    let operations = &body.blocks[0].ops;
    assert_eq!(
        operations[..4].iter().map(|one| one.kind).collect::<Vec<_>>(),
        [mir::Kind::Extract, mir::Kind::Extract, mir::Kind::Add, mir::Kind::Load]
    );
    let reference = &operations[3].loads[0];
    assert_eq!(reference.addr.map(|addr| addr.space), Some(Space::Far));
    let held = |one: &Arg| match one {
        Arg::Held(held) => held.value,
        _ => panic!("a held result"),
    };
    assert_eq!(reference.base, Some(held(&operations[2].results[0])));
    assert_eq!(reference.segment, Some(held(&operations[1].results[0])));
    assert!(!reference.pointer);
}

#[test]
fn test_huge_pointer_field_offset_retains_selector_normalization() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let aggregate = Type::new(2, "pair", TypeKind::Opaque, 4);
    let pointer =
        Type { element: Some(2), address: AddressKind::Huge, ..Type::new(3, "huge*pair", TypeKind::Pointer, 4) };
    let block = returning(1, vec![instruction(1, Op::Load, &[2], vec![indirect(1, 2, 1)])]);
    let function = with_parameters(Function::new(1, "field", 0, values(&[(1, 3), (2, 1)]), vec![], vec![block], 1), &[1]);
    let source = Program::new(
        Dialect::Pds71,
        RuntimeProfile::Pds71,
        vec![Module::new(1, "pointer", vec![void, integer, aggregate, pointer], vec![function])],
    );

    let body = lower(&source).unwrap().remove(0).body;
    let operations = &body.blocks[0].ops;

    assert_eq!(operations[..2].iter().map(|one| one.kind).collect::<Vec<_>>(), [mir::Kind::PtrOffset, mir::Kind::Load]);
    assert!(operations[1].loads[0].pointer);
}

#[test]
fn test_qb_module_instantiates_user_callee_modref_on_pointer_actuals() {
    // RPOINTLEAF treated readonly RPLANEDIST as a write to every descriptor.
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let pointer =
        Type { element: Some(1), address: AddressKind::Near, ..Type::new(2, "near*integer", TypeKind::Pointer, 2) };
    let read_call =
        Instruction { callee: Some("READ".to_owned()), ..instruction(1, Op::Call, &[], vec![model::Operand::value_ref(1)]) };
    let caller = Function {
        parameters: vec![1],
        calls: vec![CallAbi {
            instruction: 1,
            order: vec![0],
            cleanup: StackCleanup::Callee,
            distance: CallDistance::Far,
            callee: Some(1),
        }],
        ..Function::new(1, "CALLER", 0, values(&[(1, 2)]), vec![], vec![returning(1, vec![read_call])], 1)
    };
    let callee = with_parameters(
        Function::new(
            2,
            "READ",
            0,
            values(&[(1, 2), (2, 1)]),
            vec![],
            vec![returning(1, vec![instruction(1, Op::Load, &[2], vec![indirect(1, 0, 1)])])],
            1,
        ),
        &[1],
    );
    let module = Module {
        callables: vec![model::Callable {
            id: 1,
            name: "READ".to_owned(),
            result_type: None,
            parameter_types: vec![1],
            by_value: vec![false],
            segmented: vec![false],
            arrays: vec![false],
            defined: true,
        }],
        ..Module::new(1, "modref", vec![void, integer, pointer], vec![caller, callee])
    };
    let source = vbdos(vec![module.clone()]);

    let bodies = callmemory::annotated(&module, &module.functions, &lower(&source).unwrap(), None).unwrap();
    let call = ops(&bodies[0].body).into_iter().find(|one| one.kind == mir::Kind::Call).unwrap();

    assert!(call.memory_complete);
    assert!(!call.loads.is_empty());
    assert!(call.stores.is_empty());
}

#[test]
fn test_far_float_access_splits_selector_and_offset_for_x87_memory() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let single = float_type(1, "single", 4);
    let pointer =
        Type { element: Some(1), address: AddressKind::Far, ..Type::new(2, "far*single", TypeKind::Pointer, 4) };
    let block = returning(1, vec![instruction(1, Op::Load, &[2], vec![indirect(1, 0, 1)])]);
    let function =
        with_parameters(Function::new(1, "far_float", 0, values(&[(1, 2), (2, 1)]), vec![], vec![block], 1), &[1]);
    let source = vbdos(vec![Module::new(1, "float", vec![void, single, pointer], vec![function])]);
    let body = lower(&source).unwrap().remove(0).body;
    assert_eq!(
        body.blocks[0].ops[..3].iter().map(|one| one.kind).collect::<Vec<_>>(),
        [mir::Kind::Extract, mir::Kind::Extract, mir::Kind::Fload]
    );
    let reference = &body.blocks[0].ops[2].loads[0];
    assert_eq!(reference.addr.map(|addr| addr.space), Some(Space::Far));
    assert!(reference.segment.is_some());
    assert!(lowered_insns("far_float", &body) > 0);
}

#[test]
fn test_far_float_compare_fuses_the_comparison_not_an_inserted_extract() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let single = float_type(1, "single", 4);
    let pointer =
        Type { element: Some(1), address: AddressKind::Far, ..Type::new(2, "far*single", TypeKind::Pointer, 4) };
    let boolean = Type { signed: Some(true), ..Type::new(3, "boolean", TypeKind::Boolean, 2) };
    let blocks = vec![
        Block::new(
            1,
            vec![
                instruction(1, Op::Load, &[3], vec![indirect(1, 0, 1)]),
                instruction(2, Op::Lt, &[4], vec![model::Operand::value_ref(2), model::Operand::value_ref(3)]),
            ],
            Terminator::new(TerminatorKind::Branch, vec![model::Operand::value_ref(4)], vec![2, 3]),
        ),
        returning(2, vec![]),
        returning(3, vec![]),
    ];
    let function = with_parameters(
        Function::new(1, "far_compare", 0, values(&[(1, 2), (2, 1), (3, 1), (4, 3)]), vec![], blocks, 1),
        &[1, 2],
    );
    let source = vbdos(vec![Module::new(1, "float", vec![void, single, pointer, boolean], vec![function])]);
    let body = lower(&source).unwrap().remove(0).body;
    assert_eq!(
        body.blocks[0].ops.iter().map(|one| one.kind).collect::<Vec<_>>(),
        [mir::Kind::Extract, mir::Kind::Extract, mir::Kind::Fload, mir::Kind::Fcompare, mir::Kind::Branch]
    );
    assert!(lowered_insns("far_compare", &body) > 0);
}

#[test]
fn test_branch_comparison_reaches_existing_flag_form() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let boolean = Type { signed: Some(true), ..Type::new(2, "boolean", TypeKind::Boolean, 2) };
    let entry = Block::new(
        1,
        vec![instruction(1, Op::Eq, &[3], vec![model::Operand::value_ref(1), model::Operand::value_ref(2)])],
        Terminator::new(TerminatorKind::Branch, vec![model::Operand::value_ref(3)], vec![2, 3]),
    );
    let blocks = vec![entry, returning(2, vec![]), returning(3, vec![])];
    let function =
        with_parameters(Function::new(1, "compare", 0, values(&[(1, 1), (2, 1), (3, 2)]), vec![], blocks, 1), &[1, 2]);
    let source = vbdos(vec![Module::new(1, "flags", vec![void, integer, boolean], vec![function])]);
    let body = lower(&source).unwrap().remove(0).body;
    assert_eq!(body.blocks[0].ops.iter().map(|one| one.kind).collect::<Vec<_>>(), [mir::Kind::Sub, mir::Kind::Branch]);
    assert_eq!(body.blocks[0].ops.last().unwrap().test, Some(mir::Kind::Eq));
    assert!(lowered_insns("compare", &body) > 0);
}

#[test]
fn test_comparison_used_as_a_value_is_materialized_as_qb_minus_one_or_zero() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let boolean = Type { signed: Some(true), ..Type::new(2, "boolean", TypeKind::Boolean, 2) };
    let block = Block::new(
        1,
        vec![
            instruction(1, Op::Ge, &[3], vec![model::Operand::value_ref(1), model::Operand::value_ref(2)]),
            instruction(2, Op::And, &[4], vec![model::Operand::value_ref(3), model::Operand::constant(2, -1)]),
        ],
        Terminator::new(TerminatorKind::Branch, vec![model::Operand::value_ref(4)], vec![2, 3]),
    );
    let function = with_parameters(
        Function::new(
            1,
            "boolean_value",
            0,
            values(&[(1, 1), (2, 1), (3, 2), (4, 2)]),
            vec![],
            vec![block, returning(2, vec![]), returning(3, vec![])],
            1,
        ),
        &[1, 2],
    );
    let source = vbdos(vec![Module::new(1, "bool", vec![void, integer, boolean], vec![function])]);
    let body = lower(&source).unwrap().remove(0).body;
    let all = kinds(&ops(&body));
    assert!(!all.contains(&mir::Kind::Ge));
    assert_eq!(all.iter().filter(|one| **one == mir::Kind::Store).count(), 2);
    assert!(all.contains(&mir::Kind::Load));
    assert!(lowered_insns("boolean_value", &body) > 0);
}

#[test]
fn test_remainder_lowers_as_existing_divmod_pair() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let block = returning(
        1,
        vec![instruction(1, Op::Rem, &[3], vec![model::Operand::value_ref(1), model::Operand::value_ref(2)])],
    );
    let function =
        with_parameters(Function::new(1, "modulo", 0, values(&[(1, 1), (2, 1), (3, 1)]), vec![], vec![block], 1), &[1, 2]);
    let source = vbdos(vec![Module::new(1, "divide", vec![void, integer], vec![function])]);
    let lowered = lower(&source).unwrap();
    let operation = &lowered[0].body.blocks[0].ops[0];
    assert_eq!(operation.kind, mir::Kind::Divmod);
    assert_eq!(operation.results.len(), 2);
}

#[test]
fn test_integer_to_float_conversion_uses_x87_storage_load() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let single = float_type(2, "single", 4);
    let temporary = Place { extent: Some(2), ..Place::new(1, "$convert", 1, Storage::Local, -2) };
    let block = returning(
        1,
        vec![
            instruction(1, Op::Store, &[], vec![model::Operand::place_ref(1), model::Operand::constant(1, 7)]),
            instruction(2, Op::Convert, &[1], vec![model::Operand::place_ref(1)]),
        ],
    );
    let function = Function::new(1, "to_single", 0, values(&[(1, 2)]), vec![temporary], vec![block], 1);
    let source = vbdos(vec![Module::new(1, "convert", vec![void, integer, single], vec![function])]);
    let body = lower(&source).unwrap().remove(0).body;
    let conversion = &body.blocks[0].ops[1];
    assert_eq!(conversion.kind, mir::Kind::Fload);
    assert!(conversion.floating.is_some());
    assert!(lowered_insns("to_single", &body) > 0);
}

#[test]
fn test_float_conversions_and_negation_match_encodable_x87_semantics() {
    let void = Type::new(0, "void", TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let single = float_type(2, "single", 4);
    let double = float_type(3, "double", 8);
    let block = returning(
        1,
        vec![
            instruction(1, Op::Fneg, &[2], vec![model::Operand::value_ref(1)]),
            instruction(2, Op::Convert, &[3], vec![model::Operand::value_ref(2)]),
            instruction(3, Op::Convert, &[5], vec![model::Operand::value_ref(3)]),
            instruction(4, Op::Convert, &[4], vec![model::Operand::value_ref(5)]),
        ],
    );
    let function = with_parameters(
        Function::new(1, "float_convert", 0, values(&[(1, 3), (2, 3), (3, 2), (4, 1), (5, 3)]), vec![], vec![block], 1),
        &[1],
    );
    let source = vbdos(vec![Module::new(1, "float", vec![void, integer, single, double], vec![function])]);
    let body = lower(&source).unwrap().remove(0).body;
    let operations = &body.blocks[0].ops;
    assert_eq!(operations[0].floating.as_ref().unwrap().precision, floating::Precision::Exact);
    assert_eq!(operations[1..3].iter().map(|one| one.kind).collect::<Vec<_>>(), [mir::Kind::Fstore, mir::Kind::Fload]);
    assert!(operations.iter().all(|one| one.kind != mir::Kind::Copy));
    assert_eq!(operations[3].kind, mir::Kind::Fstore);
    assert!(lowered_insns("float_convert", &body) > 0);
}

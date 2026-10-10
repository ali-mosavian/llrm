//! Port of `tests/test_consts.py` and `tests/test_constant_cells.py`.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Module, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_support::hash::IndexMap;
use num_bigint::BigInt;

use super::{_MemoryQueries, _result, Calls, Known, division, initialized, known, known_walked, masked};
use crate::memory::{Addr, MemRef, MemoryKind, MemoryObject, Provenance, Unit};
use crate::regions::tests::dos;
use crate::testing::{DOS, function, layout, parsed, value};

struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    fn new(body: &str) -> Self {
        let module = parsed(&format!("{DOS}{body}"));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit(&self) -> Unit<'_> {
        crate::testing::with_registers(Unit::of(&self.module, &self.layout, function(&self.module, "f")))
    }

    fn value(
        &self,
        name: &str,
    ) -> ValueId {
        value(function(&self.module, "f"), name)
    }

    fn made(
        &self,
        name: &str,
    ) -> InstId {
        let f = function(&self.module, "f");
        let wanted = self.value(name);
        f.walk().map(|(_, inst)| inst).find(|&inst| f.instruction(inst).result == Some(wanted)).expect("defined")
    }

    /// The instructions of `@f` whose opcode `is` picks.
    fn all(
        &self,
        is: impl Fn(&Opcode) -> bool,
    ) -> Vec<InstId> {
        let f = function(&self.module, "f");
        f.walk().map(|(_, inst)| inst).filter(|&inst| is(&f.instruction(inst).opcode)).collect()
    }

    /// What `known` says of `%name`, memory solved with `calls`.
    fn solved(
        &self,
        name: &str,
        calls: &Calls,
    ) -> Option<Known> {
        known_walked(&self.unit(), calls).get(&self.value(name)).cloned()
    }

    /// What `_result` makes of `%name` given facts about named values.
    fn result(
        &self,
        name: &str,
        facts: &[(&str, Known)],
    ) -> Option<Known> {
        let facts = facts.iter().map(|(name, fact)| (self.value(name), fact.clone())).collect::<IndexMap<_, _>>();
        _result(&self.unit(), self.made(name), &facts, None)
    }
}

fn one(
    operation: &str,
    source: &str,
) -> Parsed {
    Parsed::new(&format!(
        "define void @f({source} %x) {{
b0:
  %y = {operation}
  ret void
}}
"
    ))
}

#[test]
fn test_an_index_constant_is_not_the_value_of_an_indexed_store() {
    let parsed = Parsed::new(
        "@g = global [128 x i8] zeroinitializer

define i16 @f(i16 %source) {
b0:
  %index = add i16 2, 2
  %p = getelementptr i8, ptr getelementptr (i8, ptr @g, i16 90), i16 %index
  store i16 %source, ptr %p
  %loaded = load i16, ptr %p
  ret i16 %loaded
}
",
    );
    assert_eq!(parsed.solved("index", &Calls::default()), Some(Known::new(4, 16)));
    assert_eq!(parsed.solved("loaded", &Calls::default()), None);
}

#[test]
fn a_store_through_a_constant_index_is_read_back() {
    let parsed = Parsed::new(
        "@g = global [128 x i8] zeroinitializer

define i16 @f() {
b0:
  %index = add i16 2, 2
  %p = getelementptr i8, ptr @g, i16 %index
  store i16 9, ptr %p
  %loaded = load i16, ptr getelementptr (i8, ptr @g, i16 4)
  ret i16 %loaded
}
",
    );
    assert_eq!(parsed.solved("loaded", &Calls::default()), Some(Known::new(9, 16)));
    assert_eq!(known(&parsed.unit()).get(&parsed.value("loaded")), None, "memory is solved only when asked");
}

#[test]
fn test_signed_widening_produces_a_whole_long_constant() {
    for number in [0_i64, 1, 32767, 32768, 65535] {
        let parsed = one("sext i16 %x to i32", "i16");
        let expected = ((number ^ 0x8000) - 0x8000) & 0xFFFF_FFFF;
        assert_eq!(parsed.result("y", &[("x", Known::new(number, 16))]), Some(Known::new(expected, 32)));
        assert_eq!(parsed.result("y", &[("x", Known::new(number, 8))]), None);
    }
}

#[test]
fn test_word_extension_to_int64_preserves_signedness() {
    for (operation, number, expected) in [("sext", 0x8001_u64, 0xFFFF_FFFF_FFFF_8001_u64), ("zext", 0x8001, 0x8001)] {
        let parsed = one(&format!("{operation} i16 %x to i64"), "i16");
        assert_eq!(parsed.result("y", &[("x", Known::new(number, 16))]), Some(Known::new(expected, 64)));
    }
}

#[test]
fn test_extension_of_a_known_memory_cell_folds_to_the_extended_value() {
    let parsed = Parsed::new(
        "@g = global [4 x i8] zeroinitializer

define i32 @f() {
b0:
  store i8 -15, ptr @g
  %byte = load i8, ptr @g
  %wide = zext i8 %byte to i32
  ret i32 %wide
}
",
    );
    assert_eq!(parsed.solved("wide", &Calls::default()), Some(Known::new(0xF1, 32)));
}

#[test]
fn test_a_fact_is_masked_to_its_own_width() {
    for (n, width, want) in [(5_i64, 16, 5_i64), (-1, 16, 0xFFFF), (0x1FFFF, 16, 0xFFFF), (-1, 32, 0xFFFF_FFFF)] {
        assert_eq!(masked(&BigInt::from(n), width), BigInt::from(want));
    }
}

#[test]
fn test_equal_integer_operands_are_zero_without_input_facts() {
    for operation in ["xor", "sub"] {
        for width in [8, 16, 32] {
            let parsed = one(&format!("{operation} i{width} %x, %x"), &format!("i{width}"));
            assert_eq!(parsed.result("y", &[]), Some(Known::new(0, width)));
        }
    }
}

/// A compare of known numbers is a known bit, signed or not as it says:
/// peelsize decides an unrolled iteration's branch by it.
#[test]
fn a_compare_of_known_numbers_folds_to_its_bit() {
    for (predicate, want) in
        [("slt", 1), ("ult", 0), ("sgt", 0), ("ugt", 1), ("eq", 0), ("ne", 1), ("sle", 1), ("uge", 1)]
    {
        let parsed = one(&format!("icmp {predicate} i16 %x, 5"), "i16");
        assert_eq!(parsed.result("y", &[("x", Known::new(0xFFFF, 16))]), Some(Known::new(want, 1)), "{predicate}");
        assert_eq!(parsed.result("y", &[]), None, "{predicate}");
    }
}

#[test]
fn different_unknown_operands_are_not_folded() {
    let parsed = Parsed::new(
        "define void @f(i16 %x, i16 %z) {
b0:
  %y = sub i16 %x, %z
  ret void
}
",
    );
    assert_eq!(parsed.result("y", &[("x", Known::new(3, 16))]), None);
}

#[test]
fn test_a_narrow_shift_cannot_pull_bits_from_outside_its_operand() {
    for (width, number, answer) in [(8, 0x101_i64, 0_i64), (16, 0x1235_0000, 0), (16, 0x1235_8000, 0x4000)] {
        let parsed = one(&format!("lshr i{width} %x, 1"), &format!("i{width}"));
        assert_eq!(parsed.result("y", &[("x", Known::new(number, 32))]), Some(Known::new(answer, width)));
    }
}

#[test]
fn test_int64_shift_uses_all_six_count_bits() {
    let parsed = one("lshr i64 %x, 36", "i64");
    let number = 0xFEDC_BA98_7654_3210_u64;
    assert_eq!(parsed.result("y", &[("x", Known::new(number, 64))]), Some(Known::new(number >> 36, 64)));
}

#[test]
fn an_arithmetic_shift_keeps_the_sign() {
    assert_eq!(
        one("ashr i16 %x, 1", "i16").result("y", &[("x", Known::new(0x8000, 16))]),
        Some(Known::new(0xC000, 16))
    );
    assert_eq!(
        one("ashr i16 %x, 1", "i16").result("y", &[("x", Known::new(0x4000, 16))]),
        Some(Known::new(0x2000, 16))
    );
}

#[test]
fn test_constant_steps_wrap_at_the_value_width() {
    for width in [8, 16, 32] {
        for (operation, answer) in [("add i{w} -1, 1", 0_i64), ("sub i{w} 0, 1", -1)] {
            let parsed = one(&operation.replace("{w}", &width.to_string()), "i16");
            assert_eq!(parsed.result("y", &[]), Some(Known::new(masked(&BigInt::from(answer), width), width)));
        }
    }
}

/// A division is computed where it does not fault, as `division` gives it.
#[test]
fn test_division_is_folded_where_it_does_not_fault() {
    for (kind, answer) in [
        (BinaryOp::SDiv, Some(-3_i64)),
        (BinaryOp::UDiv, Some(0x7FFC)),
        (BinaryOp::SRem, Some(-1)),
        (BinaryOp::URem, Some(1)),
    ] {
        let spelling = llrm_mir::opcode::spelling(&llrm_mir::opcode::BINARY, kind);
        let parsed = one(&format!("{spelling} i16 -7, 2"), "i16");
        assert_eq!(parsed.result("y", &[]), answer.map(|n| Known::new(masked(&BigInt::from(n), 16), 16)), "{spelling}");
        assert_eq!(one(&format!("{spelling} i16 -7, 0"), "i16").result("y", &[]), None, "{spelling}");
    }
}

#[test]
fn division_gives_quotient_and_remainder_but_not_where_it_faults() {
    for (operation, expected) in [
        ("sdiv i16 -7, 2", Some((-3_i64, -1_i64))),
        ("srem i16 -7, 2", Some((-3, -1))),
        ("udiv i16 -7, 2", Some((0x7FFC, 1))),
        ("sdiv i16 7, 0", None),
        ("sdiv i16 -32768, -1", None),
        ("add i16 7, 2", None),
    ] {
        let parsed = one(operation, "i16");
        let got = division(&parsed.unit(), parsed.made("y"), &IndexMap::default());
        let expected = expected.map(|(q, r)| (masked(&BigInt::from(q), 16), masked(&BigInt::from(r), 16)));
        assert_eq!(got, expected, "{operation}");
    }
}

/// Division folded only at 16, 32 and 64 bits, the old x86 widths: an
/// i8 quotient had none.
#[test]
fn division_folds_at_every_width() {
    for (operation, width, expected) in
        [("sdiv i8 -7, 2", 8, (-3_i64, -1_i64)), ("udiv i8 -7, 2", 8, (0x7C, 1)), ("urem i1 1, 1", 1, (1, 0))]
    {
        let parsed = one(operation, &format!("i{width}"));
        let got = division(&parsed.unit(), parsed.made("y"), &IndexMap::default());
        assert_eq!(
            got,
            Some((masked(&BigInt::from(expected.0), width), masked(&BigInt::from(expected.1), width))),
            "{operation}"
        );
    }
}

#[test]
fn test_a_fact_is_never_wider_than_the_operation_that_made_it() {
    assert_eq!(masked(&BigInt::from(0x1FFFF), 16), BigInt::from(0xFFFF));
    assert_eq!(one("add i8 -1, 3", "i16").result("y", &[]), Some(Known::new(2, 8)));
}

/// A call's reach, as `alias::calls_annotated` states it.
fn reaching(
    parsed: &Parsed,
    provenance: Provenance,
) -> Calls {
    let call = parsed.all(|op| matches!(op, Opcode::Call(_)))[0];
    Calls::from_iter([(call, std::rc::Rc::from(vec![MemRef::reach(0, provenance)]))])
}

const AROUND_A_CALL: &str = "declare void @g()

define i16 @f() {
b0:
  %a = alloca i16
  store i16 7, ptr %a
  call void @g()
  %r = load i16, ptr %a
  ret i16 %r
}
";

/// A constant cell's key had no object, so it met every call's reach and
/// died at each one. Here with a frame object, which nonlocal reach misses.
#[test]
fn test_a_call_reaching_nonlocal_keeps_an_uncaptured_static_constant() {
    let parsed = Parsed::new(AROUND_A_CALL);
    let nonlocal = reaching(&parsed, Provenance::one(MemoryObject::new(MemoryKind::Nonlocal)));
    assert_eq!(parsed.solved("r", &nonlocal), Some(Known::new(7, 16)));
}

#[test]
fn a_call_that_may_write_anything_forgets_every_cell() {
    // The cell's address is exposed: an unexposed alloca no call reaches.
    let parsed = Parsed::new(
        &AROUND_A_CALL.replace("@g()", "@g(ptr %a)").replace("void @g(ptr %a)\n\ndefine", "void @g(ptr)\n\ndefine"),
    );
    assert_eq!(parsed.solved("r", &Calls::default()), None);
    let unknown = reaching(&parsed, Provenance::one(MemoryObject::new(MemoryKind::Unknown)));
    assert_eq!(parsed.solved("r", &unknown), None);
}

#[test]
fn a_call_to_a_callee_that_only_reads_keeps_every_cell() {
    let parsed = Parsed::new(&AROUND_A_CALL.replace("declare void @g()", "declare void @g() memory(read)"));
    assert_eq!(parsed.solved("r", &Calls::default()), Some(Known::new(7, 16)));
}

/// A store's reference had no provenance, so a store to one global met a
/// cell of every other.
#[test]
fn a_store_to_another_global_keeps_the_cell() {
    let parsed = Parsed::new(
        "@a = global i16 0
@b = global i16 0

define i16 @f() {
b0:
  store i16 7, ptr @a
  store i16 1, ptr @b
  %r = load i16, ptr @a
  ret i16 %r
}
",
    );
    assert_eq!(parsed.solved("r", &Calls::default()), Some(Known::new(7, 16)));
}

#[test]
fn a_store_through_an_unknown_pointer_forgets_the_cell_it_may_hit() {
    let parsed = Parsed::new(
        "@g = global i16 0

define i16 @f(ptr %p) {
b0:
  store i16 7, ptr @g
  store i16 1, ptr %p
  %r = load i16, ptr @g
  ret i16 %r
}
",
    );
    assert_eq!(parsed.solved("r", &Calls::default()), None);
}

#[test]
fn a_store_to_a_neighbouring_field_keeps_the_cell() {
    let parsed = Parsed::new(
        "@g = global [4 x i8] zeroinitializer

define i16 @f() {
b0:
  store i16 7, ptr @g
  store i16 1, ptr getelementptr (i8, ptr @g, i16 2)
  %r = load i16, ptr @g
  ret i16 %r
}
",
    );
    assert_eq!(parsed.solved("r", &Calls::default()), Some(Known::new(7, 16)));
}

#[test]
fn a_partial_overwrite_changes_only_its_bytes() {
    let parsed = Parsed::new(
        "@g = global [4 x i8] zeroinitializer

define i16 @f() {
b0:
  store i16 4660, ptr @g
  store i8 -1, ptr getelementptr (i8, ptr @g, i16 1)
  %r = load i16, ptr @g
  ret i16 %r
}
",
    );
    assert_eq!(parsed.solved("r", &Calls::default()), Some(Known::new(0xFF34, 16)));
}

#[test]
fn a_join_knows_a_cell_only_where_every_path_agrees() {
    for (left, right, expected) in [(7, 7, Some(Known::new(7, 16))), (7, 8, None)] {
        let parsed = Parsed::new(&format!(
            "@g = global i16 0

define i16 @f(i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 {left}, ptr @g
  br label %b3

b2:
  store i16 {right}, ptr @g
  br label %b3

b3:
  %r = load i16, ptr @g
  ret i16 %r
}}
"
        ));
        assert_eq!(parsed.solved("r", &Calls::default()), expected, "{left} {right}");
    }
}

#[test]
fn a_cell_stored_in_a_loop_is_not_its_value_before_it() {
    let parsed = Parsed::new(
        "@g = global i16 0

define i16 @f(i1 %c) {
b0:
  store i16 0, ptr @g
  br label %b1

b1:
  %r = load i16, ptr @g
  %n = add i16 %r, 1
  store i16 %n, ptr @g
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %r
}
",
    );
    assert_eq!(parsed.solved("r", &Calls::default()), None);
}

/// A far store whose selector is unknown may hit a program object; one
/// whose selector lands in foreign memory cannot.
#[test]
fn a_far_store_forgets_a_cell_unless_its_segment_is_foreign() {
    for (selector, machine, kept) in [("%sel", true, false), ("-18432", false, false), ("-18432", true, true)] {
        let parsed = Parsed::new(&format!(
            "@g = global i16 0

define i16 @f(i16 %sel) {{
b0:
  store i16 7, ptr @g
  %s = inttoptr i16 {selector} to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  store i16 1, ptr addrspace(1) %far
  %r = load i16, ptr @g
  ret i16 %r
}}
"
        ));
        let dos = dos(&parsed.module);
        let unit = Unit { program: machine.then_some(&*dos), ..parsed.unit() };
        let got = known_walked(&unit, &Calls::default()).get(&parsed.value("r")).cloned();
        assert_eq!(got.is_some(), kept, "{selector} {machine}");
    }
}

#[test]
fn a_volatile_load_is_not_forwarded_a_stored_value() {
    let parsed = Parsed::new(
        "@g = global i16 0

define i16 @f() {
b0:
  store i16 7, ptr @g
  %r = load volatile i16, ptr @g
  ret i16 %r
}
",
    );
    assert_eq!(parsed.solved("r", &Calls::default()), None);
}

#[test]
fn a_direct_constant_store_initializes_the_cell_it_contains() {
    let parsed = Parsed::new(
        "@g = global [4 x i8] zeroinitializer

define void @f(i16 %x) {
b0:
  store i32 305419896, ptr @g
  store i16 %x, ptr @g
  ret void
}
",
    );
    let unit = parsed.unit();
    let stores = parsed.all(|op| matches!(op, Opcode::Store { .. }));
    let high = MemRef::at(
        &unit,
        Operand::Constant(match unit.function.instruction(stores[0]).operands[1] {
            Operand::Constant(one) => one,
            _ => unreachable!(),
        }),
        2,
    );
    let high = MemRef { disp: 2, ..high };
    assert_eq!(initialized(&unit, stores[0], &high), Some(Known::new(0x1234, 16)));
    assert_eq!(initialized(&unit, stores[1], &high), None);
}

/// `floatfacts.repeated` stores through a fresh clone each iteration; keyed
/// by address, a later reference reused an earlier one's answer.
#[test]
fn test_a_reference_at_a_reused_address_is_resolved_anew() {
    let parsed = Parsed::new(
        "@g = global [64 x i8] zeroinitializer

define void @f() {
b0:
  store i32 0, ptr getelementptr (i8, ptr @g, i16 18)
  store i32 0, ptr getelementptr (i8, ptr @g, i16 26)
  ret void
}
",
    );
    let unit = parsed.unit();
    let [first, second] = parsed.all(|op| matches!(op, Opcode::Store { .. }))[..] else { panic!() };
    let (first, second) = (MemRef::of(&unit, first).unwrap(), MemRef::of(&unit, second).unwrap());
    let mut queries = _MemoryQueries::new(unit, &IndexMap::default());
    let mut slot = first.clone();
    assert_eq!(**queries.resolve(&slot), first);
    slot = second.clone();
    assert_eq!(**queries.resolve(&slot), second);
}

/// A float store's bits are a number in memory: an integer load of them
/// read nothing.
#[test]
fn test_a_float_store_writes_its_bits() {
    let parsed = Parsed::new(
        "@g = global [12 x i8] zeroinitializer

define void @f() {
b0:
  store float 2.0, ptr @g
  store double -0.0, ptr getelementptr (i8, ptr @g, i16 4)
  %single = load i32, ptr @g
  %high = load i16, ptr getelementptr (i8, ptr @g, i16 10)
  ret void
}
",
    );
    assert_eq!(parsed.solved("single", &Calls::default()), Some(Known::new(0x4000_0000, 32)));
    assert_eq!(parsed.solved("high", &Calls::default()), Some(Known::new(0x8000, 16)));
}

/// A load through a pointer is known where the one store MemorySSA finds
/// it clobbered by wrote its bytes and its type with a known value.
#[test]
fn test_a_dominating_store_supplies_a_load_through_a_pointer() {
    for (between, known) in [
        ("", Some(Known::new(7, 16))),
        ("store i16 9, ptr %q", None),
        // The byte written and the high byte of the word stored before it.
        ("store i8 9, ptr %p", Some(Known::new(9, 16))),
        ("call void @g()", None),
        ("%s = getelementptr inbounds i8, ptr %p, i16 2\n  store i16 9, ptr %s", Some(Known::new(7, 16))),
    ] {
        let parsed = Parsed::new(&format!(
            "declare void @g()

define i16 @f(ptr %p, ptr %q) {{
b0:
  %v = add i16 3, 4
  store i16 %v, ptr %p
  {between}
  %r = load i16, ptr %p
  ret i16 %r
}}
"
        ));
        assert_eq!(parsed.solved("r", &Calls::default()), known, "{between}");
    }
}

/// A store on one path into a join supplies nothing; a load of another
/// type through the stored pointer is not the stored value.
#[test]
fn test_a_pointer_load_is_not_supplied_across_a_join_or_a_type() {
    let joined = Parsed::new(
        "define i16 @f(ptr %p, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 7, ptr %p
  br label %b2

b2:
  %r = load i16, ptr %p
  ret i16 %r
}
",
    );
    assert_eq!(joined.solved("r", &Calls::default()), None);
    let retyped = Parsed::new(
        "define i16 @f(ptr %p, ptr %q) {
b0:
  store ptr %q, ptr %p
  %r = load i16, ptr %p
  ret i16 %r
}
",
    );
    assert_eq!(retyped.solved("r", &Calls::default()), None);
}

#[test]
fn memory_facts_do_not_hang_on_which_function_was_solved_first() {
    // Overlap buckets were once numbered in one process-wide table, shared by
    // every function solved on the thread.
    let module = |first: &str, second: &str| {
        Parsed::new(&format!(
            "@{first} = global [4 x i8] zeroinitializer
@{second} = global [4 x i8] zeroinitializer

define i16 @f() {{
b0:
  store i16 7, ptr @{first}
  store i16 8, ptr @{second}
  store i8 1, ptr getelementptr (i8, ptr @{second}, i16 2)
  %a = load i16, ptr @{first}
  ret i16 %a
}}
"
        ))
    };
    let (one, two) = (module("g", "h"), module("h", "g"));
    let solve = |parsed: &Parsed| known_walked(&parsed.unit(), &Calls::default());
    let (first, second) = (solve(&one), solve(&two));
    assert_eq!(first.get(&one.value("a")), Some(&Known::new(7, 16)));
    assert_eq!((solve(&two), solve(&one)), (second, first));
}

/// Fixed point of known numbers is a number: unrolled, T075's `fix(n) / 4`
/// and `c * fix(k)` were calls of known operands no one computed.
#[test]
fn a_fixed_point_intrinsic_of_known_numbers_folds() {
    let parsed = Parsed::new(
        "declare i32 @llvm.smul.fix.i32(i32, i32, i32)
declare i32 @llvm.sdiv.fix.i32(i32, i32, i32)

define i32 @f(i32 %x) {
b0:
  %p = call i32 @llvm.smul.fix.i32(i32 %x, i32 -640, i32 8)
  %q = call i32 @llvm.sdiv.fix.i32(i32 %x, i32 1024, i32 8)
  %z = call i32 @llvm.sdiv.fix.i32(i32 %x, i32 0, i32 8)
  ret i32 %p
}
",
    );
    let x = [("x", Known::new(768, 32))];
    assert_eq!(parsed.result("p", &x), Some(Known::new(masked(&BigInt::from(-1920), 32), 32)), "3.0 * -2.5");
    assert_eq!(parsed.result("q", &x), Some(Known::new(192, 32)));
    assert_eq!(parsed.result("z", &x), None, "a zero divisor is undefined");
}

/// `known` built each access's reference without the exposed-frames table, so
/// each scanned its alloca's uses: 3.6% of compiling 100 sequential loops
/// (#556). The table is made once for the body.
#[test]
fn test_known_asks_each_bodys_exposed_frames_once() {
    let accesses: String =
        (0..40).map(|at| format!("  store i16 {at}, ptr %slot\n  %v{at} = load i16, ptr %slot\n")).collect();
    let parsed =
        Parsed::new(&format!("define i16 @f() {{\nentry:\n  %slot = alloca i16\n{accesses}  ret i16 %v39\n}}\n"));
    let before = crate::frameescape::scans();
    known_walked(&parsed.unit(), &Calls::default());
    assert_eq!(crate::frameescape::scans() - before, 0, "an access scanned its alloca's uses");
}

/// `overlaps` was keyed by a resolved reference's address, so which bucket an
/// answer lived in, and the work of finding it, changed with ASLR: `mir hoist`
/// varied up to 0.9% between identical compiles (#992). The key is the order
/// the references were resolved.
#[test]
fn test_overlap_answers_are_keyed_by_the_order_resolved_and_not_by_address() {
    let parsed = Parsed::new(
        "@g = global [64 x i8] zeroinitializer

define void @f() {
b0:
  store i32 0, ptr getelementptr (i8, ptr @g, i16 18)
  store i32 0, ptr getelementptr (i8, ptr @g, i16 26)
  ret void
}
",
    );
    let unit = parsed.unit();
    let stores = parsed.all(|op| matches!(op, Opcode::Store { .. }));
    let mut queries = _MemoryQueries::new(unit, &IndexMap::default());
    let resolved: Vec<_> = stores.iter().map(|&inst| queries.resolve(&MemRef::of(&unit, inst).unwrap())).collect();
    let root = resolved[0].root.expect("a global");
    for reference in &resolved {
        queries.may_overlap((Addr { root, disp: 0 }, 4), reference);
    }
    let mut keys: Vec<u64> = queries.overlaps.keys().map(|&(_, id)| id as u64).collect();
    keys.sort_unstable();
    assert_eq!(keys, [0, 1], "answers keyed by an address");
}

/// What the walk (`known_walked`, through-memory's answer) says of `%name`.
fn walked(
    parsed: &Parsed,
    name: &str,
) -> Option<Known> {
    known_walked(&parsed.unit(), &Calls::default()).get(&parsed.value(name)).cloned()
}

/// Two word stores and a dword load of both: dice.nib's `load i32` of the two
/// words it wrote, which the walk's whole-cell form could not serve (no one
/// store wrote the load's bytes).
#[test]
fn a_dword_loaded_from_two_word_stores_is_the_dword_they_make() {
    let parsed = Parsed::new(
        "@g = global i32 0

define i32 @f() {
b0:
  store i16 4660, ptr @g
  %hi = getelementptr i8, ptr @g, i16 2
  store i16 22136, ptr %hi
  %r = load i32, ptr @g
  ret i32 %r
}
",
    );
    assert_eq!(walked(&parsed, "r"), Some(Known::new(0x5678_1234, 32)));
}

/// A word loaded from a dword store reads two of its bytes.
#[test]
fn a_word_loaded_from_a_dword_store_is_its_bytes() {
    let parsed = Parsed::new(
        "@g = global i32 0

define i16 @f() {
b0:
  store i32 305419896, ptr @g
  %hi = getelementptr i8, ptr @g, i16 2
  %r = load i16, ptr %hi
  ret i16 %r
}
",
    );
    assert_eq!(walked(&parsed, "r"), Some(Known::new(0x1234, 16)));
}

/// QCport's mdl_bottom_selftest stored `2593.0f` and copied the struct as
/// dwords: the load of another type than the store reads the float's bits.
#[test]
fn a_float_constant_stored_is_read_back_as_its_bits_by_an_integer_load() {
    let parsed = Parsed::new(
        "@g = global float 0.0

define i32 @f() {
b0:
  store float 2593.0, ptr @g
  %r = load i32, ptr @g
  ret i32 %r
}
",
    );
    assert_eq!(walked(&parsed, "r"), Some(Known::new(0x4522_1000, 32)));
}

/// A join is known where every store that reaches it puts the same number, and
/// not where one puts another.
#[test]
fn a_load_after_a_join_is_known_where_both_arms_store_the_same_number() {
    let text = |other: u32| {
        format!(
            "@g = global i16 0

define i16 @f(i1 %c) {{
b0:
  br i1 %c, label %yes, label %no
yes:
  store i16 9, ptr @g
  br label %join
no:
  store i16 {other}, ptr @g
  br label %join
join:
  %r = load i16, ptr @g
  ret i16 %r
}}
"
        )
    };
    assert_eq!(walked(&Parsed::new(&text(9)), "r"), Some(Known::new(9, 16)));
    assert_eq!(walked(&Parsed::new(&text(10)), "r"), None);
}

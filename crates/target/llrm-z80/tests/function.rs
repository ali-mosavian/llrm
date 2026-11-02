//! IR to scheduled machine nodes through the shared builder and the Z80's
//! tables.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_z80::dagisel::function::{Selected, select_function};
use llrm_z80::dagisel::generated::z80_z80::MACHINE_OPCODES;
use llrm_z80::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16";

fn selected(
    body: &str,
    name: &str,
) -> Vec<Vec<&'static str>> {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let blocks = select_function(&module, name, Cpu::Z80).expect("selects").blocks;
    blocks.iter().map(names).collect()
}

/// The machine nodes in order. A unit lists its glued nodes last first.
fn names(one: &Selected) -> Vec<&'static str> {
    one.units
        .iter()
        .flat_map(|unit| unit.nodes.iter().rev().copied())
        .filter_map(|node| match one.dag.nodes()[node as usize].opcode {
            ISD::Machine(code) => MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, name)| *name),
            _ => None,
        })
        .collect()
}

#[test]
fn a_byte_add_is_one_add_on_a() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\n  %r = add i8 %a, %b\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADD8r", "RET"]]);
}

#[test]
fn a_word_add_is_add_hl() {
    let ir = "define i16 @f(i16 %a, i16 %b) {\n  %r = add i16 %a, %b\n  ret i16 %r\n}";
    let blocks = selected(ir, "f");
    assert!(blocks[0].contains(&"ADD16"), "{blocks:?}");
}

/// With every argument in a register the call is `CALL` between the stack
/// adjustments. Before the call node and the pseudos were in the `.td`,
/// `CallSeqEnd` could not be selected.
#[test]
fn a_call_is_a_call_between_the_call_sequence_pseudos() {
    let ir = "declare i8 @g(i8, i16)\ndefine i8 @f(i8 %a, i16 %b) {\n  %r = call i8 @g(i8 %a, i16 %b)\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["ADJCALLSTACKDOWN", "CALL", "ADJCALLSTACKUP", "RET"]]);
}

/// `a < b` unsigned: `CP8r`, then a jump to the false block on no carry and a
/// jump to the true one.
#[test]
fn an_unsigned_compare_branches_on_carry() {
    let ir = "define i8 @f(i8 %a, i8 %b) {\nentry:\n  %c = icmp ult i8 %a, %b\n  br i1 %c, label %t, label %f\nt:\n  ret i8 1\nf:\n  ret i8 2\n}";
    let blocks = selected(ir, "f");
    assert_eq!(blocks[0], ["CP8r", "JPNC", "JP"]);
}

/// Word logic is one pseudo, expanded to the two bytes' ops when emitted.
/// Before it, `and i16` was a custom lowering whose own `or` join hung the
/// legalizer.
#[test]
fn a_word_and_is_one_word_pseudo() {
    let ir = "define i16 @f(i16 %a, i16 %b) {\n  %r = and i16 %a, %b\n  ret i16 %r\n}";
    let blocks = selected(ir, "f");
    assert!(blocks[0].contains(&"AND16"), "{blocks:?}");
}

/// Two bytes joined into a word are the two halves of one pair, not an `or` of
/// extensions.
#[test]
fn two_bytes_joined_are_a_pair_not_an_or() {
    let ir = "define i16 @f(i8 %a, i8 %b, i16 %w) {\n  %x = zext i8 %a to i16\n  %y = zext i8 %b to i16\n  %h = shl i16 %y, 8\n  %r = or i16 %x, %h\n  %s = add i16 %r, %w\n  ret i16 %s\n}";
    let blocks = selected(ir, "f");
    assert!(blocks[0].contains(&"REG_SEQUENCE") && !blocks[0].contains(&"OR16"), "{blocks:?}");
}

/// `max` is a compare and a conditional move, byte or word. Before the
/// `SETCC`/`CMOV` nodes, `max8` and `max16` refused with
/// `CannotSelect(Select)`.
#[test]
fn a_select_on_a_compare_is_a_conditional_move() {
    let byte =
        "define i8 @f(i8 %a, i8 %b) {\n  %c = icmp ugt i8 %a, %b\n  %r = select i1 %c, i8 %a, i8 %b\n  ret i8 %r\n}";
    assert_eq!(selected(byte, "f"), [["CP8r", "CMOV8", "RET"]]);
    let word = "define i16 @g(i16 %a, i16 %b) {\n  %c = icmp ult i16 %a, %b\n  %r = select i1 %c, i16 %a, i16 %b\n  ret i16 %r\n}";
    assert!(selected(word, "g")[0].contains(&"CMOV16"));
}

/// A signed compare is the unsigned one on sign-flipped operands: an `xor 0x80`
/// on each side.
#[test]
fn a_signed_byte_compare_flips_both_sign_bits() {
    let ir =
        "define i8 @f(i8 %a, i8 %b) {\n  %c = icmp slt i8 %a, %b\n  %r = select i1 %c, i8 %a, i8 %b\n  ret i8 %r\n}";
    let names = &selected(ir, "f")[0];
    assert_eq!(names.iter().filter(|name| **name == "XOR8i").count(), 2, "{names:?}");
}

/// Multiply, divide, remainder and a shift by a variable are runtime calls
/// (`RTLIB`), one per operation. Before them `mul8`, `mul16` and `digits`
/// refused with `CannotSelect(Mul)` and `CannotSelect(MulHU)`.
#[test]
fn multiply_divide_and_variable_shifts_are_calls() {
    for (ir, name) in [
        ("define i8 @f(i8 %a, i8 %b) {\n  %r = mul i8 %a, %b\n  ret i8 %r\n}", "f"),
        ("define i16 @f(i16 %a, i16 %b) {\n  %r = mul i16 %a, %b\n  ret i16 %r\n}", "f"),
        ("define i16 @f(i16 %a, i16 %b) {\n  %r = udiv i16 %a, %b\n  ret i16 %r\n}", "f"),
        ("define i16 @f(i16 %a, i16 %b) {\n  %r = srem i16 %a, %b\n  ret i16 %r\n}", "f"),
        ("define i16 @f(i16 %a, i8 %n) {\n  %c = zext i8 %n to i16\n  %r = shl i16 %a, %c\n  ret i16 %r\n}", "f"),
    ] {
        let blocks = selected(ir, name);
        assert_eq!(blocks[0].iter().filter(|n| **n == "CALL").count(), 1, "{ir}: {blocks:?}");
    }
}

/// A shift by a constant stays an instruction.
#[test]
fn a_shift_by_a_constant_is_an_instruction() {
    let ir = "define i8 @f(i8 %a) {\n  %r = shl i8 %a, 3\n  ret i8 %r\n}";
    assert_eq!(selected(ir, "f"), [["SHL8", "RET"]]);
}

/// A byte read and widened, and a word read, through a pointer: the byte load
/// with a zero high byte, and the word load. Before them `scmp`, `crc16` and
/// `sum16` of the bench refused with `CannotSelect(Load)`.
#[test]
fn loads_through_a_pointer_widen_a_byte_and_read_a_word() {
    let widened = "define i16 @f(ptr %p) {\n  %b = load i8, ptr %p\n  %w = zext i8 %b to i16\n  ret i16 %w\n}";
    let blocks = selected(widened, "f");
    // one instruction widens the byte: it cannot swap the halves the way a join
    // of two bytes can
    assert_eq!(blocks, [["LD8rm", "ZEXT8", "RET"]]);
    let word = "define i16 @g(ptr %p) {\n  %w = load i16, ptr %p\n  ret i16 %w\n}";
    assert!(selected(word, "g")[0].contains(&"LD16rm"));
}

fn function(
    body: &str,
    name: &str,
) -> llrm_z80::dagisel::function::Selection {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    select_function(&module, name, Cpu::Z80).expect("selects")
}

/// A fourth word argument panicked with "Stack": three pairs hold three words
/// and the fourth has no register. Worked out from `calling.toml`: `a` HL, `b`
/// DE, `c` BC, `d` the first stack argument, offset 0 from the first stack byte
/// (the return address and saved IX in front of it are the frame resolver's, 4
/// bytes). The callee loads it from that fixed object in place, `d(ix)`.
#[test]
fn a_fourth_word_argument_is_a_fixed_object_at_offset_zero() {
    let ir = "define i16 @f(i16 %a, i16 %b, i16 %c, i16 %d) {\n  ret i16 %d\n}";
    let f = function(ir, "f");
    assert_eq!(f.blocks.len(), 1);
    assert!(names(&f.blocks[0]).starts_with(&["LD16rf"]), "{:?}", names(&f.blocks[0]));
    assert_eq!(f.blocks[0].dag.fixed_offsets.values().copied().collect::<Vec<_>>(), [0]);
}

/// `f(i32 a, i32 b)` as a caller: `a` is HL and DE, the low word of `b` BC, its
/// high word the one stack word. The call sequence reserves 2 bytes and one
/// `STSP16` stores the high word at offset 0 in it.
#[test]
fn a_call_with_two_longs_stores_one_word_on_the_stack() {
    let ir = "declare i32 @g(i32, i32)\ndefine i32 @f(i32 %a, i32 %b) {\n  %r = call i32 @g(i32 %a, i32 %b)\n  ret i32 %r\n}";
    let f = function(ir, "f");
    let all: Vec<_> = f.blocks.iter().flat_map(names).collect();
    assert_eq!(all.iter().filter(|name| **name == "STSP16").count(), 1, "{all:?}");
    let block = &f.blocks[0];
    let node = |wanted: &str| {
        block
            .dag
            .nodes()
            .iter()
            .position(|n| matches!(
                n.opcode,
                ISD::Machine(code) if MACHINE_OPCODES.iter().any(|(v, name)| u32::from(*v) == code && *name == wanted)
            ))
            .unwrap_or_else(|| panic!("no {wanted} in {all:?}"))
    };
    let constant = |at: usize, operand: usize| {
        let operands = &block.dag.nodes()[at].ops;
        block
            .dag
            .constant(operands[operand])
            .unwrap_or_else(|| panic!("operand {operand} of {wanted_name} is no constant", wanted_name = at))
    };
    // STSP16 operands: value, offset, chain
    let store = node("STSP16");
    assert_eq!(constant(store, 1), 0);
    assert_eq!(constant(node("ADJCALLSTACKDOWN"), 0), 2);
}

/// A signed byte widened to a word is the byte and its sign (`rla; sbc a,a`).
/// Before the pattern `sext i8 to i16` refused with `CannotSelect(SignExtend)`,
/// which any signed index or char arithmetic hits.
#[test]
fn a_signed_byte_widens_with_its_sign() {
    let ir = "define i16 @f(i8 %a) {\n  %r = sext i8 %a to i16\n  ret i16 %r\n}";
    let blocks = selected(ir, "f");
    assert!(blocks[0].contains(&"SEXT8"), "{blocks:?}");
}

/// The bench kernel `sum_arr` (clang msp430 IR, a byte loop over a global,
/// `tests/function/sum_arr.ll`) went through the mid-end and panicked in LSR
/// ("Formula is illegal") because the cost model accepted no addressing mode at
/// all; 10 of the 23 bench kernels did. The symptom is the panic, so the test
/// is the compile.
#[test]
fn a_loop_over_a_global_compiles_through_the_mid_end() {
    use llrm_z80::driver::{Level, Request, selections};
    let ir = include_str!("function/sum_arr.ll");
    let worker = std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(move || {
            selections(ir, &Request::new(Level::O2))
                .map(|all| all.into_iter().map(|(name, result)| (name, result.err())).collect::<Vec<_>>())
        })
        .unwrap();
    assert_eq!(worker.join().expect("no panic in the mid-end").unwrap(), [("sum_arr".to_owned(), None)]);
}

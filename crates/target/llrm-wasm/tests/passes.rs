//! The post-selection passes on hand-built functions. Inputs and expected
//! outputs are `llc -mtriple=wasm32 -O0 -fast-isel=false` (system LLVM 20.1.8)
//! `-print-before/after=wasm-explicit-locals`, unless a test says otherwise.

use llrm_wasm::argument_move::argument_move;
use llrm_wasm::dagisel::generated::ids::opcode as op;
use llrm_wasm::dagisel::post_ids as post;
use llrm_wasm::explicit_locals::explicit_locals;
use llrm_wasm::mir::{Function, Inst, Operand::*, ValType};
use llrm_wasm::reg_numbering::reg_numbering;

fn name(opcode: u16) -> &'static str {
    [
        (op::ARGUMENT_i32, "ARGUMENT_i32"),
        (op::LOCAL_GET_I32, "LOCAL_GET_I32"),
        (op::LOCAL_SET_I32, "LOCAL_SET_I32"),
        (op::ADD_I32, "ADD_I32"),
        (op::CONST_I32, "CONST_I32"),
        (op::LOAD_I32_A32, "LOAD_I32_A32"),
        (op::STORE_I32_A32, "STORE_I32_A32"),
        (op::RETURN, "RETURN"),
        (post::DROP_I32, "DROP_I32"),
        (post::LOCAL_TEE_I32, "LOCAL_TEE_I32"),
        (post::TEE_I32, "TEE_I32"),
        (post::COPY_I32, "COPY_I32"),
    ]
    .iter()
    .find(|(id, _)| *id == opcode)
    .map_or("?", |(_, n)| n)
}

/// `%d = NAME ops` per instruction.
fn dump(f: &Function) -> Vec<String> {
    f.blocks[0]
        .iter()
        .map(|i| {
            let defs: Vec<_> =
                i.operands.iter().filter_map(|o| if let Def(r) = o { Some(format!("%{r}")) } else { None }).collect();
            let uses: Vec<_> = i
                .operands
                .iter()
                .filter_map(|o| match o {
                    Def(_) => None,
                    Use(r) => Some(format!("%{r}")),
                    Imm(v) => Some(v.to_string()),
                })
                .collect();
            let head = if defs.is_empty() { String::new() } else { format!("{} = ", defs.join(", ")) };
            format!("{head}{} {}", name(i.opcode), uses.join(", ")).trim_end().to_string()
        })
        .collect()
}

fn function(
    params: usize,
    vregs: usize,
    insts: Vec<Inst>,
) -> Function {
    let mut f = Function::new("f");
    f.params = vec![ValType::I32; params];
    for _ in 0..vregs {
        f.new_vreg(ValType::I32);
    }
    f.blocks[0] = insts;
    f
}

fn arg(
    reg: u32,
    index: i64,
) -> Inst {
    Inst::new(op::ARGUMENT_i32, &[Def(reg), Imm(index)])
}

/// A load: `LOCAL_GET` of the argument in front, the result through a local.
#[test]
fn a_load_reads_its_argument_and_returns_through_a_local() {
    let mut f = function(
        1,
        2,
        vec![
            arg(0, 0),
            Inst::new(op::LOAD_I32_A32, &[Def(1), Imm(2), Imm(0), Use(0)]),
            Inst::new(op::RETURN, &[Use(1)]),
        ],
    );
    explicit_locals(&mut f);

    assert_eq!(
        dump(&f),
        [
            "%3 = LOCAL_GET_I32 0",
            "%2 = LOAD_I32_A32 2, 0, %3",
            "LOCAL_SET_I32 1, %2",
            "%4 = LOCAL_GET_I32 1",
            "RETURN %4",
        ]
        .map(String::from)
    );
    assert_eq!(f.locals, [ValType::I32]);
}

/// The `local.get` of the last operand goes in first, so the operands read in
/// source order; before the fix to the insertion point they came out swapped.
#[test]
fn a_store_reads_its_operands_in_order() {
    let mut f = function(
        2,
        2,
        vec![
            arg(0, 1),
            arg(1, 0),
            Inst::new(op::STORE_I32_A32, &[Imm(2), Imm(0), Use(1), Use(0)]),
            Inst::new(op::RETURN, &[]),
        ],
    );

    explicit_locals(&mut f);

    assert_eq!(
        dump(&f),
        ["%3 = LOCAL_GET_I32 0", "%2 = LOCAL_GET_I32 1", "STORE_I32_A32 2, 0, %3, %2", "RETURN"].map(String::from)
    );
}

/// Locals are numbered after the parameters in order of first use or def.
#[test]
fn two_adds_take_locals_after_the_parameters() {
    let mut f = function(
        3,
        5,
        vec![
            arg(0, 2),
            arg(1, 1),
            arg(2, 0),
            Inst::new(op::ADD_I32, &[Def(3), Use(2), Use(1)]),
            Inst::new(op::ADD_I32, &[Def(4), Use(3), Use(0)]),
            Inst::new(op::RETURN, &[Use(4)]),
        ],
    );

    explicit_locals(&mut f);

    assert_eq!(
        dump(&f),
        [
            "%7 = LOCAL_GET_I32 0",
            "%6 = LOCAL_GET_I32 1",
            "%5 = ADD_I32 %7, %6",
            "LOCAL_SET_I32 3, %5",
            "%10 = LOCAL_GET_I32 3",
            "%9 = LOCAL_GET_I32 2",
            "%8 = ADD_I32 %10, %9",
            "LOCAL_SET_I32 4, %8",
            "%11 = LOCAL_GET_I32 4",
            "RETURN %11",
        ]
        .map(String::from)
    );
    assert_eq!(f.locals.len(), 2);
}

/// A constant used once, not argument-fed: its set and get both appear.
#[test]
fn a_constant_goes_through_a_local() {
    let mut f = function(
        0,
        2,
        vec![
            Inst::new(op::CONST_I32, &[Def(0), Imm(0)]),
            Inst::new(op::LOAD_I32_A32, &[Def(1), Imm(2), Imm(64), Use(0)]),
            Inst::new(op::RETURN, &[Use(1)]),
        ],
    );

    explicit_locals(&mut f);

    assert_eq!(
        dump(&f),
        [
            "%2 = CONST_I32 0",
            "LOCAL_SET_I32 0, %2",
            "%4 = LOCAL_GET_I32 0",
            "%3 = LOAD_I32_A32 2, 64, %4",
            "LOCAL_SET_I32 1, %3",
            "%5 = LOCAL_GET_I32 1",
            "RETURN %5",
        ]
        .map(String::from)
    );
}

/// From `explicitLocals` in the pin's source: a result nobody reads is
/// dropped, not stored.
#[test]
fn a_result_nobody_reads_is_dropped() {
    let mut f = function(0, 1, vec![Inst::new(op::CONST_I32, &[Def(0), Imm(7)]), Inst::new(op::RETURN, &[])]);

    explicit_locals(&mut f);

    assert_eq!(dump(&f), ["%1 = CONST_I32 7", "DROP_I32 %1", "RETURN"].map(String::from));
}

/// From the pin's source: `ARGUMENT`s found after other instructions return to
/// the front, in order.
#[test]
fn arguments_move_to_the_front_of_the_entry_block() {
    let mut f = function(2, 3, vec![Inst::new(op::CONST_I32, &[Def(2), Imm(1)]), arg(0, 0), arg(1, 1)]);

    assert!(argument_move(&mut f));

    assert_eq!(dump(&f), ["%0 = ARGUMENT_i32 0", "%1 = ARGUMENT_i32 1", "%2 = CONST_I32 1"].map(String::from));
}

/// Stackified registers count down from `i32::MIN`, locals follow the
/// parameters, unread registers take nothing.
#[test]
fn registers_are_numbered_by_kind() {
    let mut f = function(
        1,
        3,
        vec![arg(0, 0), Inst::new(op::ADD_I32, &[Def(1), Use(0), Use(0)]), Inst::new(op::RETURN, &[Use(1)])],
    );
    f.stackify(1);

    let numbers = reg_numbering(&f);

    assert_eq!(numbers[0], 0);
    assert_eq!(numbers[1], i32::MIN as u32);
    assert_eq!(numbers[2], u32::MAX, "an unread register is unused");
}

/// From the pin's source: a `TEE` keeps its result on the stack and stores
/// the other copy in a local; the stack operand's tree starts at the
/// `local.get` that feeds the tee.
#[test]
fn a_tee_stores_one_copy_and_keeps_the_other() {
    let mut f = function(
        1,
        4,
        vec![
            arg(0, 0),
            Inst::new(post::TEE_I32, &[Def(1), Def(2), Use(0)]),
            Inst::new(op::ADD_I32, &[Def(3), Use(1), Use(2)]),
            Inst::new(op::RETURN, &[Use(3)]),
        ],
    );
    f.stackify(1);

    explicit_locals(&mut f);

    assert_eq!(
        dump(&f),
        [
            "%4 = LOCAL_GET_I32 0",
            "%1 = LOCAL_TEE_I32 1, %4",
            "%6 = LOCAL_GET_I32 1",
            "%5 = ADD_I32 %1, %6",
            "LOCAL_SET_I32 2, %5",
            "%7 = LOCAL_GET_I32 2",
            "RETURN %7",
        ]
        .map(String::from)
    );
}

/// From the pin's source: a `COPY` vanishes; the `local.get` that fed it
/// defines the copy's result instead.
#[test]
fn a_copy_is_replaced_by_its_source() {
    let mut f =
        function(1, 2, vec![arg(0, 0), Inst::new(post::COPY_I32, &[Def(1), Use(0)]), Inst::new(op::RETURN, &[Use(1)])]);
    f.stackify(1);

    explicit_locals(&mut f);

    assert_eq!(dump(&f), ["%1 = LOCAL_GET_I32 0", "RETURN %1"].map(String::from));
}

use llrm_wasm::reg_stackify::reg_stackify;

/// From the pin's source (`Optimize == false`): a definition directly above its
/// only use stays on the stack, so the load's result needs no local. The
/// pin's -O0 differs from LLVM 20's here, which does not run the pass.
#[test]
fn a_load_feeding_the_return_stays_on_the_stack() {
    let mut f = function(
        1,
        2,
        vec![
            arg(0, 0),
            Inst::new(op::LOAD_I32_A32, &[Def(1), Imm(2), Imm(0), Use(0)]),
            Inst::new(op::RETURN, &[Use(1)]),
        ],
    );

    assert!(reg_stackify(&mut f));
    explicit_locals(&mut f);

    assert_eq!(dump(&f), ["%2 = LOCAL_GET_I32 0", "%1 = LOAD_I32_A32 2, 0, %2", "RETURN %1"].map(String::from));
    assert!(f.locals.is_empty());
}

#[test]
fn a_chain_of_adds_is_one_tree() {
    let mut f = function(
        3,
        5,
        vec![
            arg(0, 2),
            arg(1, 1),
            arg(2, 0),
            Inst::new(op::ADD_I32, &[Def(3), Use(2), Use(1)]),
            Inst::new(op::ADD_I32, &[Def(4), Use(3), Use(0)]),
            Inst::new(op::RETURN, &[Use(4)]),
        ],
    );

    reg_stackify(&mut f);
    explicit_locals(&mut f);

    assert_eq!(
        dump(&f),
        [
            "%6 = LOCAL_GET_I32 0",
            "%5 = LOCAL_GET_I32 1",
            "%3 = ADD_I32 %6, %5",
            "%7 = LOCAL_GET_I32 2",
            "%4 = ADD_I32 %3, %7",
            "RETURN %4",
        ]
        .map(String::from)
    );
}

/// The first operand's definition is not adjacent, the second's is: the add is
/// commuted once so both stay on the stack.
#[test]
fn an_add_is_commuted_to_stackify_both_operands() {
    let mut f = function(
        0,
        3,
        vec![
            Inst::new(op::CONST_I32, &[Def(0), Imm(7)]),
            Inst::new(op::CONST_I32, &[Def(1), Imm(9)]),
            Inst::new(op::ADD_I32, &[Def(2), Use(1), Use(0)]),
            Inst::new(op::RETURN, &[Use(2)]),
        ],
    );

    reg_stackify(&mut f);

    assert!((0..3).all(|r| f.is_stackified(r)));
    assert_eq!(f.blocks[0][2].operands, [Def(2), Use(0), Use(1)], "the operands swapped");
}

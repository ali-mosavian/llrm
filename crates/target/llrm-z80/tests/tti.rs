//! The Z80 cost model's addressing modes: what one `ld` takes.

use llrm_mir::tti::*;
use llrm_z80::tti::Z80Tti;

/// A register alone and an absolute address are modes; a register plus a
/// displacement, two registers and a scaled index are not. With `Basic`'s
/// "nothing is legal", LSR panicked ("Formula is illegal") on any loop
/// over a global: 10 of the 23 bench kernels.
#[test]
fn a_register_alone_and_an_absolute_address_are_the_modes() {
    let tti = Z80Tti::new();
    // (gv, offset, base register, scale)
    assert!(tti.is_legal_addressing_mode(false, 0, true, 0), "ld a,(hl)");
    assert!(tti.is_legal_addressing_mode(false, 0, false, 1), "a lone index register is a register");
    assert!(tti.is_legal_addressing_mode(true, 0, false, 0), "ld a,(sym)");
    assert!(tti.is_legal_addressing_mode(true, 6, false, 0), "ld a,(sym+6)");
    assert!(!tti.is_legal_addressing_mode(false, 5, true, 0), "register plus displacement");
    assert!(!tti.is_legal_addressing_mode(true, 0, true, 0), "symbol plus register");
    assert!(!tti.is_legal_addressing_mode(false, 0, true, 1), "two registers");
    assert!(!tti.is_legal_addressing_mode(false, 0, false, 2), "scaled index");
}

fn arith(
    op: ArithOp,
    ty: Ty,
    kind: CostKind,
) -> i64 {
    Z80Tti::new().arithmetic_cost(op, ty, kind, OperandInfo::ANY, OperandInfo::ANY).value
}

/// Worked from the table by hand. `add a,b` is 4 T and 1 byte (1 unit); `add
/// hl,bc` is 11 T (3 units) and 1 byte; the word `and` is six instructions of 4
/// T each (6 units) and 6 bytes. Before this the model was the 6502's
/// table renamed: a word add priced as two byte operations, which it is not
/// here.
#[test]
fn prices_are_the_sums_of_the_table_rows() {
    use ArithOp::*;
    let (i8, i16) = (Ty::Int(8), Ty::Int(16));
    assert_eq!(arith(Add, i8, CostKind::RecipThroughput), 1);
    assert_eq!(arith(Add, i16, CostKind::RecipThroughput), 3);
    assert_eq!(arith(Add, i16, CostKind::CodeSize), 1);
    assert_eq!(arith(And, i16, CostKind::RecipThroughput), 6);
    assert_eq!(arith(And, i16, CostKind::CodeSize), 6);
    // push 11 + or 4 + sbc hl 15 + pop 10 = 40 T
    let tti = Z80Tti::new();
    assert_eq!(tti.cmp_sel_cost(CmpSel::ICmp, i16, CostKind::Latency).value, 10);
}

/// A call costs more than the instruction it replaces, a 16-bit one more than
/// an 8-bit one, and every kind and type the passes ask about is priced (an
/// absent table row panics).
#[test]
fn calls_cost_more_and_every_operation_is_priced() {
    use ArithOp::*;
    for kind in [CostKind::RecipThroughput, CostKind::Latency, CostKind::CodeSize, CostKind::SizeAndLatency] {
        for ty in [Ty::Int(8), Ty::Int(16), Ty::Int(32), Ty::Int(64), Ty::Pointer, Ty::Float, Ty::Double] {
            for op in [Add, Sub, Mul, SDiv, UDiv, SRem, URem, Shl, LShr, AShr, And, Or, Xor, FAdd, FSub, FMul, FDiv] {
                assert!(arith(op, ty, kind) >= 0, "{op:?} {ty:?} {kind:?}");
            }
        }
    }
    assert!(arith(Mul, Ty::Int(16), CostKind::Latency) > arith(Mul, Ty::Int(8), CostKind::Latency));
    assert!(arith(Mul, Ty::Int(8), CostKind::Latency) > arith(Add, Ty::Int(16), CostKind::Latency));
    assert!(arith(UDiv, Ty::Int(16), CostKind::Latency) > arith(Mul, Ty::Int(16), CostKind::Latency));
}

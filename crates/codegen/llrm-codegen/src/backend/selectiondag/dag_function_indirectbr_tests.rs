//! `indirectbr` and `blockaddress`: a computed goto's jump, the blocks it may
//! reach, and the symbol each address-taken block is defined as.

use super::dag_function::selected;
use crate::model::ir::{Loc, Operation, Space};

const COMPUTED: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@t = internal constant [2 x ptr] [ptr blockaddress(@f, %one), ptr blockaddress(@f, %two)]

define i32 @f(i32 %x) {
entry:
  %p = getelementptr [2 x ptr], ptr @t, i32 0, i32 %x
  %a = load ptr, ptr %p
  indirectbr ptr %a, [label %one, label %two]
one:
  ret i32 1
two:
  ret i32 2
}

define ptr @g() {
  ret ptr blockaddress(@f, %two)
}
";

fn done(name: &str) -> crate::backend::isel::Selected {
    let module = llrm_mir::parse::module(COMPUTED).expect("parses");
    selected(&module, name, &llrm_x86_m32::M32, crate::backend::cpu::profile("486").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0))
}

/// Before: the selector refused an `indirectbr` ("an indirect jump through
/// memory"), so a computed goto compiled on no target. Now the jump names the
/// blocks it may reach, and its block has them as successors.
#[test]
fn an_indirectbr_is_a_jump_naming_its_blocks() {
    let done = done("f");
    let insns = done.body.insns();
    let jump = insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .find(|what| what.op == Operation::Jump && what.indirect && what.computed)
        .expect("a computed jump");
    assert_eq!(jump.table.len(), 2, "{:?}", jump.table);
    let entry = done.body.blocks.iter().find(|block| block.at == done.body.entry).expect("the entry");
    assert_eq!(&entry.succ[..], &jump.table[..]);
}

/// Before: the address of a block was a table the module never emitted, and
/// the label of the block was not named by anything the code could read: each
/// address-taken block is defined as a symbol of its own.
#[test]
fn an_address_taken_block_is_defined_as_a_symbol() {
    let done = done("f");
    assert_eq!(done.body.address_symbols.len(), 2, "{:?}", done.body.address_symbols);
    for (at, name) in done.body.address_symbols.iter() {
        assert!(done.body.blocks.iter().any(|block| block.at == *at), "{name} names no block");
        assert!(name.starts_with("$BA"), "{name}");
    }
}

/// A block's address, read in another function, is that symbol's address: the
/// function that holds the block is not lowered yet, or at all, there.
#[test]
fn a_block_address_in_another_function_is_the_symbol() {
    let done = done("g");
    let insns = done.body.insns();
    let symbol = insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .flat_map(|what| what.sources.iter())
        .find_map(|source| match source {
            Loc::Imm(imm) => imm.address.filter(|address| address.space == Space::Segment),
            _ => None,
        })
        .expect("an address");
    assert!(symbol.index >= 1 << 48, "{symbol:?}");
}

/// Before: `goto *table[i]` on the segmented 16-bit target jumped through a
/// 4-byte cell (`JMP16m` was given the 32-bit width): a far jump, which the
/// object writer refused. A near code pointer is a word.
#[test]
fn a_computed_goto_through_a_table_jumps_through_a_word_at_16_bits() {
    let text = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-f80:16-n8:16:32\"
@t = internal constant [2 x ptr] [ptr blockaddress(@f, %one), ptr blockaddress(@f, %two)]
define i16 @f(i16 %x) addrspace(1) {
entry:
  %p = getelementptr [2 x ptr], ptr @t, i16 0, i16 %x
  %a = load ptr, ptr %p
  indirectbr ptr %a, [label %one, label %two]
one:
  ret i16 1
two:
  ret i16 2
}
";
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m16::M16, crate::backend::cpu::profile("386").expect("a cpu"), &[])
        .unwrap_or_else(|e| panic!("{}", e.0));
    let insns = done.body.insns();
    let jump = insns
        .iter()
        .filter_map(|one| one.what.as_ref())
        .find(|what| what.op == Operation::Jump && what.indirect)
        .expect("an indirect jump");
    assert!(matches!(&jump.sources[..], [Loc::Mem(cell)] if cell.width == 2), "{:?}", jump.sources);
}

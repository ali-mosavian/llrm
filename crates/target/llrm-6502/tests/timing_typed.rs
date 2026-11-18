//! `bytes_of` printed each operand and read it back through
//! `timing::instruction` to size it: one allocation per instruction, in every
//! size check of the peephole passes (`shift_loop_in_a` priced its candidates
//! this way: 3.5% of the instructions of `idioms` -O2). The size is read from
//! the typed operand.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use llrm_6502::asm::{Asm, Index, Inst, Op, Operand};
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::peephole_cells::bytes_of;
use llrm_6502::timing;

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(
        &self,
        layout: Layout,
    ) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(
        &self,
        pointer: *mut u8,
        layout: Layout,
    ) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// Every opcode of the table, found by trying every three letters.
fn opcodes() -> Vec<Op> {
    let mut found = Vec::new();
    for a in b'a'..=b'z' {
        for b in b'a'..=b'z' {
            for c in b'a'..=b'z' {
                let name = String::from_utf8(vec![a, b, c]).unwrap();
                found.extend(Op::parse(&name));
            }
        }
    }
    found
}

fn operands() -> Vec<Operand> {
    let names = ["rc5", "rc5+1", "rc", "rc1x", "label", "s_f_3", "__mulsi3"];
    let mut all = vec![Operand::None, Operand::Acc, Operand::Imm("1".into()), Operand::Imm("<(sym)".into())];
    for name in names {
        let name = name.to_string();
        all.extend([
            Operand::Direct(name.clone()),
            Operand::Indexed(name.clone(), Index::X),
            Operand::Indexed(name.clone(), Index::Y),
            Operand::IndirectY(name.clone()),
            Operand::IndirectX(name.clone()),
            Operand::Indirect(name),
        ]);
    }
    all
}

/// The typed price is the text price for every opcode, every operand form
/// and both CPUs (a form the text prints and reads back as another,
/// `Direct("a")`, is not one the passes build).
fn the_typed_price_is_the_text_price_for_every_form() {
    let mut priced = 0;
    for cpu in [Cpu::Mos6502, Cpu::W65c02] {
        for op in opcodes() {
            for operand in operands() {
                let text = operand.to_string();
                if Operand::parse(&text) != operand {
                    continue;
                }
                let inst = Inst::new(op, operand);
                let typed = timing::of(cpu, &inst);
                assert_eq!(typed, timing::instruction(cpu, op.name(), &text), "{cpu:?} {} {text}", op.name());
                priced += usize::from(typed.is_some());
            }
        }
    }
    assert!(priced > 500, "only {priced} forms priced: the table was not walked");
}

fn sizing_instructions_allocates_nothing_per_instruction() {
    let n = 2000;
    let text: String = (0..n).map(|_| "  lda rc4\n  sta rc5\n  ldy #3\n  lda (rc2),y\n").collect();
    let asm = Asm::parse(&text);
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes = bytes_of(&asm.lines);
    let made = ALLOCATIONS.load(Ordering::Relaxed) - before;
    assert_eq!(bytes, Some(n as u32 * 8));
    assert!(made < 10, "{made} allocations for {} instructions", 4 * n);
}

/// One test: the allocation counter is the process's.
#[test]
fn typed_sizes() {
    sizing_instructions_allocates_nothing_per_instruction();
    the_typed_price_is_the_text_price_for_every_form();
}

//! A constant word several nodes share is loaded into a pair once (3 B and 3 B once against 5 B a store), whichever
//! store is selected first.

use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_z80::compile::{merge_lone_constant_stores, text};
use llrm_z80::driver::{Level, Request};
use llrm_z80::regalloc::Class;

/// particle's `bench_particle`: `vz[i] = 1` and the loop counter's initial 1 share the word 1.
const PARTICLE: &str = include_str!("data/imm_shared_store.ll");

/// -Os moved 15 B with the order the DAG is selected in: the choice for the second of two stores of one constant read
/// the DAG after the first was selected (`hasOneUse`). The users are counted before selection now, so particle keeps
/// its smaller form (`ld (hl), #1` / `ld (hl), #0`) whichever order selects.
#[test]
fn a_shared_constant_word_is_stored_as_immediates_where_that_is_smaller() {
    let body = text(PARTICLE, "bench_particle", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    assert!(body.contains("ld (hl), #1"), "the immediate stores of the shared word 1 were lost:\n{body}");
}

const ONE_STORE: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
    target triple = \"msp430\"\n\
    define void @f(ptr %p) {\n  store volatile i16 1, ptr %p\n  ret void\n}\n";

/// One store of the constant stays two immediates: 5 B against 3 B and 3 B.
#[test]
fn a_constant_word_stored_once_is_two_immediates() {
    let body = text(ONE_STORE, "f", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    assert!(body.contains("ld (hl), #1"), "{body}");
}

/// kernel (`frame_address_stack_args` -O1): the word 0 was selected as a pair for a store whose other DAG users
/// became `adc`s of their own, and cost 9 B for the 8 B of two immediate stores. Made and stored once in a block, it
/// is stored as immediates without a second allocation; a constant two stores share stays the choice's.
#[test]
fn a_constant_made_and_stored_once_in_a_block_is_stored_as_immediates() {
    let store = |v, slot| Inst::new(Kind::Op("LD16fr"), vec![Operand::Use(Reg::Virt(v)), Operand::Frame(slot), Operand::Imm(0)]);
    let make = |v| Inst::new(Kind::Op("LD16ri"), vec![Operand::Def(Reg::Virt(v)), Operand::Imm(0)]);
    let mut f = MFunction::new("kernel");
    f.classes = vec![Class::Gr16; 2];
    f.blocks = vec![Block { id: 0, insts: vec![make(0), store(0, 0), make(1), store(1, 1), store(1, 2)], phis: Vec::new() }];
    assert!(merge_lone_constant_stores(&mut f));
    let kinds: Vec<Kind> = f.blocks[0].insts.iter().map(|inst| inst.kind).collect();
    assert_eq!(kinds, [Kind::Op("LD16fi"), Kind::Op("LD16ri"), Kind::Op("LD16fr"), Kind::Op("LD16fr")]);
}

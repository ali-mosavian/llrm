//! A spill store of a pair just reloaded from the same slot, with a use in
//! between, reached the output: `ld e,-24(ix); ld d,-23(ix); ld l,e; ld h,d;
//! ld -24(ix),e; ld -23(ix),d`. 95 of them in `div_constants_long` at -Os, 570
//! B. The pin's `RemoveDeadStores` takes only a store that follows its load at
//! once.

use llrm_lir::registers::RegId;
use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_spike_mf::regalloc::{Assignment, Target};
use llrm_spike_mf::spill::remove_redundant_slot_accesses;
use llrm_z80::regalloc::{Class, Z80Registers};
use llrm_z80::{REGISTER_INFO, reg};

const SLOT: i32 = 3;
const OTHER: i32 = 4;

fn load(
    v: u32,
    slot: i32,
) -> Inst {
    <Z80Registers as Target>::spill_load(Class::Gr16, Reg::Virt(v), slot).unwrap()
}

fn store(
    v: u32,
    slot: i32,
) -> Inst {
    <Z80Registers as Target>::spill_store(Class::Gr16, Reg::Virt(v), slot).unwrap()
}

fn read(v: u32) -> Inst {
    Inst::new(Kind::Op("USE"), vec![Operand::Use(Reg::Virt(v))])
}

fn write(v: u32) -> Inst {
    Inst::new(Kind::Op("DEF"), vec![Operand::Def(Reg::Virt(v)), Operand::Imm(0)])
}

fn call() -> Inst {
    Inst { call: true, ..Inst::new(Kind::Op("CALL"), vec![]) }
}

/// `insts` over virtual registers 0..4 living in `regs`; the instructions left.
fn run(
    insts: Vec<Inst>,
    regs: [RegId; 4],
) -> usize {
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Gr16; 4];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    remove_redundant_slot_accesses::<Z80Registers>(
        &mut function,
        &Assignment(regs.to_vec()),
        &REGISTER_INFO,
        &[SLOT, OTHER],
    );
    function.blocks[0].insts.len()
}

fn alike() -> [RegId; 4] {
    [reg::HL; 4]
}

#[test]
fn a_reload_stored_back_with_a_use_between_loses_the_store() {
    assert_eq!(run(vec![load(0, SLOT), read(0), store(1, SLOT)], alike()), 2);
}

#[test]
fn a_write_of_the_register_between_keeps_the_store() {
    assert_eq!(run(vec![load(0, SLOT), write(2), store(1, SLOT)], alike()), 3);
}

#[test]
fn a_call_between_keeps_the_store() {
    assert_eq!(run(vec![load(0, SLOT), call(), store(1, SLOT)], alike()), 3);
}

#[test]
fn a_store_of_another_register_keeps_the_store() {
    assert_eq!(run(vec![load(0, SLOT), read(0), store(1, SLOT)], [reg::HL, reg::DE, reg::HL, reg::HL]), 3);
}

#[test]
fn a_store_to_another_slot_keeps_the_store() {
    assert_eq!(run(vec![load(0, SLOT), read(0), store(1, OTHER)], alike()), 3);
}

#[test]
fn a_store_of_the_slot_from_another_register_makes_the_old_value_stale() {
    let insts = vec![load(0, SLOT), store(2, SLOT), read(0), store(1, SLOT)];
    assert_eq!(run(insts, [reg::HL, reg::HL, reg::DE, reg::HL]), 4);
}

#[test]
fn a_second_reload_into_the_same_register_goes() {
    assert_eq!(run(vec![load(0, SLOT), read(0), load(1, SLOT), read(1)], alike()), 3);
}

/// `g` loops over a 32-bit `x` live across the divide calls (a C function of
/// `x / 7`, `x % 7`, `x / dv`, `x % dv` and a sum).
#[test]
fn a_compiled_loop_stores_no_reload_back_to_its_slot() {
    use llrm_z80::compile::text;
    use llrm_z80::driver::{Level, Request};
    let ir = include_str!("kernels/ir/reload_stored_back.ll");
    let out = text(ir, "g", 0, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    let slot = |line: &str, prefix: &str| -> Option<(String, String)> {
        // `ld r, -N(ix)` is a reload (prefix "ld "), `ld -N(ix), r` a store.
        let rest = line.strip_prefix(prefix)?;
        let (a, b) = rest.split_once(", ")?;
        let (reg, mem) = if prefix == "ld " && !a.contains("(ix)") { (a, b) } else { (b, a) };
        mem.ends_with("(ix)").then(|| (reg.to_owned(), mem.to_owned()))
    };
    assert!(
        lines.iter().any(|line| line.starts_with("ld ") && line.ends_with("(ix)") && !line.starts_with("ld -")),
        "premise: it reloads\n{out}"
    );
    let mut repeated = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Some((reg, mem)) = slot(line, "ld ").filter(|(_, mem)| !line.starts_with(&format!("ld {mem}"))) else {
            continue;
        };
        for later in &lines[at + 1..lines.len().min(at + 6)] {
            // only plain loads between: anything else may change the register
            if !later.starts_with("ld ") {
                break;
            }
            if later.starts_with(&format!("ld {mem}, {reg}")) {
                repeated.push(format!("{line} ... {later}"));
            }
            if later.starts_with(&format!("ld {reg},")) || later.starts_with(&format!("ld {mem},")) {
                break;
            }
        }
    }
    assert!(repeated.is_empty(), "{repeated:#?}\n{out}");
}

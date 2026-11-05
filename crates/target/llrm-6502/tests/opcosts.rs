//! The machine's operation prices are what the assembler makes of the
//! sequences the backend emits.

use std::path::PathBuf;
use std::process::Command;

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::opcosts::{SNIPPETS, snippet};

/// The bytes ca65 and ld65 make of `text`.
fn assembled(
    class: &str,
    text: &str,
) -> i64 {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("opcosts-{class}"));
    std::fs::create_dir_all(&dir).unwrap();
    let equates: String = (0..16).map(|n| format!("rc{n} = ${:02X}\n", 0x80 + n)).collect();
    let source = format!("{equates}.export start\nf:\n  rts\nstart:\n  {text}");
    std::fs::write(dir.join("p.s"), source.replace(".import f\n", "")).unwrap();
    std::fs::write(
        dir.join("p.cfg"),
        "MEMORY { M: start=$0200, size=$1000, file=\"%O\"; }\nSEGMENTS { CODE: load=M, type=ro; }\n",
    )
    .unwrap();
    let steps: [(&str, &[&str]); 2] = [("ca65", &["p.s", "-o", "p.o"]), ("ld65", &["-C", "p.cfg", "-o", "p.bin", "p.o"])];
    for (program, args) in steps {
        let out = Command::new(program).args(args).current_dir(&dir).output().expect(program);
        assert!(out.status.success(), "{program}: {}", String::from_utf8_lossy(&out.stderr));
    }
    // the leading `rts` of `f` is one byte
    std::fs::metadata(dir.join("p.bin")).unwrap().len() as i64 - 1
}

/// The machine answered every price with 1: an `i8` add, a call and a 16-bit
/// address advance were all one unit, in clocks and in bytes. Each byte price
/// is now the assembled size of its sequence, each clock price the
/// estimator's.
#[test]
fn the_6502_prices_are_the_assembled_sizes_of_their_sequences() {
    let machine = llrm_6502::driver::machine();
    let (clocks, bytes) = (machine.costs(), machine.byte_costs());
    let price = |costs: &llrm_mir::target::OperationCosts, class: &str| match class {
        "add" => costs.add,
        "load" => costs.load,
        "store" => costs.store,
        "move" => costs.r#move,
        "shift" => costs.shift,
        "branch" => costs.branch,
        "address" => costs.address,
        "call" => costs.call,
        "return" => costs.return_,
        other => panic!("{other}"),
    };
    for (class, text) in SNIPPETS {
        assert_eq!(price(&bytes, class), assembled(class, text), "{class}: bytes against ca65");
        assert_eq!(price(&clocks, class), snippet(Cpu::Mos6502, class).0, "{class}: clocks");
        assert!(price(&clocks, class) > 1, "{class}: clocks are still the unit price");
    }
    assert_eq!((clocks.native_bits, bytes.native_bits), (8, 8));
}

/// The machine stated no registers (0), so no pressure was ever priced: a loop
/// holding more values than the register file had was never seen to spill.
/// The numbers are the allocator's pools, and the register file of
/// `registers.regs`: a, x, y, `rc2`..`rc15`, of which `rc8`..`rc15` survive a
/// call.
#[test]
fn the_6502_states_the_register_file_the_allocator_has() {
    let machine = llrm_6502::driver::machine();
    let (bytes, kept) = llrm_6502::alloc::register_file(0);
    assert_eq!((machine.registers(), machine.call_registers()), (bytes, kept));
    assert_eq!((bytes, kept), (3 + 14, 8), "a, x, y, rc2..rc15; rc8..rc15 kept");
    // a platform with more scratch cells has more bytes and the same kept ones
    assert_eq!(llrm_6502::alloc::register_file(24), (bytes + 8, kept));
}

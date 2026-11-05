//! `Machine::size_costs` was `OperationCosts::default()`: every operation one byte (an instruction count),
//! a store of a constant 1 where the instruction is 8 bytes, a `lea` 1 where it is 4. Each class is now the
//! encoder's length of the instructions it becomes, and binutils agrees with the encoder.

use std::process::Command;

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::{mc, opcosts};
use llrm_mir::target::Machine;

mod support;

#[test]
fn the_size_costs_are_bytes_not_one_per_operation() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let costs = opcosts::size_costs(cpu).expect("the forms encode");
        assert_eq!((costs.add, costs.shift, costs.load, costs.store), (2, 2, 4, 4), "{cpu:?}");
        assert_eq!((costs.store_immediate, costs.address, costs.call, costs.return_), (8, 4, 6, 2), "{cpu:?}");
    }
    let wide = |cpu| opcosts::size_costs(cpu).map(|costs| (costs.multiply, costs.divide)).unwrap();
    assert_eq!(wide(Cpu::M68000), (12, 12), "two pushes, jsr and the cleanup");
    assert_eq!(wide(Cpu::M68020), (4, 4), "muls.l and divs.l");
}

#[test]
fn the_machine_prices_size_by_cpu() {
    let machine = |cpu| llrm_m68k::machine::M68kMachine { cpu };
    assert_eq!(machine(Cpu::M68000).size_costs().multiply, 12);
    assert_eq!(machine(Cpu::M68020).size_costs().multiply, 4);
    assert_eq!(machine(Cpu::M68020).size_costs().store_immediate, 8);
}

#[test]
fn binutils_assembles_every_form_to_the_length_the_encoder_gives() {
    let Some(root) = support::oracle_root() else { return };
    let tool = |name: &str| {
        let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
        command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
        command
    };
    for (cpu, flag) in [(Cpu::M68000, "-m68000"), (Cpu::M68020, "-m68020")] {
        for (class, form) in opcosts::forms(cpu) {
            let text: String = form.iter().map(|one| format!("{}\n", mc::print(one).unwrap())).collect();
            let source = support::unique_path("opcosts-s");
            let object = support::unique_path("opcosts-o");
            std::fs::write(&source, format!(".text\n{text}")).unwrap();
            let assembled = tool("as").arg(flag).arg("-o").arg(&object).arg(&source).output().unwrap();
            assert!(assembled.status.success(), "{cpu:?} {class}: {text}{}", String::from_utf8_lossy(&assembled.stderr));
            let listing = tool("size").arg(&object).output().unwrap();
            let size: i64 = String::from_utf8_lossy(&listing.stdout).lines().nth(1).unwrap().split_whitespace().next().unwrap().parse().unwrap();
            let encoded: i64 = form.iter().map(|one| llrm_m68k::encode::encode(one).unwrap().bytes.len() as i64).sum();
            assert_eq!(size, encoded, "{cpu:?} {class}: binutils {size} B, the encoder {encoded} B:\n{text}");
        }
    }
}

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
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let costs = opcosts::size_costs(cpu).unwrap();
        assert_eq!((costs.branch, costs.argument, costs.pop, costs.function - llrm_m68k::opcosts::FUNCTION_RESIDUE_BYTES), (2, 2, 2, 4), "{cpu:?}: bra.b, a push, a pop, one saved register");
        assert_eq!((costs.float_add, costs.float_multiply, costs.float_divide), (34, 34, 34), "{cpu:?}: 4 words pushed, jsr, lea, 2 stores");
        assert_eq!((costs.float_compare, costs.float_to_int), (26, 20), "{cpu:?}");
        assert_eq!((costs.float_load, costs.float_store), (8, 8), "{cpu:?}: two longs");
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
            let mut text: String = form.iter().map(|one| format!("{}\n", mc::print(one).unwrap())).collect();
            // `bra 2` is a branch to address 2 for the assembler: a short branch over a `nop` is the form it sizes.
            if class == "branch" {
                text = "bra.b 1f\nnop\n1:\n".to_owned();
            }
            let source = support::unique_path("opcosts-s");
            let object = support::unique_path("opcosts-o");
            std::fs::write(&source, format!(".text\n{text}")).unwrap();
            let assembled = tool("as").arg(flag).arg("-o").arg(&object).arg(&source).output().unwrap();
            assert!(assembled.status.success(), "{cpu:?} {class}: {text}{}", String::from_utf8_lossy(&assembled.stderr));
            let listing = tool("size").arg(&object).output().unwrap();
            let size: i64 = String::from_utf8_lossy(&listing.stdout).lines().nth(1).unwrap().split_whitespace().next().unwrap().parse().unwrap();
            let mut encoded: i64 = form.iter().map(|one| llrm_m68k::encode::encode(one).unwrap().bytes.len() as i64).sum();
            if class == "branch" {
                encoded += 2;
            }
            assert_eq!(size, encoded, "{cpu:?} {class}: binutils {size} B, the encoder {encoded} B:\n{text}");
        }
    }
}

/// `size_costs` filled 15 of its fields; the others kept `OperationCosts::default()`: a conditional branch counted 1 B
/// (2 or 4), a soft-float operation 1 B (about 34), so a function's estimate was half its object. A field equal to its
/// default must be one `opcosts::AT_DEFAULT` names with a reason, and every name there must still be at its default.
#[test]
fn no_field_is_left_at_the_default_for_an_operation_the_encoder_sizes() {
    let fields = |costs: &llrm_mir::target::OperationCosts| -> Vec<(String, String)> {
        let text = format!("{costs:?}");
        let inner = text.split_once('{').unwrap().1.trim_end_matches(['}', ' ']);
        inner.split(", ").map(|pair| {
            let (name, value) = pair.split_once(": ").unwrap();
            (name.trim().to_owned(), value.to_owned())
        }).collect()
    };
    let default = fields(&llrm_mir::target::OperationCosts::default());
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let ours = fields(&opcosts::size_costs(cpu).unwrap());
        let unpriced: Vec<&str> = default.iter().zip(&ours).filter(|(d, o)| d.1 == o.1).map(|(d, _)| d.0.as_str()).collect();
        let named: Vec<&str> = opcosts::AT_DEFAULT.iter().map(|(name, _)| *name).collect();
        let missing: Vec<&&str> = unpriced.iter().filter(|name| !named.contains(name)).collect();
        assert!(missing.is_empty(), "{cpu:?}: left at the default with no reason: {missing:?}");
        let stale: Vec<&&str> = named.iter().filter(|name| !unpriced.contains(name)).collect();
        assert!(stale.is_empty(), "{cpu:?}: named in AT_DEFAULT but priced: {stale:?}");
    }
}

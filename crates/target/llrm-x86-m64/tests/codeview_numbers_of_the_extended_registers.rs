//! `-g` at m64 wrote no COFF object for a function with a variable in r8d-r15d:
//! the sub-registers had no CodeView number, so the CodeView writer refused the
//! whole object ("register r15d has no CodeView number").
//!
//! Numbers: LLVM's `CodeViewRegisters.def`, R8B..R15B 344..351 (lines 255-262),
//! R8W..R15W 352..359 (280-287), R8D..R15D 360..367 (305-312).

const REGISTERS: &str = include_str!("../src/registers.regs");

fn codeview(name: &str) -> Option<u16> {
    let registers = llrm_target::registers::parse(REGISTERS).expect("registers.regs parses");
    registers.iter().find(|one| one.name == name).unwrap_or_else(|| panic!("{name} is not in the file")).codeview
}

#[test]
fn test_every_extended_sub_register_has_the_pins_codeview_number() {
    for index in 0..8u16 {
        let n = 8 + index;
        for (suffix, base) in [("l", 344), ("w", 352), ("d", 360)] {
            let name = format!("r{n}{suffix}");
            assert_eq!(codeview(&name), Some(base + index), "{name}");
        }
    }
}

//! `Machine::costs` was `OperationCosts::default()`: one clock an operation. A
//! 32-bit multiply on the 68000 (a `__mulsi3` call of 162 or more clocks) and
//! an `add` cost the same, so every clock decision weighed a loop's
//! saving as an instruction count against bytes that were real. Each class is
//! now the clocks of the instructions it becomes, from the manuals (MC68000UM
//! section 8 instruction times; MC68020UM section 8, cache case).

use llrm_m68k::clockcosts::{MULTIPLIER_BITS, clock_costs};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::machine::M68kMachine;
use llrm_mir::target::Machine;

/// The 68000 cells are written down from MC68000UM section 8 and not read back
/// through `timing`: `add.l d1,d0` 8, `lsl.l #2,d0` 12 (long: 8 + 2n), `lea
/// (4,a0),a1` 8, `move.l (4,a0),d0` 16, `move.l d0,(4,a0)` 16, `move.l d0,d1`
/// 4, `jsr` to a label 20, `rts` 16, a taken `bra.b` 10. The 68020 cells are
/// `timing`'s (MC68020UM section 8, cache case).
#[test]
fn the_native_operations_cost_what_the_manuals_say() {
    let c = clock_costs(Cpu::M68000).unwrap();
    assert_eq!((c.add, c.shift, c.address, c.load, c.store, c.r#move), (8, 12, 8, 16, 16, 4));
    assert_eq!((c.call, c.return_, c.branch), (20, 16, 10));
    let c = clock_costs(Cpu::M68020).unwrap();
    assert_eq!((c.add, c.shift, c.address, c.load, c.store, c.r#move), (2, 4, 4, 7, 5, 2));
    assert_eq!((c.call, c.return_, c.branch), (7, 10, 6));
}

/// A 32-bit multiply at the 68000 is two pushes, `jsr`, the cleanup and the
/// routine at `MULTIPLIER_BITS` (`routine::mulsi3_clocks`): 624 for the
/// routine. Priced at the `jsr` alone it was 18 clocks beside an `add`'s 8.
#[test]
fn a_multiply_without_the_instruction_costs_its_library_routine() {
    let c = clock_costs(Cpu::M68000).unwrap();
    let routine = i64::from(llrm_m68k::routine::mulsi3_clocks(Cpu::M68000, MULTIPLIER_BITS));
    assert_eq!(routine, 624);
    assert!(c.multiply > routine + c.call, "{c:?}");
    assert!(
        c.multiply > 50 * c.add,
        "a multiply is dearer than an add by two orders: {} against {}",
        c.multiply,
        c.add
    );
    let c = clock_costs(Cpu::M68020).unwrap();
    assert!(c.multiply < 60, "muls.l is one instruction: {}", c.multiply);
}

#[test]
fn the_machine_prices_clocks_by_cpu() {
    let machine = |cpu| M68kMachine { cpu };
    assert_eq!(machine(Cpu::M68000).costs().add, 8);
    assert_eq!(machine(Cpu::M68020).costs().add, 2);
}

/// A soft-float operation was priced at its pushes and a `jsr` (156 clocks of
/// `__adddf3`'s 2,200 at the 68000), a divide at the 68000 at 52 against a
/// routine of 634 to 2,216. Each adds the routine's measured clocks.
#[test]
fn a_library_operation_costs_its_routine_and_an_instruction_does_not() {
    let c = clock_costs(Cpu::M68000).unwrap();
    assert!(c.float_add > 2000 && c.float_multiply > 6000 && c.float_divide > 17000, "{c:?}");
    assert!(c.float_compare > 1000 && c.float_to_int > 800, "{c:?}");
    assert!(c.divide > 1300, "{}", c.divide);
    let c = clock_costs(Cpu::M68020).unwrap();
    assert!(c.divide < 100, "divs.l is one instruction: {}", c.divide);
    assert!(c.float_add > 1000, "{}", c.float_add);
}

/// The sources the measured routine clocks were taken from: `int32.c`,
/// `fp/*.inc`, `fp/f64/*`, FNV-1a over their contents in path order. Fails when
/// any of them changes: measure again (`MEASURED`) and set the stamp.
#[test]
fn the_measured_clocks_are_those_of_the_shipped_routines() {
    let lib = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/compiler-rt/lib");
    let sorted = |dir: &str, keep: &dyn Fn(&str) -> bool| {
        let mut names: Vec<String> = std::fs::read_dir(format!("{lib}/{dir}"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| keep(name) && std::path::Path::new(&format!("{lib}/{dir}/{name}")).is_file())
            .collect();
        names.sort();
        names.into_iter().map(|name| format!("{lib}/{dir}/{name}")).collect::<Vec<_>>()
    };
    let mut files = vec![format!("{lib}/int/int32.c")];
    files.extend(sorted("fp", &|name| name.ends_with(".inc")));
    files.extend(sorted("fp/f64", &|_| true));
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for file in &files {
        for byte in std::fs::read(file).unwrap() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    assert_eq!(
        hash,
        llrm_m68k::clockcosts::SOURCE_STAMP,
        "{} files: the runtime changed, measure its routines again",
        files.len()
    );
}

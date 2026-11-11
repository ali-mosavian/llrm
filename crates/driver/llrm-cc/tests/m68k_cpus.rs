//! `LLRM_CC_TARGET=m68k-68040` was "no target": the table listed only the
//! 68000 and the 68020, so no C program could be compiled for the other four
//! CPUs the m68k backend accepts, and the sweeps could not price them.

#[test]
fn every_cpu_the_m68k_backend_takes_is_a_driver_target() {
    let table = llrm_cc::targets(include_str!("../targets.toml")).unwrap();
    for cpu in ["68000", "68010", "68020", "68030", "68040", "68060"] {
        let target = table.get(&format!("m68k-{cpu}")).unwrap_or_else(|| panic!("no target m68k-{cpu}"));
        assert_eq!(target.llc, [format!("-mcpu={cpu}")], "m68k-{cpu} compiles for its own CPU");
        assert!(target.clang.contains(&format!("-mcpu={cpu}")), "m68k-{cpu} asks clang for its own CPU");
    }
}

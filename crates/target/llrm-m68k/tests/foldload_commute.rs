//! A sum of loaded values kept `move.l (%a0)+, %d2; add.l %d0, %d2; move.l
//! %d2, %d0` (6 B and a saved register) because the pin marks `ADD32dd`
//! commutable and not `ADD32dr`, so the loaded tied operand could not become
//! the memory source: 30 B against gcc's 20 B (bench/kernels/arrsum).

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn a_loaded_addend_is_added_from_post_increment_whichever_operand_it_is() {
    let out =
        assembly(include_str!("../bench/kernels/arrsum.ll"), cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    // any address register: #7787 let the pointer take %a1, and the test named
    // %a0
    let folded = out
        .lines()
        .any(
            |line| {
                let line = line.trim_start();
                line.starts_with("add.l\t(%a") && line.contains(")+, %d")
            },
        );
    assert!(folded, "the load stayed a separate move:\n{out}");
}

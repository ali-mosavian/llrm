//! `s += i` tied the add to the counter that outlives it, so the loop paid
//! `move.l %d1,%d2; add.l %d0,%d2; move.l %d2,%d0` for `add.l %d1,%d0` (sum
//! 68000 -O2: 40 B against gcc's 18). `ADD32dr` is not marked commutative by
//! the pin; it is now where the result returns to the source that dies.
//! `fib` guards the rotation `b' = a + b; a' = b`, where swapping the tie
//! made the loop a copy longer.
//! The fixtures are clang's output with no passes, what `llrm-cc` hands the
//! driver.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SUM: &str = include_str!("fixtures/sum_raw.ll");
const FIB: &str = include_str!("fixtures/fib_raw.ll");

/// The instructions from the target of the last backward branch up to it.
fn loop_body(asm: &str) -> Vec<&str> {
    let lines: Vec<&str> = asm.lines().collect();
    let branch = lines.iter().rposition(|line| line.starts_with("\tb") && line.contains(".LBB")).expect("a branch");
    let target = lines[branch].split_whitespace().last().unwrap();
    let label = lines.iter().position(|line| line.strip_suffix(':') == Some(target)).expect("its label");
    lines[label + 1..=branch].iter().copied().filter(|line| line.starts_with('\t')).collect()
}

#[test]
fn a_sum_loop_adds_in_place() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(SUM, cpu_of(cpu).unwrap(), level).expect("compiles");
            let body = loop_body(&out);
            assert_eq!(body.len(), 4, "{cpu}: add, addq, subq, bne expected:\n{out}");
            assert!(body.iter().any(|line| line.starts_with("\tadd.l\t%d1, %d0")), "{cpu}: not in place:\n{out}");
        }
    }
}

#[test]
fn a_fibonacci_loop_is_no_longer_after_the_swap() {
    for level in [Level::O2, Level::Os] {
        let out = assembly(FIB, cpu_of("68000").unwrap(), level).expect("compiles");
        let body = loop_body(&out);
        let copies =
            body.iter().filter(|line| line.starts_with("\tmove.l\t%d") || line.starts_with("\tmove.l\t%a")).count();
        assert!(copies <= 2, "the rotation grew a copy:\n{out}");
    }
}

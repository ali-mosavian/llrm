//! A flag test on a byte in memory, `if (p->flags & 1)`, loaded the byte, masked it
//! and branched: `move.b (1,%a0),%d0; and.b #1,%d0; bne` (8 B, clobbers %d0). gcc:
//! `btst #0,(1,%a0)` (6 B, 16 clk against 20 on the 68000). QB `suggest.c` had 65
//! such `and.b` and `declare.c` 22 `and.l` (gcc: 2 and 0).

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::timing::cost;

const CPUS: [&str; 6] = ["68000", "68010", "68020", "68030", "68040", "68060"];
const DATALAYOUT: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16""#;

fn test_of(mask: u32) -> String {
    format!(
        r#"{DATALAYOUT}
declare void @a(i32)
define void @k(ptr %p) {{
  %f = load i8, ptr %p
  %m = and i8 %f, {mask}
  %t = icmp ne i8 %m, 0
  br i1 %t, label %yes, label %no
yes:
  call void @a(i32 1)
  ret void
no:
  ret void
}}
"#
    )
}

/// The premise, from the tables: `btst #k,(d16,An)` is no dearer than the load plus
/// `and.b #imm,Dn` in clocks on every CPU.
#[test]
fn btst_on_memory_takes_no_more_clocks_than_a_load_and_an_and() {
    for name in CPUS {
        let cpu = cpu_of(name).unwrap();
        let btst = cost(cpu, "btst #0, (1,%a0)", 6, false).unwrap().clocks;
        let load = cost(cpu, "move.b (1,%a0), %d0", 4, false).unwrap().clocks;
        let and = cost(cpu, "and.b #1, %d0", 4, false).unwrap().clocks;
        assert!(btst <= load + and, "{name}: btst {btst} clk against {load} + {and}");
    }
}

#[test]
fn a_one_bit_mask_on_a_loaded_byte_is_a_btst_on_the_byte_in_memory() {
    for name in CPUS {
        for level in [Level::O1, Level::O2, Level::Os] {
            for mask in [1, 2, 64] {
                let out = assembly(&test_of(mask), cpu_of(name).unwrap(), level).expect("compiles");
                let bit = mask.trailing_zeros();
                assert!(out.contains(&format!("btst\t#{bit}, (")), "{name} {level:?} mask {mask}: no btst on memory:\n{out}");
                assert!(!out.contains("and.b"), "{name} {level:?} mask {mask}: the and stayed:\n{out}");
            }
        }
    }
}

/// A mask of two bits is not a bit test: it stays a load and an `and`.
#[test]
fn a_two_bit_mask_is_not_a_bit_test() {
    let out = assembly(&test_of(3), cpu_of("68020").unwrap(), Level::Os).expect("compiles");
    assert!(!out.contains("btst"), "{out}");
}

/// The sign bit of a byte is a `tst.b` of it (2 B), cheaper than any `btst`.
#[test]
fn the_sign_bit_of_a_loaded_byte_is_a_tst() {
    let out = assembly(&test_of(128), cpu_of("68020").unwrap(), Level::Os).expect("compiles");
    assert!(out.contains("tst.b") && !out.contains("btst"), "{out}");
}

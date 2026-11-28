//! When an index and a pointer need the same registers, the pointer wins: an
//! indexed address `(d8,An,Xn)` takes more clocks than `(An)` on every CPU
//! (MC68000UM Table 8-2: 14 against 8 for a long; the 68020 and up add their
//! own). LSR left `local[11 - i]` as `(d,%sp,%d4)` beside the `(%a2)+` walk of
//! `local[i]` at the price of one pointer and one index; the two pointers cost
//! the same registers and 6 clk less per access (frameless -O1 +68 clk, bintree
//! -O1 +1784 clk against the pointer form, each for 2 to 4 B).

use llrm_m68k::driver::{Level, assembly, cpu_of};
use llrm_m68k::machine::index_is_dearer_than_plain;
use llrm_m68k::timing::cost;

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare i32 @add4(i32, i32, i32, i32)
define i32 @chain(i32 %n) {
  %local = alloca [12 x i32], align 2
  br label %fill
fill:
  %i = phi i32 [ 0, %0 ], [ %i.next, %fill ]
  %v = add nsw i32 %n, %i
  %p = getelementptr inbounds [12 x i32], ptr %local, i32 0, i32 %i
  store i32 %v, ptr %p, align 2
  %i.next = add nuw nsw i32 %i, 1
  %fill.done = icmp eq i32 %i.next, 12
  br i1 %fill.done, label %sum, label %fill
sum:
  %j = phi i32 [ 0, %fill ], [ %j.next, %sum ]
  %s = phi i32 [ 0, %fill ], [ %s.next, %sum ]
  %up = getelementptr inbounds [12 x i32], ptr %local, i32 0, i32 %j
  %a = load i32, ptr %up, align 2
  %rev = sub nsw i32 11, %j
  %down = getelementptr inbounds [12 x i32], ptr %local, i32 0, i32 %rev
  %b = load i32, ptr %down, align 2
  %r = call i32 @add4(i32 %a, i32 %b, i32 %n, i32 %j)
  %s.next = add nsw i32 %s, %r
  %j.next = add nuw nsw i32 %j, 1
  %sum.done = icmp eq i32 %j.next, 12
  br i1 %sum.done, label %out, label %sum
out:
  ret i32 %s.next
}
"#;

/// The premise, read from the timing tables: `(d8,An,Xn)` is dearer than
/// `(An)` on every CPU, in clocks where the table has them (not the 68060's:
/// 1 clk each) and in the extension word's bytes.
#[test]
fn an_indexed_address_is_dearer_than_a_plain_one_on_every_cpu() {
    for name in ["68000", "68010", "68020", "68030", "68040", "68060"] {
        let cpu = cpu_of(name).unwrap();
        let plain = cost(cpu, "move.l (%a1), %d0", 2, false).unwrap().clocks;
        let indexed = cost(cpu, "move.l (0,%a1,%d1), %d0", 4, false).unwrap().clocks;
        assert!(indexed >= plain, "{name}: (d8,An,Xn) {indexed} clk against (An) {plain} clk");
        assert!(index_is_dearer_than_plain(cpu), "{name}: {indexed} clk, 4 B against {plain} clk, 2 B");
    }
}

#[test]
fn a_stack_array_read_up_and_down_is_two_pointers_not_a_pointer_and_an_index() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O1, Level::O2] {
            let out = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            let indexed: Vec<&str> = out.lines().filter(|l| l.contains(",%sp,%d")).collect();
            assert!(
                indexed.is_empty(),
                "{cpu} {level:?}: an indexed stack address where a pointer costs the same registers:\n{out}"
            );
        }
    }
}

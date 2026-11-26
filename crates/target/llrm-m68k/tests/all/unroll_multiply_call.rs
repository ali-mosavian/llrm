//! A 64-trip loop storing `i * k + 1` was unrolled in full on the 68000 at
//! -O2/-O3: 64 multiplies of `k`, each a chain of shifts and adds with the
//! partial results spilled, where the rolled loop strength-reduces the product
//! to an add (a 64-trip nest: 1,672 B and 199,200 clk against 96 B and 96,556).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const HEAD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

/// The loop alone over a local array, `k` an argument.
const FLAT: &str = "declare void @use(ptr, i32)
define void @k(i32 %k) {
entry:
  %p = alloca [64 x i32]
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]
  %m = mul i32 %i, %k
  %v = add i32 %m, 1
  %q = getelementptr i32, ptr %p, i32 %i
  store i32 %v, ptr %q
  %n = add i32 %i, 1
  %c = icmp eq i32 %n, 64
  br i1 %c, label %done, label %loop
done:
  call void @use(ptr %p, i32 %k)
  ret void
}
";

/// The same loop inside a loop of known trips, `k` its counter.
const NESTED: &str = "declare void @use(ptr, i32)
define void @k() {
entry:
  %p = alloca [64 x i32]
  br label %outer
outer:
  %k = phi i32 [ 0, %entry ], [ %kn, %latch ]
  br label %loop
loop:
  %i = phi i32 [ 0, %outer ], [ %n, %loop ]
  %m = mul i32 %i, %k
  %v = add i32 %m, 1
  %q = getelementptr i32, ptr %p, i32 %i
  store i32 %v, ptr %q
  %n = add i32 %i, 1
  %c = icmp eq i32 %n, 64
  br i1 %c, label %latch, label %loop
latch:
  call void @use(ptr %p, i32 %k)
  %kn = add i32 %k, 1
  %d = icmp eq i32 %kn, 40
  br i1 %d, label %done, label %outer
done:
  ret void
}
";

fn instructions(
    body: &str,
    level: Level,
) -> usize {
    let text = format!("{HEAD}{body}");
    let out = assembly(&text, cpu_of("68000").unwrap(), level).expect("compiles");
    out.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count()
}

#[test]
fn a_counter_times_an_invariant_stays_rolled_where_the_copies_keep_the_multiply() {
    for level in [Level::O2, Level::O3] {
        for (name, body) in [("flat", FLAT), ("nested", NESTED)] {
            let n = instructions(body, level);
            assert!(n < 100, "{name} {level:?}: {n} instructions, the loop was copied 64 times");
        }
    }
}

#[test]
fn a_multiply_is_a_call_only_where_there_is_no_long_multiply_and_no_chain() {
    use llrm_m68k::dagisel::hooks::Cpu;
    use llrm_m68k::machine::M68k;
    use llrm_mir::tti::Tti;
    let m68000 = M68k::for_cpu(Cpu::M68000);
    let m68020 = M68k::for_cpu(Cpu::M68020);
    assert!(m68000.multiply_by_constant_clocks(Some(11)) > 100, "11 has no chain: `__mulsi3`");
    assert!(m68000.multiply_by_constant_clocks(None) > 100, "an unknown factor is a call");
    assert_eq!(m68000.multiply_by_constant_clocks(Some(5)), 0, "5 is a chain");
    assert_eq!(m68000.multiply_by_constant_clocks(Some(1)), 0);
    assert_eq!(m68020.multiply_by_constant_clocks(Some(11)), 0, "the 68020 has `muls.l`");
}

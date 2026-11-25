//! A pointer stepped by a constant lived in a data register between two
//! copies (`move.l %a0, %d2; addq.l #6, %d2; move.l %d2, %a0`, 3 instructions,
//! a register and 10 bytes); it is one instruction in the address register,
//! chosen by cost. A counter decremented by `add #-1` is `subq`.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

use crate::support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// Sums `n` longs read `stride` bytes apart.
fn walk(stride: i64) -> String {
    format!(
        "define i32 @f(ptr %p, i32 %n) {{
entry:
  br label %l
l:
  %q = phi ptr [ %p, %entry ], [ %q1, %l ]
  %i = phi i32 [ %n, %entry ], [ %i1, %l ]
  %s = phi i32 [ 0, %entry ], [ %s1, %l ]
  %v = load i32, ptr %q
  %s1 = add i32 %s, %v
  %q1 = getelementptr i8, ptr %q, i32 {stride}
  %i1 = sub i32 %i, 1
  %c = icmp eq i32 %i1, 0
  br i1 %c, label %x, label %l
x:
  ret i32 %s1
}}"
    )
}

fn text(
    ir: &str,
    cpu: Cpu,
) -> String {
    text_with(&module(ir), "f", cpu, Options::default()).expect("compiles")
}

/// The address register that `text`'s loop sums through: `add.l (%aN), %d0`.
fn pointer(text: &str) -> &str {
    let (_, rest) = text.split_once("add.l\t(").expect("the loop adds through a pointer");
    rest.split_once(')').expect("a closing parenthesis").0
}

/// The step is added in the address register: `addq` for 1 to 8, `lea` up to
/// 16 bits, `adda` beyond; the counter's `add #-1` is `subq`. Which address
/// register holds the pointer is the allocator's choice (`%a1` since the
/// scratch order changed; the test said `%a0`), so it is read off the load.
#[test]
fn a_pointer_step_is_one_instruction_in_the_address_register() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for (stride, step) in [(6, "addq.l\t#6, "), (12, "lea\t(12,"), (40000, "adda.l\t#40000, ")] {
            let out = text(&walk(stride), cpu);
            let a = pointer(&out);
            let want = if step.starts_with("lea") { format!("{step}{a}), {a}") } else { format!("{step}{a}") };
            assert!(out.contains(&want), "{cpu:?} stride {stride}: want {want}:\n{out}");
            assert!(
                !out.contains(&format!("{a}, %d")) && !out.contains(&format!(", {a}\n\tmove")),
                "{cpu:?} stride {stride}:\n{out}"
            );
        }
        let out = text(&walk(6), cpu);
        assert!(out.contains("subq.l\t#1, %d1") && !out.contains("add.l\t#-1"), "{cpu:?}:\n{out}");
    }
}

/// A pointer that steps once per outer iteration, past an inner loop, is a phi
/// of either register class; the pass left it as `move.l %a0, %d1; addq.l #1,
/// %d1; move.l %d1, %a0` (a byte checksum loop, 3 instructions per byte).
#[test]
fn a_pointer_stepped_after_an_inner_loop_is_one_addq() {
    let ir = "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %outer
outer:
  %q = phi ptr [ %p, %entry ], [ %q1, %inner.end ]
  %i = phi i32 [ %n, %entry ], [ %i1, %inner.end ]
  %h = phi i32 [ 0, %entry ], [ %h2, %inner.end ]
  %b = load i8, ptr %q
  %z = zext i8 %b to i32
  %h0 = xor i32 %h, %z
  br label %inner
inner:
  %k = phi i32 [ 8, %outer ], [ %k1, %inner ]
  %g = phi i32 [ %h0, %outer ], [ %g1, %inner ]
  %g1 = shl i32 %g, 1
  %k1 = sub i32 %k, 1
  %c = icmp eq i32 %k1, 0
  br i1 %c, label %inner.end, label %inner
inner.end:
  %h2 = add i32 %g1, 1
  %q1 = getelementptr i8, ptr %q, i32 1
  %i1 = sub i32 %i, 1
  %e = icmp eq i32 %i1, 0
  br i1 %e, label %x, label %outer
x:
  ret i32 %h2
}";
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text(ir, cpu);
        let bounced = out.lines().any(|line| line.starts_with("\tmove.l\t%a") && line.contains(", %d"));
        assert!(out.contains("addq.l\t#1, %a") && !bounced, "{cpu:?}:\n{out}");
    }
}

/// Exit status of `f(args)`; `None` without an optional qemu.
fn status(
    ir: &str,
    args: &[i64],
) -> Option<i32> {
    let qemu = support::qemu()?;
    let mut all = start("f", args);
    all.extend(items_with(&module(ir), "f", Cpu::M68000, Options::default()).expect("compiles"));
    let path = support::unique_path("addrarith");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    assert!(code.is_some(), "f({args:?}) ended on a signal");
    code
}

/// A function that fills a buffer on its stack with `offset / 4 + 1` at each
/// multiple of 4, then sums `n` longs read `stride` bytes apart.
fn filled_walk(stride: i64) -> String {
    format!(
        "define i32 @f(i32 %n) {{
entry:
  %buf = alloca [512 x i8]
  br label %fill
fill:
  %o = phi i32 [ 0, %entry ], [ %o1, %fill ]
  %p = getelementptr i8, ptr %buf, i32 %o
  %w = lshr i32 %o, 2
  %v = add i32 %w, 1
  store i32 %v, ptr %p
  %o1 = add i32 %o, 4
  %done = icmp eq i32 %o1, 512
  br i1 %done, label %go, label %fill
go:
  br label %l
l:
  %q = phi ptr [ %buf, %go ], [ %q1, %l ]
  %i = phi i32 [ %n, %go ], [ %i1, %l ]
  %s = phi i32 [ 0, %go ], [ %s1, %l ]
  %x = load i32, ptr %q
  %s1 = add i32 %s, %x
  %q1 = getelementptr i8, ptr %q, i32 {stride}
  %i1 = sub i32 %i, 1
  %c = icmp eq i32 %i1, 0
  br i1 %c, label %x, label %l
x:
  ret i32 %s1
}}"
    )
}

/// The values: each kind of step (`addq`, `lea`) reads the words it should, and
/// the counter that is `subq` counts the right number of trips.
#[test]
fn walking_by_each_kind_of_step_reads_the_same_words() {
    for stride in [4i64, 8, 12, 100] {
        for n in [1i64, 2, 3, 5] {
            let want: i64 = (0..n).map(|k| k * stride / 4 + 1).sum();
            if let Some(code) = status(&filled_walk(stride), &[n]) {
                assert_eq!(i64::from(code), want & 0xFF, "stride {stride}, n {n}");
            }
        }
    }
}

/// `adda.l #N, %aX` read as the `lea (N,%aX), %aX` it replaces.
fn as_lea(text: &str) -> String {
    text.lines()
        .map(|line| match line.strip_prefix("\tadda.l\t#").and_then(|rest| rest.split_once(", ")) {
            Some((imm, reg)) => format!("\tlea\t({imm},{reg}), {reg}"),
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the allocation shows, without what each CPU prices on its own after
/// it: the call form (`jsr (%aN)` through a register loaded once, or `jsr
/// (name,%pc)`) and the frame's save and restore, which holds that register.
/// The 68040 and the 68020 price a call target differently
/// (`calltarget.rs`), so their text differs there by design.
fn allocation(text: &str) -> String {
    let own_pricing = |line: &&str| {
        line.starts_with("\tjsr")
            || (line.starts_with("\tmovem.l\t") && (line.ends_with("-(%sp)") || line.contains("(%sp)+")))
            || line.strip_prefix("\tlea\t").is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_alphabetic()))
    };
    as_lea(text).lines().filter(|line| !own_pricing(line)).collect::<Vec<_>>().join("\n")
}

/// On the 68040 `adda.l #N` costs one clock and `lea (N,An)` two, and the
/// tied `adda` was chosen before allocation: it changed the allocation of the
/// whole function, and seed 277 came out 18 bytes larger and 8 clocks slower
/// than at the 68020 (154 assembly lines differed beyond the adds). The form
/// is chosen after allocation now, so the two CPUs allocate alike. Seeds 277
/// and 9 stopped using `adda` at the 68040 (the premise gone, not the bug:
/// cast sinking and extension promotion changed their code), so the fixture
/// is `pointer-streams.ll`, five pointers bumped by constants no `addq`
/// takes, which uses it by construction.
#[test]
fn the_cpu_that_prefers_adda_to_lea_allocates_as_the_one_that_does_not() {
    use llrm_m68k::driver::{Level, assembly, cpu_of};
    let ir = include_str!("../data/pointer-streams.ll");
    let on = |cpu: &str| assembly(ir, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
    let (m68020, m68040) = (on("68020"), on("68040"));
    assert!(m68040.contains("adda.l"), "the 68040 takes adda somewhere");
    assert_eq!(allocation(&m68040), allocation(&m68020));
}

/// A pointer plus a register that only addresses read was added in a data
/// register and copied into each address register that used it
/// (`move.l %a0, %d0; add.l %d2, %d0; move.l %d0, %a1`, bench/library/partition
/// kept six saved registers). It is `lea (0,%a0,%d2)`.
#[test]
fn a_pointer_plus_a_register_read_only_as_an_address_is_an_indexed_lea() {
    let ir = include_str!("../../bench/library/partition.ll");
    let m = llrm_mir::parse::module(ir).expect("parses");
    let out = text_with(&m, "k", Cpu::M68000, Options { size: true, ..Options::default() }).expect("compiles");
    assert!(out.contains("lea\t(0,%a"), "no indexed lea:\n{out}");
}

/// A sum that is stored, not used as an address, stayed data: the store through
/// `(An)+` names its value last, and the pass took that last register for the
/// store's base, made the sum an indexed `lea` into an address register and
/// encoded `move.l %a2, (%a0)+` as `move.l %d2, (%a0)+` (a local array filled
/// with `n + i` read back 0).
#[test]
fn a_sum_stored_through_post_increment_is_not_made_an_address() {
    use llrm_m68k::compile::{Options, text_with};
    use llrm_m68k::dagisel::hooks::Cpu;
    let ir = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %n) {
entry:
  %a = alloca [4 x i32]
  br label %l
l:
  %i = phi i32 [0, %entry], [%i1, %l]
  %p = getelementptr [4 x i32], ptr %a, i32 0, i32 %i
  %v = add i32 %n, %i
  store i32 %v, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, 4
  br i1 %c, label %l, label %s
s:
  %q = getelementptr [4 x i32], ptr %a, i32 0, i32 3
  %r = load i32, ptr %q
  ret i32 %r
}";
    let module = llrm_mir::parse::module(ir).expect("parses");
    let out = text_with(&module, "f", Cpu::M68000, Options::default()).expect("compiles");
    let indexed_lea = |line: &&str| line.starts_with("\tlea\t(0,%a") && line.contains(", %d");
    assert!(!out.lines().any(|line| indexed_lea(&line)), "the stored sum is an address:\n{out}");
}

/// A pointer stepped in a loop and returned (`while (*p != 58 && *p) p += 3;
/// return p;`) went through a data register on every step (`move.l %a0, %d0;
/// addq.l #3, %d0; movea.l %d0, %a0`, 6 bytes): the return, a copy into the
/// fixed address register, counted as a data use. It is `addq.l #3, %a0`.
fn walk_and_return() -> &'static str {
    "define ptr @f(ptr %p) optsize {
entry:
  br label %l
l:
  %q = phi ptr [ %p, %entry ], [ %q1, %n ]
  %c = load i8, ptr %q
  %z = icmp eq i8 %c, 0
  br i1 %z, label %x, label %t
t:
  %e = icmp eq i8 %c, 58
  br i1 %e, label %x, label %n
n:
  %q1 = getelementptr i8, ptr %q, i32 3
  br label %l
x:
  ret ptr %q
}"
}

/// How many register-to-register copies between a data and an address register
/// the text holds.
fn class_crossings(text: &str) -> usize {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            let Some((op, args)) = line.split_once('\t') else { return false };
            matches!(op, "move.l" | "movea.l")
                && args.split_once(", ").is_some_and(|(from, to)| {
                    (from.starts_with("%a") && to.starts_with("%d")) || (from.starts_with("%d") && to.starts_with("%a"))
                })
        })
        .count()
}

#[test]
fn a_returned_pointer_step_stays_in_the_address_register_at_os() {
    let options = Options { size: true, ..Options::default() };
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(walk_and_return()), "f", cpu, options).expect("compiles");
        assert_eq!(class_crossings(&out), 0, "{cpu:?}: the step crosses classes:\n{out}");
        assert!(out.contains("addq.l\t#3, %a"), "{cpu:?}: the step is addq on an address register:\n{out}");
    }
}

/// `p = *at; while (t[*++p] != 255); *at = p;`: the step's input is read by the
/// add and by the byte load that folded the `+ 1` (`move.b (1,%aN)`), so the
/// pass that puts a pointer add in the address register (it wanted the copy
/// read by the add alone) left it in a data register: `move.l %a1, %d0;
/// addq.l #1, %d0; movea.l %d1, %a2` round the loop, 2 copies and a register
/// per trip (QB `kw_scan`).
const SCAN: &str = "define void @f(ptr %at, ptr %t) {
entry:
  %p0 = load ptr, ptr %at
  br label %l
l:
  %q = phi ptr [ %p0, %entry ], [ %q1, %l ]
  %q1 = getelementptr i8, ptr %q, i32 1
  %b = load i8, ptr %q1
  %z = zext i8 %b to i32
  %e = getelementptr i8, ptr %t, i32 %z
  %k = load i8, ptr %e
  %c = icmp eq i8 %k, -1
  br i1 %c, label %x, label %l
x:
  store ptr %q1, ptr %at
  ret void
}";

#[test]
fn a_pointer_step_read_by_a_displaced_load_too_stays_in_the_address_register() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(SCAN), "f", cpu, Options { size: true, ..Options::default() }).expect("compiles");
        assert!(out.contains("addq.l\t#1, %a"), "{cpu:?}:\n{out}");
        let (_, body) = out.split_once(".LBB0_1:").expect("a loop");
        let body = body.split_once("\tbne\t.LBB0_1").expect("its back edge").0;
        let crossings = body
            .lines()
            .filter(|line| {
                let line = line.trim();
                (line.starts_with("move.l\t%a") && line.contains(", %d")) || line.starts_with("movea.l\t%d")
            })
            .count();
        assert_eq!(crossings, 0, "{cpu:?}: copies between address and data registers in the loop:\n{out}");
        assert!(body.contains("(%a"), "{cpu:?}: the byte load reads the stepped pointer:\n{out}");
    }
}

/// A pointer read at one place and stepped by 1 or 2 at others: every step
/// read the pointer through a copy into the others' data-class inputs, so none
/// was made an address add and each paid `move; addq; move` through a data
/// register (QB `decl_step`: 136 B, gcc 112; declare.c -146 B at 68020).
#[test]
fn the_steps_of_one_pointer_are_address_adds() {
    let ir = "define ptr @f(ptr %p) minsize {
entry:
  br label %l
l:
  %q = phi ptr [ %p, %entry ], [ %q1, %one ], [ %q2, %two ]
  %c = load i8, ptr %q
  switch i8 %c, label %x [ i8 46, label %one
                           i8 33, label %two ]
one:
  %q1 = getelementptr i8, ptr %q, i32 1
  br label %l
two:
  %q2 = getelementptr i8, ptr %q, i32 2
  br label %l
x:
  ret ptr %q
}";
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let asm = text(ir, cpu);
        let in_data = asm.lines().filter(|line| line.starts_with("\taddq.l\t#") && line.contains(", %d")).count();
        assert_eq!(in_data, 0, "{cpu:?}: a pointer step in a data register:\n{asm}");
    }
}

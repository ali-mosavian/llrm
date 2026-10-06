use llrm_analysis::testing::corpus;
use llrm_mir::interpret;
use llrm_mir::program::Program;

use crate::pipeline::{self, Applied};

// Enough for every corpus entry that finishes at all.
const FUEL: u64 = 2_000_000;

#[test]
fn the_pipeline_keeps_every_corpus_module_verifying_and_computing_the_same() {
    let applied = Applied::default();
    let mut ran = 0;
    for (name, mut module) in corpus() {
        let entry = module.named("main").filter(|&id| module.global(id).function().is_some_and(|one| !one.is_declaration() && one.parameters().is_empty()));
        let before = entry.map(|_| interpret::run(&module, "main", Vec::new(), FUEL));
        // @main is run below, so it is the program's entry, internal or not.
        let entered = |program: &mut Program| {
            program.exports.entries.insert("main".to_owned());
            pipeline::applied(program, &applied)
        };
        Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), entered)
            .and_then(|done| done)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        if let Some(Ok(before)) = before {
            assert_eq!(interpret::run(&module, "main", Vec::new(), FUEL), Ok(before), "{name}");
            ran += 1;
        }
    }
    assert!(ran > 0, "no corpus entry runs");
}

/// Commutes @f's add so that its constant comes first or last.
struct Commute {
    name: &'static str,
    constant_first: bool,
}

impl llrm_mir::passes::FunctionPass for Commute {
    fn name(&self) -> &'static str {
        self.name
    }

    fn run(&mut self, unit: &mut llrm_mir::passes::Unit, _: &mut llrm_mir::passes::Analyses) -> llrm_mir::passes::PreservedAnalyses {
        use llrm_mir::module::Operand;
        let (_, add) = unit.function.walk().find(|&(_, one)| unit.function.instruction(one).opcode == llrm_mir::opcode::Opcode::Binary(llrm_mir::opcode::BinaryOp::Add)).expect("an add");
        let operands = unit.function.instruction(add).operands.clone();
        if matches!(operands[0], Operand::Constant(_)) == self.constant_first {
            return llrm_mir::passes::PreservedAnalyses::all();
        }
        unit.function.set_operands(add, vec![operands[1], operands[0]]);
        llrm_mir::passes::PreservedAnalyses::none()
    }
}

/// Arenas never shrink, so before bodies were compared by their text two
/// passes undoing each other ran the whole size-scaled limit.
#[test]
#[should_panic(expected = "cycle after 1 rounds")]
fn passes_that_undo_each_other_stop_after_one_cycle() {
    let mut module = llrm_analysis::testing::parsed("define i16 @f(i16 %x) {\nb0:\n  %y = add i16 %x, 1\n  ret i16 %y\n}\n");
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes = vec![Box::new(Commute { name: "first", constant_first: true }), Box::new(Commute { name: "last", constant_first: false })];
    crate::testing::managed(&mut module, fixed);
}

/// A loop counted in an internal global no code outside names, a
/// `nocallback` routine called each trip: the count is proven and the
/// loop copied out. GlobalsAA was never required, so every call wrote the
/// counter and FPDEEP's PRINT loop stayed rolled.
#[test]
fn a_call_keeps_no_global_it_cannot_name() {
    let mut module = llrm_analysis::testing::parsed(
        "@i = internal global i16 0

declare void @print(i16) nocallback

define void @main() {
b0:
  store i16 1, ptr @i
  br label %b1

b1:
  %v = load i16, ptr @i
  %go = icmp sle i16 %v, 3
  br i1 %go, label %b2, label %b3

b2:
  call void @print(i16 %v)
  %w = load i16, ptr @i
  %n = add i16 %w, 1
  store i16 %n, ptr @i
  br label %b1, !llvm.loop !0

b3:
  ret void
}

!0 = distinct !{!0, !1}
!1 = !{!\"llvm.loop.unroll.full\"}
",
    );
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    let text = llrm_mir::print::module(&module);
    for trip in 1..=3 {
        assert!(text.contains(&format!("call void @print(i16 {trip})")), "{text}");
    }
}

/// FPDEEP built by PDS or VB kept its `FOR i = 1 TO 3` rolled, three PRINTs and their
/// float chains computed at run time: the raise leaves a dead `ptrtoint` of the global
/// holding `i`, GlobalsAA took it as an escape, and the dead code went only after the
/// call-clobbered reload of `i` had been priced; a second pipeline run folded all of it.
#[test]
fn a_global_whose_only_escape_is_dead_code_is_tracked_after_the_cleanup() {
    let mut module = llrm_analysis::testing::parsed(
        "@i = internal global i16 0

declare void @print(i16) nocallback

define void @main() {
b0:
  %p = ptrtoint ptr @i to i16
  %q = add i16 %p, 2
  br label %b1

b1:
  %v = phi i16 [ 1, %b0 ], [ %n, %b2 ]
  store i16 %v, ptr @i
  %go = icmp sgt i16 %v, 3
  br i1 %go, label %b3, label %b2

b2:
  call void @print(i16 %v)
  %w = load i16, ptr @i
  %n = add i16 %w, 1
  br label %b1, !llvm.loop !0

b3:
  ret void
}

!0 = distinct !{!0, !1}
!1 = !{!\"llvm.loop.unroll.full\"}
",
    );
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    let text = llrm_mir::print::module(&module);
    for trip in 1..=3 {
        assert!(text.contains(&format!("call void @print(i16 {trip})")), "{text}");
    }
}

/// Zero stores merged into a memset mid-pipeline left a call in the loop
/// that loop motion could not see past: nbodys' `accX = 0: accY = 0` kept
/// posY's loads in its inner loop, 1.9% more instructions. The merge waits
/// for the scalar passes.
#[test]
fn zeros_stored_in_a_loop_leave_its_invariant_loads_hoisted() {
    let mut module = llrm_analysis::testing::parsed(&format!(
        "{}@acc = internal global [2 x i16] zeroinitializer
@g = internal global [4 x i16] zeroinitializer

declare void @print(i16) nocallback

define void @main(i16 %n, i16 %m) {{
b0:
  %next = getelementptr i16, ptr @acc, i16 1
  %w = getelementptr i16, ptr @g, i16 %n
  store i16 %n, ptr %w
  %r = getelementptr i16, ptr @g, i16 %m
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  store i16 0, ptr @acc
  store i16 0, ptr %next
  %v = load i16, ptr %r
  call void @print(i16 %v)
  %j = add i16 %i, 1
  br label %b1

b3:
  ret void
}}
",
        llrm_analysis::testing::DOS
    ));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    let text = llrm_mir::print::module(&module);
    let body = &text[text.find("b2:").expect("the loop")..];
    let body = &body[..body.find("br i1").expect("its latch")];
    assert!(!body.contains("load"), "{text}");
}

/// matmul8's init: an inner loop branching on `i == j` is peeled, then its
/// outer loop, which decides every branch. Peel ran once per body, so the
/// outer loop stayed rolled around eight undecided diamonds.
#[test]
fn a_loop_whose_inner_loop_was_peeled_is_peeled_in_turn() {
    let text = "define i16 @main() {
b0:
  %a = alloca [64 x i16]
  br label %outer

outer:
  %i = phi i16 [ 0, %b0 ], [ %in, %outerlatch ]
  %go = icmp slt i16 %i, 8
  br i1 %go, label %inner, label %done

inner:
  %j = phi i16 [ 0, %outer ], [ %jn, %latch ]
  %more = icmp slt i16 %j, 8
  br i1 %more, label %body, label %outerlatch

body:
  %row = shl i16 %i, 3
  %at = add i16 %row, %j
  %slot = getelementptr inbounds i16, ptr %a, i16 %at
  %same = icmp eq i16 %i, %j
  br i1 %same, label %diagonal, label %other

diagonal:
  store i16 2, ptr %slot
  br label %latch

other:
  %sum = add i16 %i, %j
  %r = srem i16 %sum, 3
  store i16 %r, ptr %slot
  br label %latch

latch:
  %jn = add i16 %j, 1
  br label %inner

outerlatch:
  %in = add i16 %i, 1
  br label %outer

done:
  %p = getelementptr inbounds i16, ptr %a, i16 13
  %v = load i16, ptr %p
  %q = getelementptr inbounds i16, ptr %a, i16 18
  %w = load i16, ptr %q
  %t = add i16 %v, %w
  ret i16 %t
}
";
    let mut module = llrm_analysis::testing::parsed(text);
    let before = interpret::run(&module, "main", Vec::new(), FUEL);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    let after = llrm_mir::print::module(&module);
    assert_eq!(interpret::run(&module, "main", Vec::new(), FUEL), before, "{after}");
    assert!(!after.contains(" phi "), "{after}");
}

/// A private static a loop stores between reads of a far array goes out of
/// the loop: sum_three stored it after every element.
#[test]
fn a_static_stored_between_far_reads_leaves_the_loop() {
    let text = format!(
        "{}@t = internal global [2 x i8] zeroinitializer

define i16 @f(ptr %desc, i16 %n) {{
b0:
  store i16 0, ptr @t, !tbaa !2
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %acc = phi i16 [ 0, %b0 ], [ %s2, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %segat = getelementptr i8, ptr %desc, i16 2
  %seg = load i16, ptr %segat
  %offat = getelementptr i8, ptr %desc, i16 10
  %off = load i16, ptr %offat
  %twice = mul i16 %i, 2
  %at = add i16 %off, %twice
  %near = inttoptr i16 %seg to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %near to ptr addrspace(1)
  %e = getelementptr i8, ptr addrspace(1) %far, i16 %at
  %v = load i16, ptr addrspace(1) %e
  %s1 = add i16 %acc, %v
  store i16 %s1, ptr @t, !tbaa !2
  %v2 = load i16, ptr addrspace(1) %e
  %s2 = add i16 %s1, %v2
  store i16 %s2, ptr @t, !tbaa !2
  %inext = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}}

!0 = !{{!\"llrm hir\"}}
!1 = !{{!\"place\", !0, i64 0}}
!2 = !{{!1, !1, i64 0}}
",
        llrm_analysis::testing::DOS
    );
    let mut module = crate::testing::parsed(&text);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    let after = crate::testing::printed(&module);
    let body = after.split("b2:").nth(1).unwrap_or("");
    assert!(!body.split("\n\n").next().unwrap_or("").contains("store i16"), "{after}");
}

/// Inline assembly has effects: of the same block called twice in a loop, with
/// its result unused or the same input, none is merged, hoisted or deleted.
#[test]
fn the_pipeline_keeps_every_inline_assembly_block_where_it_is() {
    let block = "llrm.ia16.asm.cd1a.ax.cx.flags.n";
    let mut module = llrm_analysis::testing::parsed(&format!(
        "{}declare i16 @{block}(i16)
declare void @llrm.ia16.asm.fa.-.-.-.n()

define i16 @main(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  call void @llrm.ia16.asm.fa.-.-.-.n()
  call void @llrm.ia16.asm.fa.-.-.-.n()
  %t = call i16 @{block}(i16 0)
  %u = call i16 @{block}(i16 0)
  %j = add i16 %i, 1
  br label %b1

b3:
  ret i16 0
}}
",
        llrm_analysis::testing::DOS
    ));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    let text = llrm_mir::print::module(&module);
    let body = &text[text.find("b2:").expect("the loop")..];
    let body = &body[..body.find("br label").expect("its latch")];
    assert_eq!(body.matches("call void @llrm.ia16.asm.fa").count(), 2, "{text}");
    assert_eq!(body.matches(&format!("@{block}(")).count(), 2, "{text}");
}

/// A wait on the BIOS tick (`DEF SEG = &H40: DO: LOOP UNTIL PEEK(&H6C) <> t`)
/// compiled to `cmp ax, ax` and spun: the second read was the first. The
/// frontend states `volatile` of such a read, and it stays in the loop
/// whatever else the passes know of memory at a fixed address.
#[test]
fn a_wait_on_a_device_read_keeps_the_read_in_the_loop() {
    let text = format!(
        "{}define i16 @f() {{
b0:
  %s = inttoptr i16 64 to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %dev = addrspacecast ptr addrspace(1) %far to ptr addrspace(4)
  %tick = getelementptr i8, ptr addrspace(4) %dev, i16 108
  %t = load volatile i16, ptr addrspace(4) %tick
  br label %b1

b1:
  %v = load volatile i16, ptr addrspace(4) %tick
  %same = icmp eq i16 %v, %t
  br i1 %same, label %b1, label %b2

b2:
  ret i16 %v
}}
",
        llrm_analysis::testing::DOS
    );
    let mut module = crate::testing::parsed(&text);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    let after = crate::testing::printed(&module);
    let body = after.split("b1:").nth(1).unwrap_or("").split("\n\n").next().unwrap_or("");
    assert!(body.contains("load volatile i16, ptr addrspace(4)"), "{after}");
}

/// `int a = *p; *q = 1; return a + *p` with `q` a `char *`: C lets the char
/// store write `*p`, so the second load stays. TypeBasedAA's `omnipotent char`
/// was not read as the parent of `int2`, and llrm-c's -O2 added `*p` to itself.
#[test]
fn a_char_store_between_two_int_loads_leaves_the_second_load() {
    let text = "define i16 @f(ptr %p, ptr %q) {
b0:
  %a = load i16, ptr %p, !tbaa !5
  store i8 1, ptr %q, !tbaa !4
  %b = load i16, ptr %p, !tbaa !5
  %s = add i16 %a, %b
  ret i16 %s
}

!0 = !{!\"Simple C/C++ TBAA\"}
!1 = !{!\"omnipotent char\", !0, i64 0}
!2 = !{!\"int2\", !1, i64 0}
!4 = !{!1, !1, i64 0}
!5 = !{!2, !2, i64 0}
";
    assert!(text.contains("store i8 1, ptr %q"), "the shape that was forwarded over");
    let mut module = llrm_analysis::testing::parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    let printed = llrm_mir::print::module(&module);
    assert_eq!(printed.matches("load i16").count(), 2, "{printed}");
}

/// A loop of calls to a function that touches no memory is peeled, but each body's pipeline ran its
/// peel before anything stated what its callees do, so the call looked like it may write and a
/// copy that grows was refused (nib's nbody_fixed read 3,457,356 instructions against 3,116,356).
#[test]
fn a_loop_calling_a_pure_function_is_judged_on_what_the_function_does() {
    let mut module = llrm_analysis::testing::parsed(
        "define internal i16 @g(i16 %x) {
b0:
  %y = mul i16 %x, 7
  ret i16 %y
}

define i16 @f(i16 %p) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %p, %b0 ], [ %sum, %b2 ]
  %go = icmp slt i16 %i, 6
  br i1 %go, label %b2, label %b3

b2:
  %r = call i16 @g(i16 %i)
  %sum = add i16 %acc, %r
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}
",
    );
    let applied = Applied { options: pipeline::Options { inline: crate::inline::Threshold::new(0), ..pipeline::Options::default() }, ..Applied::default() };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    assert!(!text.contains("phi i16 [ 0,"), "the loop of calls to @g stayed rolled:\n{text}");
}

/// QCport -O2 ran 11 KB past BCC's code, and out of memory loading a level: every call a body of nine
/// operations was cheaper than in clocks was copied, though three copies come to more bytes than three
/// calls. -O2 puts such a copy back as -Os does, unless the clocks it saves pay for the bytes.
#[test]
fn o2_does_not_copy_a_body_into_three_sites_where_the_code_grows() {
    let mut module = llrm_analysis::testing::parsed(
        "define i16 @mix(i16 %a, i16 %b) {
b:
  %t0 = xor i16 %a, %b
  %t1 = shl i16 %a, 3
  %t2 = add i16 %t0, %t1
  %t3 = lshr i16 %b, 2
  %t4 = sub i16 %t2, %t3
  %t5 = and i16 %t4, 2047
  %t6 = or i16 %t5, %a
  %t7 = xor i16 %t6, %b
  %t8 = add i16 %t7, 5
  ret i16 %t8
}

define i16 @f(i16 %x, i16 %y) {
b:
  %p = call i16 @mix(i16 %x, i16 %y)
  %q = call i16 @mix(i16 %y, i16 %x)
  %r = call i16 @mix(i16 %p, i16 %q)
  ret i16 %r
}
",
    );
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| pipeline::applied(program, &Applied::default())).and_then(|done| done).unwrap();
    assert_eq!(llrm_mir::print::module(&module).matches("call i16 @mix").count(), 3);
}

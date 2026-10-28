use llrm_analysis::testing::m16_ir;
use llrm_analysis::testing::{corpus_files, parse_corpus_entry};
use llrm_mir::interpret;
use llrm_mir::module::Module;
use llrm_mir::program::Program;

use crate::pipeline::{self, Applied};

// Enough for every corpus entry that finishes at all.
const FUEL: u64 = 2_000_000;

/// The entries are independent, so they run on threads, largest first (one
/// entry takes longer than all the others together). Run one after another they
/// were a single 95 s test, the longest of the library tests by 20x, and set
/// the gate's lib step.
#[test]
fn the_pipeline_keeps_every_corpus_module_verifying_and_computing_the_same() {
    let mut files = corpus_files();
    files.sort_by_key(|(_, path)| std::cmp::Reverse(std::fs::metadata(path).map(|one| one.len()).unwrap_or(0)));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let ran = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    while let Some((name, path)) = files.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) {
                        if keeps_verifying_and_computing_the_same(name, parse_corpus_entry(name, path)) {
                            ran.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                })
            })
            .collect();
        for worker in workers {
            if let Err(panic) = worker.join() {
                std::panic::resume_unwind(panic);
            }
        }
    });
    assert!(ran.into_inner() > 0, "no corpus entry runs");
}

/// Whether the entry ran to a result that survived the pipeline unchanged.
fn keeps_verifying_and_computing_the_same(
    name: &str,
    mut module: Module,
) -> bool {
    let applied = Applied::default();
    let entry = module
        .named("main")
        .filter(
            |&id| module.global(id).function().is_some_and(|one| !one.is_declaration() && one.parameters().is_empty()),
        );
    let before = entry.map(|_| interpret::run(&module, "main", Vec::new(), FUEL));
    // @main is run below, so it is the program's entry, internal or not.
    let entered = |program: &mut Program| {
        program.exports.entries.insert("main".to_owned());
        pipeline::applied(program, &applied)
    };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), entered)
        .and_then(|done| done)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    let Some(Ok(before)) = before else { return false };
    assert_eq!(interpret::run(&module, "main", Vec::new(), FUEL), Ok(before), "{name}");
    true
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

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        _: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        use llrm_mir::module::Operand;
        let (_, add) = unit
            .function
            .walk()
            .find(|&(_, one)| {
                unit.function.instruction(one).opcode
                    == llrm_mir::opcode::Opcode::Binary(llrm_mir::opcode::BinaryOp::Add)
            })
            .expect("an add");
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
    let mut module =
        llrm_analysis::testing::parsed("define i16 @f(i16 %x) {\nb0:\n  %y = add i16 %x, 1\n  ret i16 %y\n}\n");
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes = vec![
        Box::new(Commute { name: "first", constant_first: true }),
        Box::new(Commute { name: "last", constant_first: false }),
    ];
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    for trip in 1..=3 {
        assert!(text.contains(&format!("call void @print(i16 {trip})")), "{text}");
    }
}

/// FPDEEP built by PDS or VB kept its `FOR i = 1 TO 3` rolled, three PRINTs and
/// their float chains computed at run time: the raise leaves a dead `ptrtoint`
/// of the global holding `i`, GlobalsAA took it as an escape, and the dead code
/// went only after the call-clobbered reload of `i` had been priced; a second
/// pipeline run folded all of it.
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    for trip in 1..=3 {
        assert!(text.contains(&format!("call void @print(i16 {trip})")), "{text}");
    }
}

/// qb stride.bas at -O2: a loop whose only effect was stores to a global
/// nothing reads. The module step removed the stores and left the counting
/// loop (`add ax, 5; cmp ax, 95; jbe`); no loop deletion followed it.
#[test]
fn a_loop_whose_stores_to_a_never_read_global_went_is_deleted() {
    let mut module = llrm_analysis::testing::parsed(&format!(
        "{}@b = internal global [402 x i8] zeroinitializer

declare void @print(i16) nocallback

define void @main() {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b1 ]
  %q = udiv i16 %i, 5
  %p = getelementptr i8, ptr @b, i16 %i
  store i16 %q, ptr %p
  %n = add nuw nsw i16 %i, 5
  %go = icmp ugt i16 %i, 95
  br i1 %go, label %b2, label %b1

b2:
  call void @print(i16 210)
  ret void
}}
",
        llrm_analysis::testing::DOS
    ));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("main".to_owned());
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    assert!(!text.contains("phi"), "{text}");
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    let body = &text[text.find("b2:").expect("the loop")..];
    let body = &body[..body.find("br i1").expect("its latch")];
    assert!(!body.contains("load"), "{text}");
}

/// A declaration a pass makes is seen by the passes after it in the same run,
/// as LLVM's `getOrInsertFunction` puts it in the module at once. Loop idiom
/// declared `llvm.memset` (the module had none), and the merge after it did not
/// know the callee for a memset: lru's two 14-byte fills stayed two calls,
/// 52 clocks and 61 bytes at -Os.
#[test]
fn two_fills_made_into_memsets_in_a_module_with_none_are_one_memset() {
    let mut module = llrm_analysis::testing::parsed(&format!(
        "{}@c = global [14 x i16] zeroinitializer

define void @main() {{
b0:
  %second = getelementptr i16, ptr @c, i16 7
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  %p = getelementptr i16, ptr @c, i16 %i
  store i16 -1, ptr %p
  %j = add i16 %i, 1
  %more = icmp ult i16 %j, 7
  br i1 %more, label %b1, label %b2

b2:
  br label %b3

b3:
  %k = phi i16 [ 0, %b2 ], [ %l, %b3 ]
  %q = getelementptr i16, ptr %second, i16 %k
  store i16 -1, ptr %q
  %l = add i16 %k, 1
  %again = icmp ult i16 %l, 7
  br i1 %again, label %b3, label %b4

b4:
  ret void
}}
",
        llrm_analysis::testing::DOS
    ));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("main".to_owned());
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    let calls: Vec<&str> = text.lines().filter(|line| line.contains("call void @llvm.memset")).collect();
    assert!(calls.len() == 1 && calls[0].contains("i16 28"), "{text}");
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    let body = &text[text.find("b2:").expect("the loop")..];
    let body = &body[..body.find("\n\n").unwrap_or(body.len())];
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let printed = llrm_mir::print::module(&module);
    assert_eq!(printed.matches("load i16").count(), 2, "{printed}");
}

/// A loop of calls to a function that touches no memory is peeled, but each
/// body's pipeline ran its peel before anything stated what its callees do, so
/// the call looked like it may write and a copy that grows was refused (nib's
/// nbody_fixed read 3,457,356 instructions against 3,116,356).
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
    let applied = Applied {
        options: pipeline::Options { inline: crate::inline::Threshold::none(), ..pipeline::Options::default() },
        ..Applied::default()
    };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    assert!(!text.contains("phi i16 [ 0,"), "the loop of calls to @g stayed rolled:\n{text}");
}

/// QCport -O2 ran 11 KB past BCC's code, and out of memory loading a level:
/// every call a body of nine operations was cheaper than in clocks was copied,
/// though three copies come to more bytes than three calls. -O2 puts such a
/// copy back as -Os does, unless the clocks it saves pay for the bytes.
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
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        // The clocks' budget; the walk decides as LLVM does, and LLVM inlines
        // all three.
        pipeline::applied(
            program,
            &Applied { options: pipeline::Options { llvm_order: false, ..Default::default() }, ..Default::default() },
        )
    })
    .and_then(|done| done)
    .unwrap();
    assert_eq!(llrm_mir::print::module(&module).matches("call i16 @mix").count(), 3);
}

/// The pin's IPSCCP states what a function's callers pass as a range on its
/// parameters at -O1 too. The stand-in it replaced (gcc's `-fipa-vrp`) ran at
/// -O2 and up, and -O1 stated nothing.
#[test]
fn test_o1_states_the_ranges_from_the_callers_arguments_as_o2_does() {
    let text = "define internal i16 @pick(i16 %x) {
b:
  %small = icmp ult i16 %x, 10
  %r = select i1 %small, i16 %x, i16 7
  ret i16 %r
}

define i16 @main() {
b:
  %a = call i16 @pick(i16 3)
  %c = call i16 @pick(i16 4)
  %s = add i16 %a, %c
  ret i16 %s
}
";
    let ranged = |options: pipeline::Options| {
        let mut module = crate::testing::parsed(text);
        let options = pipeline::Options { inline: crate::inline::Threshold::none(), ..options };
        let applied = Applied { options, ..Applied::default() };
        let entered = |program: &mut Program| {
            program.exports.entries.insert("main".to_owned());
            pipeline::applied(program, &applied)
        };
        Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), entered)
            .and_then(|done| done)
            .unwrap();
        crate::testing::printed(&module).contains("range(")
    };
    assert!(ranged(pipeline::Options::standard()), "premise: -O2 states the range");
    assert!(ranged(pipeline::Options::basic()), "-O1 stated no range from the callers' arguments");
}

/// `decide` and `ports` asked the counted loops' intervals (`Bounded`,
/// llrm-only) at -O2: about 7 M of d_faces' 1176 M. The pin's passes ask
/// ValueTracking, and no level works the intervals out.
#[test]
fn no_level_works_out_the_counted_loops_intervals() {
    let text = "
@a = global [16 x i16] zeroinitializer

define i16 @f(i16 %n) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %slot = getelementptr [16 x i16], ptr @a, i16 0, i16 %i
  store i16 %i, ptr %slot
  %next = add i16 %i, 1
  %more = icmp ult i16 %next, 16
  br i1 %more, label %loop, label %done
done:
  %last = load i16, ptr @a
  ret i16 %last
}
";
    let asked = |options: pipeline::Options| {
        let mut module = crate::testing::parsed(text);
        let options = pipeline::Options { inline: crate::inline::Threshold::none(), ..options };
        let applied = Applied { options, ..Applied::default() };
        llrm_mir::passes::trace_recomputes(true);
        let entered = |program: &mut Program| {
            program.exports.entries.insert("f".to_owned());
            pipeline::applied(program, &applied)
        };
        Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), entered)
            .and_then(|done| done)
            .unwrap();
        let runs = llrm_mir::passes::recomputes();
        llrm_mir::passes::trace_recomputes(false);
        runs.iter().filter(|(name, ..)| *name == "bounded").map(|(.., count)| count).sum::<usize>()
    };
    for options in [pipeline::Options::basic(), pipeline::Options::standard()] {
        assert_eq!(asked(options), 0, "the counted loops' intervals were worked out");
    }
}

const SUMMARISED: &str = "
define i16 @count(i16 %n) {
b1:
  %i = alloca i16
  %sum = alloca i16
  store i16 0, ptr %sum
  store i16 0, ptr %i
  br label %b2
b2:
  %a = load i16, ptr %i
  %c = icmp ult i16 %a, %n
  br i1 %c, label %b3, label %b4
b3:
  %s = load i16, ptr %sum
  %j = load i16, ptr %i
  %t = add i16 %s, %j
  store i16 %t, ptr %sum
  %k = add i16 %j, 1
  store i16 %k, ptr %i
  br label %b2
b4:
  %r = load i16, ptr %sum
  ret i16 %r
}

define i16 @main() {
entry:
  %r = call i16 @count(i16 100)
  ret i16 %r
}
";

/// `text` through the pipeline under `options`, with how many times each module
/// analysis was worked out, by how it came out (`first`: nothing held).
fn pipelined(
    text: &str,
    options: pipeline::Options,
) -> (Module, Vec<(&'static str, &'static str, usize)>) {
    let mut module = crate::testing::parsed(text);
    let applied = Applied { options, ..Applied::default() };
    llrm_mir::passes::trace_recomputes(true);
    let entered = |program: &mut Program| {
        program.exports.entries.insert("main".to_owned());
        pipeline::applied(program, &applied)
    };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), entered).and_then(|done| done).unwrap();
    let runs = llrm_mir::passes::recomputes();
    llrm_mir::passes::trace_recomputes(false);
    let mut by_how: Vec<(&'static str, &'static str, usize)> = Vec::new();
    for (name, _, how, count) in runs {
        match by_how.iter_mut().find(|(one, other, _)| *one == name && *other == how) {
            Some(found) => found.2 += count,
            None => by_how.push((name, how, count)),
        }
    }
    (module, by_how)
}

/// The summaries and the globals' facts are worked out from nothing once for a
/// program, not once for each segment of the pipeline and each module pass that
/// makes its own analyses: the ones after the first start from what the
/// last left (`Carried`). bench/sieve at -O1 worked the summaries out from
/// nothing 5 times.
#[test]
fn the_summaries_are_worked_out_from_nothing_once() {
    assert!(
        std::env::var_os("LLRM_CHECK_STALE").is_none(),
        "counts recomputes: LLRM_CHECK_STALE recomputes held analyses and adds to them"
    );
    let (_, runs) = pipelined(SUMMARISED, pipeline::Options::basic());
    let first =
        |name: &str| runs.iter().find(|(one, how, _)| *one == name && *how == "first").map_or(0, |found| found.2);
    let counted = |name: &str| runs.iter().filter(|(one, ..)| *one == name).collect::<Vec<_>>();
    assert_eq!(
        (first("summaries"), first("globals-aa")),
        (1, 1),
        "from nothing again after the first run: {:?} {:?}",
        counted("summaries"),
        counted("globals-aa")
    );
}

/// The summaries carried to the segment after `GlobalDce` are of bodies whose
/// ids it has moved: it removed `@dead`, which came before `@bump` and `@main`.
/// A summary kept by id and not by body would give `@bump` the summary `@dead`
/// had (no memory touched) and let `@main` return the 1 it stored before the
/// call. Each body's `Mark` names its function, so the bodies that moved are
/// worked out again. A guard: it passes without the carry too.
#[test]
fn summaries_carried_past_a_removed_global_stay_with_their_bodies() {
    let text = "
@g = global i16 0

define internal i16 @dead() {
entry:
  ret i16 7
}

define void @bump() {
entry:
  store i16 5, ptr @g
  ret void
}

define i16 @main() {
entry:
  store i16 1, ptr @g
  call void @bump()
  %v = load i16, ptr @g
  ret i16 %v
}
";
    let options = pipeline::Options { inline: crate::inline::Threshold::none(), ..pipeline::Options::basic() };
    let (module, runs) = pipelined(text, options);
    let printed = crate::testing::printed(&module);
    let first = runs.iter().find(|(name, how, _)| *name == "summaries" && *how == "first").map_or(0, |found| found.2);
    assert_eq!(first, 1, "premise: the summaries were carried from segment to segment: {runs:?}");
    assert!(!printed.contains("@dead"), "premise: the global that came first is removed:\n{printed}");
    assert!(printed.contains("call void @bump()"), "premise: the call stays:\n{printed}");
    assert!(printed.contains("load i16, ptr @g"), "the load after the call was forwarded from before it:\n{printed}");
}

/// gcc's `pass_ch` stage ran after the interprocedural step under LLVM's order
/// too, 8 passes the pin has no counterpart for: d_faces compile +52 M
/// instructions at -O1, +41 M at -O2, +72 M at -O3.
#[test]
fn test_the_llvm_order_has_no_gcc_header_copy_stage() {
    let on = Applied { options: pipeline::Options { llvm_order: true, ..Default::default() }, ..Applied::default() };
    let off = Applied { options: pipeline::Options { llvm_order: false, ..Default::default() }, ..Applied::default() };
    assert!(pipeline::header_copied(&on).is_empty(), "premise: LLVM's order copies headers in LoopRotate");
    assert!(!pipeline::header_copied(&off).is_empty(), "premise: gcc's order has the stage");
}

/// What the pipeline runs after the summaries are made and holds them past
/// (`manager.freeze`) adds no memory operation. Six of those passes
/// (loop-simplify, LCSSA, fold, algebraic, indvars, dead) did not say so, and
/// each edit of theirs made the module's summaries and globals' facts again.
#[test]
fn the_passes_after_the_summaries_add_no_memory_operation() {
    let applied = Applied {
        options: pipeline::Options { llvm_order: false, ..pipeline::Options::basic() },
        ..Applied::default()
    };
    let passes = pipeline::header_copied(&applied);
    assert!(passes.len() >= 7, "premise: the pass list this test names");
    let adding: Vec<_> = passes.iter().filter(|pass| pass.adds_memory_operations()).map(|pass| pass.name()).collect();
    assert!(adding.is_empty(), "held past, so they must not add one: {adding:?}");
}

thread_local! {
    static BURNED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

thread_local! {
    /// How long `Costly` burns, in loop trips.
    static BURN: std::cell::Cell<u64> = const { std::cell::Cell::new(2_000_000) };
}

/// The work of `trips` of `Costly`'s loop in the unit the clock reads
/// (instructions, or CPU nanoseconds where there is no counter).
fn burnt(trips: u64) -> u64 {
    let start = llrm_support::debug::work();
    let mut sum = 0u64;
    for at in 0..trips {
        sum = std::hint::black_box(sum.wrapping_add(at));
    }
    BURNED.with(|burned| burned.set(sum));
    llrm_support::debug::work() - start
}

/// An analysis that costs a lot.
struct Costly;

impl llrm_mir::passes::Analysis for Costly {
    type Result = u64;
    const NAME: &'static str = "costly";
    fn run(
        _: &llrm_mir::context::Context,
        _: &llrm_mir::datalayout::DataLayout,
        _: &llrm_mir::module::Function,
        _: &mut llrm_mir::passes::Analyses,
    ) -> u64 {
        let mut sum = 0u64;
        for at in 0..BURN.with(std::cell::Cell::get) {
            sum = std::hint::black_box(sum.wrapping_add(at));
        }
        BURNED.with(|burned| burned.set(sum));
        sum
    }
}

/// Finds nothing to do, having asked for `Costly`.
struct Asks;

impl llrm_mir::passes::FunctionPass for Asks {
    fn name(&self) -> &'static str {
        "asks"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        analyses.get::<Costly>(unit.context, unit.layout, unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// `LLRM_DEBUG=runs` billed a pass with the analyses it computed first: 63% of
/// the pipeline's work read as idle (floatloop 5.7 G, sroa 5.0 G) when the
/// passes' own work was 3% and the analyses were reused by the next pass to
/// ask. A step now says how much of its work was its own.
fn billed_asking_for_costly(trips: u64) -> (super::Run, u64) {
    BURN.with(|burn| burn.set(trips));
    let reference = burnt(trips);
    use llrm_mir::module::{GlobalKind, Module};
    use llrm_mir::passes::{Analyses, Declared, Outer, Unit};
    llrm_mir::passes::observe(llrm_mir::passes::Observer {
        span: |kind, _, run| if kind == "analysis" { llrm_support::debug::analysed(run) } else { run() },
        function: |_, run| run(),
        count: |_, _| {},
    });
    let mut module: Module = crate::testing::parsed("define i16 @f() {\nb:\n  ret i16 0\n}\n");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let mut analyses = Analyses::new(std::rc::Rc::new(Outer::of(&module, None)));
    let mut declared = Declared::of(&module);
    let Module { context, globals, metadata, .. } = &mut module;
    let GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    let mut unit = Unit { context, layout: &layout, function, id: None, metadata, declared: &mut declared };
    let mut run = super::Run {
        version: 0,
        promotes: true,
        dump: None,
        inputs: false,
        rounds: 0,
        steps: 0,
        skipped: 0,
        fixed: 0,
        billing: true,
        idle: 0,
        useful: 0,
        idle_with_analyses: 0,
        useful_with_analyses: 0,
        classified: unit.function.mark(),
        stretches: Vec::new(),
        unreached: 0,
        slice: None,
        persisted: 0,
        records: unit.context.extension::<crate::settled::Settled>(),
        rerun: false,
        idle_at: Vec::new(),
    };
    run.step(&mut Asks, &"asks", &mut unit, &mut analyses);
    (run, reference)
}

#[test]
fn test_a_steps_own_work_leaves_out_the_analyses_it_computed() {
    let (run, reference) = billed_asking_for_costly(2_000_000);
    assert!(
        analysis_was_billed(run.idle_with_analyses, reference),
        "premise: the analysis cost {} of {reference}",
        run.idle_with_analyses
    );
    assert!(run.idle * 10 < run.idle_with_analyses, "own {} of {} with the analysis", run.idle, run.idle_with_analyses);
}

/// A fixed floor on the analysis's work (1,000,000) read "instructions" with a
/// counter and "CPU nanoseconds" without one, where a fast runner spends
/// 790,710 on 2,000,000 trips: the premise failed there though the step billed
/// the analysis. A short analysis is billed too, whatever the unit.
#[test]
fn test_the_premise_of_an_analysis_is_its_cost_in_the_clocks_unit_not_a_number_of_them() {
    let (run, reference) = billed_asking_for_costly(100_000);
    assert!(
        analysis_was_billed(run.idle_with_analyses, reference),
        "the analysis cost {} of {reference}",
        run.idle_with_analyses
    );
}

/// The step billed the analysis, read in the clock's own unit against the same
/// loop measured alone. CPU nanoseconds of one loop differ by more than 2x from
/// one read to the next on a shared runner (1,351,415 against 2,947,056,
/// #1442's CI): the floor is an eighth of the reference, which still
/// tells a billed analysis from none.
fn analysis_was_billed(
    billed: u64,
    reference: u64,
) -> bool {
    billed > 0 && billed * 8 >= reference
}

/// A noisy clock reads the analysis at 0.3 of its reference and the step still
/// counts as billed; a step that billed nothing, or a tenth, does not.
#[test]
fn test_a_noisy_clock_still_shows_the_analysis_billed_and_none_does_not() {
    assert!(analysis_was_billed(300_000, 1_000_000));
    assert!(analysis_was_billed(1_351_415, 2_947_056));
    assert!(!analysis_was_billed(0, 1_000_000));
    assert!(!analysis_was_billed(100_000, 1_000_000));
}

/// Finds nothing to do, and counts how often it was asked.
struct Watch {
    name: &'static str,
    reads: llrm_mir::footprint::Footprint,
    runs: std::rc::Rc<std::cell::Cell<usize>>,
}

impl llrm_mir::passes::FunctionPass for Watch {
    fn name(&self) -> &'static str {
        self.name
    }

    fn reads(&self) -> llrm_mir::footprint::Footprint {
        self.reads
    }

    fn run(
        &mut self,
        _: &mut llrm_mir::passes::Unit,
        _: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        self.runs.set(self.runs.get() + 1);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// Changes the constant of @f's add, once: an arithmetic value that only a
/// `ret` reads.
struct Fold {
    done: bool,
}

impl llrm_mir::passes::FunctionPass for Fold {
    fn name(&self) -> &'static str {
        "fold-once"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        _: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        if std::mem::replace(&mut self.done, true) {
            return llrm_mir::passes::PreservedAnalyses::all();
        }
        let (_, add) = unit
            .function
            .walk()
            .find(|&(_, one)| {
                unit.function.instruction(one).opcode
                    == llrm_mir::opcode::Opcode::Binary(llrm_mir::opcode::BinaryOp::Add)
            })
            .expect("an add");
        let ty = unit.function.instruction(add).ty;
        let operands = unit.function.instruction(add).operands.clone();
        let two = llrm_mir::module::Operand::Constant(unit.context.int(ty, 2));
        unit.function.set_operands(add, vec![operands[0], two]);
        llrm_mir::passes::PreservedAnalyses::none()
    }
}

/// A fold in one function made every pass before it run again, the whole round,
/// though what it changed was an arithmetic value only a `ret` reads: 13.6% of
/// x_popcount -O2's compile was passes that found nothing (decide alone 3.4 of
/// 94 Minstr), and a late fold (`& 3` over a sum masked again) cost it x1.024.
/// A pass says what it reads (`FunctionPass::reads`), the driver what the edits
/// reached (`footprint::of`): decide, which reads conditions, memory and
/// the CFG, is not run again; one that reads everything is.
#[test]
fn a_fold_does_not_rerun_the_passes_that_read_nothing_it_reached() {
    use llrm_mir::footprint::Footprint;
    let text = "define i16 @f(i16 %x) {\nb0:\n  %y = add i16 %x, 1\n  ret i16 %y\n}\n";
    let mut module = llrm_analysis::testing::parsed(text);
    let (narrow, wide) = (std::rc::Rc::new(std::cell::Cell::new(0)), std::rc::Rc::new(std::cell::Cell::new(0)));
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes = vec![
        Box::new(Watch {
            name: "narrow",
            reads: Footprint::CONTROL | Footprint::MEMORY | Footprint::STRUCTURE,
            runs: std::rc::Rc::clone(&narrow),
        }),
        Box::new(Watch { name: "wide", reads: Footprint::ALL, runs: std::rc::Rc::clone(&wide) }),
        Box::new(Fold { done: false }),
    ];
    crate::testing::managed(&mut module, fixed);
    assert_eq!(wide.get(), 2, "a pass that reads everything is run again after the fold");
    assert_eq!(narrow.get(), 1, "decide-like: the fold reached no condition, no memory and no block");
}

/// Changes the constant of @f's add on each of its first `left` runs, the way a
/// chain of folds does one step per round.
struct Chain {
    left: usize,
    runs: std::rc::Rc<std::cell::Cell<usize>>,
}

impl llrm_mir::passes::FunctionPass for Chain {
    fn name(&self) -> &'static str {
        "chain"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        _: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        self.runs.set(self.runs.get() + 1);
        if self.left == 0 {
            return llrm_mir::passes::PreservedAnalyses::all();
        }
        self.left -= 1;
        let (_, add) = unit
            .function
            .walk()
            .find(|&(_, one)| {
                unit.function.instruction(one).opcode
                    == llrm_mir::opcode::Opcode::Binary(llrm_mir::opcode::BinaryOp::Add)
            })
            .expect("an add");
        let ty = unit.function.instruction(add).ty;
        let operands = unit.function.instruction(add).operands.clone();
        let constant = llrm_mir::module::Operand::Constant(unit.context.int(ty, 10 + self.left as i128));
        unit.function.set_operands(add, vec![operands[0], constant]);
        llrm_mir::passes::PreservedAnalyses::none()
    }
}

/// The rounds of a body's passes were bounded by its size (`max(16, blocks +
/// insts + 1)`) and nothing else: the fourth and fifth round of d_faces'
/// hottest body were 354 of its 6,246 Minstr, and found nothing the 284 rows of
/// the vsgcc tables and the 65 QCport modules show (the objects are the same).
/// `Options::rounds` bounds them; -Omax runs to the fixed point.
#[test]
fn the_rounds_of_a_body_are_bounded_by_the_option_and_omax_has_none() {
    let text = "define i16 @f(i16 %x) {\nb0:\n  %y = add i16 %x, 1\n  ret i16 %y\n}\n";
    let through = |options: pipeline::Options| {
        let mut module = llrm_analysis::testing::parsed(text);
        let count = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), options, ..Applied::default() });
        fixed.only = false;
        fixed.passes = vec![Box::new(Chain { left: 8, runs: std::rc::Rc::clone(&count) })];
        crate::testing::managed(&mut module, fixed);
        count.get()
    };
    let runs = |rounds: usize| through(pipeline::Options { rounds, ..pipeline::Options::standard() });
    assert_eq!(runs(4), 4, "the budget ends the rounds");
    assert_eq!(runs(usize::MAX), 9, "premise: the chain runs to its end when nothing bounds it");
    // -O1 ran four rounds, the third of which changed five passes of d_faces
    // and the fourth none: 253 of 3,084 Minstr for the rounds after the
    // second, and 5 of the 71 vsgcc objects differ without them (+0.18%
    // clocks).
    assert_eq!(through(pipeline::Options::basic()), 2, "-O1 runs two rounds");
    assert_eq!(pipeline::Options::basic().rounds, 2);
    assert_eq!(pipeline::Options::standard().rounds, 4);
    assert_eq!(pipeline::Options::speed().rounds, 4);
    assert_eq!(pipeline::Options::size().rounds, 4);
    assert_eq!(pipeline::Options::aggressive().rounds, usize::MAX);
}

/// Sixty sweeps of `p[j] += v[j]` over 100 cells, as bench/particle makes them
/// (six sweeps are completely unrolled by `LoopFullUnroll`, which LLVM's order
/// runs before the jam, and leave no nest to jam),
/// through the whole pipeline at -O3 with and without unroll-and-jam, and at
/// -O2: the jam changes the code of -O3 alone, and the nest computes what it
/// did. The slot was built with the jam's `on` option off, so -O3 made the
/// same text as -O3 without the jam and the particle row never gained.
#[test]
fn only_o3_jams_a_nest_that_sweeps_the_same_cells() {
    let cells = |base: i32| (0..100).map(|at| format!("i16 {}", base + at % 7)).collect::<Vec<_>>().join(", ");
    let text = m16_ir(&format!(
        "target datalayout = \"@m16-datalayout@\"
@p = global [100 x i16] [{}]
@v = global [100 x i16] [{}]

define i16 @main() {{
b0:
  br label %outer

outer:
  %t = phi i16 [ 0, %b0 ], [ %tn, %outerlatch ]
  %go = icmp slt i16 %t, 60
  br i1 %go, label %pre, label %done

pre:
  br label %inner

inner:
  %j = phi i16 [ 0, %pre ], [ %jn, %body ]
  %more = icmp slt i16 %j, 100
  br i1 %more, label %body, label %outerlatch

body:
  %o = mul i16 %j, 2
  %pp = getelementptr i8, ptr @p, i16 %o
  %pv = getelementptr i8, ptr @v, i16 %o
  %a = load i16, ptr %pp
  %b = load i16, ptr %pv
  %s = add i16 %a, %b
  store i16 %s, ptr %pp
  %jn = add i16 %j, 1
  br label %inner

outerlatch:
  %tn = add i16 %t, 1
  br label %outer

done:
  %q = getelementptr i8, ptr @p, i16 40
  %w = load i16, ptr %q
  ret i16 %w
}}
",
        cells(1),
        cells(3)
    ));
    let through = |options: pipeline::Options| {
        let mut module = llrm_analysis::testing::parsed(&text);
        Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
            pipeline::applied(program, &Applied { options, ..Applied::default() })
        })
        .and_then(|done| done)
        .unwrap();
        (llrm_mir::print::module(&module), interpret::run(&module, "main", Vec::new(), FUEL))
    };
    let before = interpret::run(&llrm_analysis::testing::parsed(&text), "main", Vec::new(), FUEL);
    let (jammed, result) = through(pipeline::Options::speed());
    let (plain, plain_result) = through(pipeline::Options { unroll_and_jam: false, ..pipeline::Options::speed() });
    assert_eq!((&result, &plain_result), (&before, &before), "{jammed}");
    assert_ne!(jammed, plain);
    assert_eq!(
        through(pipeline::Options::default()).0,
        through(pipeline::Options { unroll_and_jam: false, ..Default::default() }).0
    );
}

/// Reads what the module says each function does to memory, and finds nothing
/// to do; counts how often it was asked.
struct ReadsCallees {
    runs: std::rc::Rc<std::cell::Cell<usize>>,
}

impl llrm_mir::passes::FunctionPass for ReadsCallees {
    fn name(&self) -> &'static str {
        "reads-callees"
    }

    fn run(
        &mut self,
        _: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        self.runs.set(self.runs.get() + 1);
        let _ = analyses.outer().callees().len();
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// A pipeline of the two passes that find nothing: one that reads the module's
/// facts and one that reads none.
fn watching(
    reader: &std::rc::Rc<std::cell::Cell<usize>>,
    blind: &std::rc::Rc<std::cell::Cell<usize>>,
) -> pipeline::Fixed {
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes = vec![
        Box::new(ReadsCallees { runs: std::rc::Rc::clone(reader) }),
        Box::new(Watch { name: "blind", reads: llrm_mir::footprint::Footprint::ALL, runs: std::rc::Rc::clone(blind) }),
    ];
    fixed
}

const CALLS_G: &str = "declare i16 @g(i16)
declare i16 @g2(i16)

define i16 @f(i16 %x) {
b0:
  %a = call i16 @g(i16 %x)
  %b = add i16 %a, %x
  ret i16 %b
}
";

/// The interprocedural stages sent a body that had found nothing back through
/// the pipeline for a callee's new name and fewer arguments: d_faces -O3 ran
/// its 1,200-operation body twice more, 19 and 31 passes, to change nothing,
/// 340 of 7,400 Minstr. A body that found nothing runs no pass again where
/// nothing it read has changed.
#[test]
fn a_body_run_again_with_nothing_changed_starts_no_pass() {
    let mut module = llrm_analysis::testing::parsed(CALLS_G);
    let (reader, blind) = (std::rc::Rc::default(), std::rc::Rc::default());
    crate::testing::managed(&mut module, watching(&reader, &blind));
    assert_eq!((reader.get(), blind.get()), (1, 1));
    crate::testing::managed(&mut module, watching(&reader, &blind));
    assert_eq!((reader.get(), blind.get()), (1, 1), "nothing changed: no pass is started");
}

/// A pass that read what the module says of a callee runs again where that
/// changed; one that read none of the module's facts does not.
#[test]
fn a_pass_runs_again_where_what_it_read_of_a_callee_changed_and_one_that_read_none_does_not() {
    let mut module = llrm_analysis::testing::parsed(CALLS_G);
    let (reader, blind) = (std::rc::Rc::default(), std::rc::Rc::default());
    crate::testing::managed(&mut module, watching(&reader, &blind));
    let (_, g) = module.function_mut("g").expect("g");
    g.attrs.push(llrm_mir::opcode::Attribute::Memory(vec![(None, "none".to_owned())]));
    crate::testing::managed(&mut module, watching(&reader, &blind));
    assert_eq!(reader.get(), 2, "@g now does nothing to memory, which the pass read");
    assert_eq!(blind.get(), 1, "the pass that read none of the module's facts is not asked");
}

/// A call sent to another function that says the same, by a stage that vouches
/// for the edit, is not a change to what the passes read; sent to one that says
/// otherwise it is.
#[test]
fn test_a_body_run_again_after_a_call_is_sent_to_an_equal_copy_starts_no_pass() {
    let retargeted = |write: bool| {
        let mut module = llrm_analysis::testing::parsed(CALLS_G);
        let (reader, blind) = (std::rc::Rc::default(), std::rc::Rc::default());
        crate::testing::managed(&mut module, watching(&reader, &blind));
        if write {
            let (_, g2) = module.function_mut("g2").expect("g2");
            g2.attrs.push(llrm_mir::opcode::Attribute::Memory(vec![(None, "none".to_owned())]));
        }
        let copy = module.named("g2").expect("g2");
        let (types_and_constants, f) = module.function_mut("f").expect("f");
        let before = f.mark();
        let call = f
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| matches!(f.instruction(inst).opcode, llrm_mir::opcode::Opcode::Call(_)))
            .expect("a call");
        let Some(&llrm_mir::module::Operand::Constant(old)) = f.instruction(call).operands.last() else {
            panic!("a direct call")
        };
        let ty = types_and_constants.get(old).ty;
        let new = types_and_constants
            .constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Global(copy) });
        let last = f.instruction(call).operands.len() - 1;
        f.set_operand(call, last, llrm_mir::module::Operand::Constant(new));
        module.context.extension::<crate::settled::Settled>().vouch(module.function_mut("f").expect("f").1, before);
        crate::testing::managed(&mut module, watching(&reader, &blind));
        (reader.get(), blind.get())
    };
    assert_eq!(retargeted(false), (1, 1), "the copy says what the callee said: no pass is started");
    assert_eq!(retargeted(true), (2, 1), "the copy writes nothing, which the pass that read callees asked");
}

/// -O1 gave small functions called more than once no inline (gcc's -O1) where
/// the pin's -O1 inlines them as -O2 does (d_faces: 11 defines in, 6 out; ours
/// 11), and searched allocations and made the spiller's second route (34% of
/// d_faces' -O1 compile).
#[test]
fn o1_inlines_as_the_pin_does_and_has_no_allocation_search_or_second_route() {
    let text = "
define internal i16 @twice(i16 %x) {
entry:
  %y = add i16 %x, 7
  ret i16 %y
}

define internal i16 @once(i16 %x) {
entry:
  %y = mul i16 %x, 3
  ret i16 %y
}

define i16 @main(i16 %a) {
entry:
  %p = call i16 @twice(i16 %a)
  %q = call i16 @twice(i16 %p)
  %r = call i16 @once(i16 %q)
  ret i16 %r
}
";
    let o1 = pipeline::Options::basic();
    assert!(!o1.searches());
    let o2 = pipeline::Options::standard();
    let (kept, _) = pipelined(text, o1);
    let kept = crate::testing::printed(&kept);
    let (gone, _) = pipelined(text, o2);
    let defines = |text: &str| text.lines().filter(|line| line.starts_with("define ")).count();
    assert_eq!(defines(&kept), defines(&crate::testing::printed(&gone)), "-O1 keeps what -O2 inlines:\n{kept}");
    assert_eq!(defines(&kept), 1, "-O1 leaves {} defines, the pin's -O1 only @main:\n{kept}", defines(&kept));
}

/// tile.c's inner sum is 40 trips of one load and one add. LLVM's full unroll
/// has no cap on the trips: it completes the loop where the unrolled size is
/// under the threshold, 150 at -O2 and 300 at -O3
/// (`UnrollThresholdAggressive`). gcc's `max-completely-peel-times` (16) kept
/// it a loop at -O3 (tile -O3 8,159 clocks against clang's 4,236).
#[test]
fn test_a_forty_trip_loop_is_completed_where_the_unrolled_size_is_under_the_threshold() {
    let text = "@t = global [40 x i16] zeroinitializer

define i16 @f() {
b0:
  br label %h

h:
  %i = phi i16 [ 0, %b0 ], [ %n, %b ]
  %s = phi i16 [ 0, %b0 ], [ %a, %b ]
  %go = icmp slt i16 %i, 40
  br i1 %go, label %b, label %x

b:
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr @t, i16 %o
  %v = load i16, ptr %p
  %a = add i16 %s, %v
  %n = add i16 %i, 1
  br label %h

x:
  ret i16 %s
}
";
    let looped = |options: pipeline::Options| {
        let mut module = llrm_analysis::testing::parsed(text);
        Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
            pipeline::applied(program, &Applied { options, ..Applied::default() })
        })
        .and_then(|done| done)
        .unwrap();
        llrm_mir::print::module(&module).contains("phi i16")
    };
    assert!(looped(pipeline::Options::standard()), "-O2: 202 over the threshold of 150 keeps 40 trips rolled");
    assert!(!looped(pipeline::Options::speed()), "-O3: 202 under 300 copies them out");
    assert!(!looped(pipeline::Options::aggressive()), "-Omax copies them out");
}

/// The LLVM order was off unless an env switch set it, so a default compile ran
/// the rounds, not the parity pipeline: the default now has the three stages
/// (early, cleanup, simplify) and the off knob the single fixed-point stage.
#[test]
fn test_the_llvm_order_is_the_default_and_its_off_switch_is_the_rounds() {
    let on = Applied::default();
    assert!(on.options.llvm_order, "the order is on by default");
    assert_eq!(pipeline::function_stages(&on).len(), 4, "early, cleanup, simplify and late");
    let off = Applied { options: pipeline::Options { llvm_order: false, ..Default::default() }, ..Applied::default() };
    assert_eq!(pipeline::function_stages(&off).len(), 1, "the rounds are one stage");
}

/// The translated SROA and PromoteMemToReg were switches that defaulted off
/// (`LLRM_EXP_SROA`, `LLRM_EXP_MEM2REG`) while the registry called their slots
/// Old: the parity pipeline ran the stand-ins. Now both are on by default, the
/// registry says Translated, and the explicit off knobs do not fail its check.
#[test]
fn test_sroa_and_mem2reg_are_on_by_default_and_the_registry_calls_their_slots_translated() {
    let on = Applied::default();
    assert!(on.options.sroa && on.options.mem2reg, "translated SROA and mem2reg are on by default");
    let kind = |slot: &str| crate::parity::REGISTRY.iter().find(|one| one.slot == slot).map(|one| one.kind);
    for slot in ["sroa@1179", "promote@1241", "sroa@646", "sroa@776"] {
        assert_eq!(kind(slot), Some(crate::parity::SlotKind::Translated), "{slot}");
    }
    let off = Applied {
        options: pipeline::Options { sroa: false, mem2reg: false, ..Default::default() },
        ..Applied::default()
    };
    for applied in [&on, &off] {
        for phase in [crate::parity::Phase::Early, crate::parity::Phase::Cleanup, crate::parity::Phase::Simplify] {
            let built = pipeline::llvm_order(applied, phase).iter().map(|one| one.name()).collect::<Vec<_>>();
            let level = crate::parity::Level::O2;
            let checked = crate::parity::check_phase_with(
                phase,
                &built,
                level,
                applied.options.sroa,
                applied.options.mem2reg,
                applied.options.rotate_slot,
                &[],
                &[],
            );
            assert_eq!(checked, Ok(()), "{phase:?} sroa {} mem2reg {}", applied.options.sroa, applied.options.mem2reg);
        }
    }
}

/// With `Options::llvm_order` a body goes through LLVM's early function
/// pipeline, GlobalCleanup and function simplification once each, in that
/// order, and `gvn` and `dse` run once; without it the rounds stand. The
/// EarlyCSE in both of its places is the translated pass.
#[test]
fn test_llvm_order_runs_the_function_pipeline_once_with_one_gvn_and_the_translated_earlycse() {
    let on = Applied { options: pipeline::Options { llvm_order: true, ..Default::default() }, ..Applied::default() };
    let off = Applied { options: pipeline::Options { llvm_order: false, ..Default::default() }, ..Applied::default() };
    assert_eq!(pipeline::function_stages(&off).len(), 1, "the rounds are one stage");
    assert_eq!(pipeline::function_stages(&on).len(), 4, "early, cleanup, simplify and late");
    let names = |phase| pipeline::llvm_order(&on, phase).iter().map(|one| one.name()).collect::<Vec<_>>();
    let simplify = names(crate::parity::Phase::Simplify);
    let count = |what: &str| simplify.iter().filter(|one| one.split('@').next() == Some(what)).count();
    assert_eq!((count("gvn"), count("dse"), count("earlycse")), (1, 1, 1), "{simplify:?}");
    assert!(names(crate::parity::Phase::Early).contains(&"earlycse@1180"), "{:?}", names(crate::parity::Phase::Early));
}

/// Old: LLVM's `InstSimplifyPass` (PassBuilderPipelines.cpp#L1662) was Missing
/// and never ran, so an identity left after the last pass stayed in the body.
/// Now its slot is Translated and the order ends with it.
#[test]
fn test_instsimplify_closes_the_function_pipeline_and_its_slot_is_translated() {
    let on = Applied { options: pipeline::Options { llvm_order: true, ..Default::default() }, ..Applied::default() };
    let late = pipeline::llvm_order(&on, crate::parity::Phase::Late).iter().map(|one| one.name()).collect::<Vec<_>>();
    assert_eq!(
        late,
        [
            "float2int@1594",
            "lower-constant-intrinsics@1597",
            "loop-interchange@1631",
            "infer-alignment@1362",
            "loop-load-elim@1400",
            "combine@1403",
            "simplifycfg@1438",
            "slp@1455",
            "combine@1464",
            "unroll@1476",
            "sroa@1494",
            "infer-alignment@1498",
            "combine@1499",
            "licm@1508",
            "instsimplify@1662",
            "divrempairs@1667",
            "tailrec@1677",
            "simplifycfg@1682"
        ]
    );
    let kind = crate::parity::REGISTRY.iter().find(|one| one.slot == "instsimplify@1662").map(|one| one.kind);
    assert_eq!(kind, Some(crate::parity::SlotKind::Translated));
}

/// A body computes the same answer through LLVM's order.
#[test]
fn test_llvm_order_keeps_a_loop_computing_the_same() {
    let text = "define i16 @f(i16 %n) {\nb0:\n  %p = alloca i16\n  store i16 0, ptr %p\n  br label %h\nh:\n  %i = phi i16 [ 0, %b0 ], [ %j, %b ]\n  %c = icmp slt i16 %i, %n\n  br i1 %c, label %b, label %x\nb:\n  %s = load i16, ptr %p\n  %t = add i16 %s, %i\n  store i16 %t, ptr %p\n  %j = add i16 %i, 1\n  br label %h\nx:\n  %r = load i16, ptr %p\n  ret i16 %r\n}\n";
    let mut module = llrm_analysis::testing::parsed(text);
    let before = interpret::run(&module, "f", vec![interpret::Val::Int { bits: 6, width: 16 }], FUEL);
    let applied =
        Applied { options: pipeline::Options { llvm_order: true, ..Default::default() }, ..Applied::default() };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap_or_else(|error| panic!("{error}"));
    let after = interpret::run(&module, "f", vec![interpret::Val::Int { bits: 6, width: 16 }], FUEL);
    assert_eq!(format!("{before:?}"), format!("{after:?}"));
    assert!(!llrm_mir::print::module(&module).contains("alloca"), "an alloca is left");
}

/// Turning a pass off (`unroll: false`) left its Translated slot out of the
/// order, and the parity check panicked ("slot unroll@753 is Translated and
/// not run in Simplify") in the sixteen nib tests that build such options.
#[test]
fn test_a_switched_off_pass_does_not_fail_the_slot_check() {
    let text = "define i16 @f(i16 %n) {\nb0:\n  ret i16 %n\n}\n";
    let mut module = llrm_analysis::testing::parsed(text);
    let applied = Applied {
        options: pipeline::Options { llvm_order: true, unroll: false, ..Default::default() },
        ..Applied::default()
    };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap_or_else(|error| panic!("{error}"));
}

/// #2194, x_hanoi2's `mv`: the loop `tailrec` makes of the second call has the
/// invariant argument `to` as a plain parameter, and its `mul` sits behind the
/// `n == 0` exit. One speculating `hoist` ahead of any rotate lifted it into
/// the entry block, before the test, so the frame was needed before the test
/// (x_hanoi2 -O2 +22.8%). LLVM's LPM1 hoists without speculation, rotates (a
/// guard before the loop), then hoists with it: the `mul` lands behind the
/// guard. `rotate_slot: false` holds the order to the failing form (the
/// premise).
#[test]
fn test_llvm_order_hoists_behind_the_guard_the_rotate_makes_not_into_the_entry() {
    let text = "@top = global [3 x i16] zeroinitializer
define void @mv(i16 %n, i16 %from, i16 %to, i16 %via) {
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %done, label %body
body:
  %m = sub i16 %n, 1
  call void @mv(i16 %m, i16 %from, i16 %via, i16 %to)
  %o = mul i16 %to, 2
  %p = getelementptr i8, ptr @top, i16 %o
  %t = load i16, ptr %p
  %t1 = add i16 %t, 1
  store i16 %t1, ptr %p
  call void @mv(i16 %m, i16 %from, i16 %to, i16 %via)
  ret void
done:
  ret void
}
";
    let entry_has_mul = |rotate_slot: bool| {
        let mut module = llrm_analysis::testing::parsed(text);
        let applied = Applied {
            options: pipeline::Options { llvm_order: true, rotate_slot, ..Default::default() },
            ..Applied::default()
        };
        let printed =
            crate::testing::managed(&mut module, pipeline::Fixed::linear(&applied, pipeline::Phase::Simplify));
        let entry = printed.split("\n\n").find(|one| one.contains("b0:")).expect("the entry block").to_owned();
        (entry.contains("mul i16") || entry.contains("shl i16"), printed)
    };
    let (without, plain) = entry_has_mul(false);
    assert!(
        without,
        "premise: without the rotate slot the mul (or the shift it becomes) is in the entry block\n{plain}"
    );
    let (with, rotated) = entry_has_mul(true);
    assert!(!with, "the mul (or shift) is behind the guard, not in the entry block\n{rotated}");
}

/// The rotate slot is LLVM's `LoopRotate`: a loop whose latch only increments
/// (`for (i = 0; ; i++)` with the test in the header after a body block) has
/// the increment folded into the block that exits (`simplifyLoopLatch`) and
/// is one block, test last. The stand-in `rotate` copies the header and leaves
/// the latch a block of its own.
#[test]
fn test_the_rotate_slot_folds_a_one_increment_latch_into_the_exit_test() {
    let text = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b3 ]
  %s = add i16 %acc, %i
  %go = icmp slt i16 %s, %n
  br i1 %go, label %b3, label %b2

b3:
  %next = add i16 %i, 1
  br label %b1

b2:
  %r = phi i16 [ %s, %b1 ]
  ret i16 %r
}
";
    let mut module = llrm_analysis::testing::parsed(text);
    let applied = Applied {
        options: pipeline::Options { llvm_order: true, rotate_slot: true, ..Default::default() },
        ..Applied::default()
    };
    let printed = crate::testing::managed(&mut module, pipeline::Fixed::linear(&applied, pipeline::Phase::Simplify));
    let looped = printed.split("\n\n").find(|one| one.contains("phi i16 [ 0")).expect("the loop block");
    assert!(looped.contains("add i16 %") && looped.contains("icmp slt") && looped.contains("br i1"), "{printed}");
    assert_eq!(printed.matches("phi i16 [ 0,").count(), 1, "{printed}");
    assert!(!printed.contains("b3:"), "the latch is folded into the exit test\n{printed}");
}

/// `LLRM_DUMP_INPUTS` for the POCs of single passes: a pass that finds nothing
/// to do left no dump, so its input was nowhere to be had. A run that changes
/// nothing now leaves its input and a manifest line.
#[test]
fn test_a_run_that_changes_nothing_leaves_its_input_and_a_manifest_line() {
    use llrm_mir::module::{GlobalKind, Module};
    use llrm_mir::passes::{Analyses, Declared, Outer, Unit};
    let directory = std::env::temp_dir().join(format!("llrm-inputs-{}", std::process::id()));
    let mut module: Module = crate::testing::parsed("define i16 @f() {\nb:\n  ret i16 0\n}\n");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let mut analyses = Analyses::new(std::rc::Rc::new(Outer::of(&module, None)));
    let mut declared = Declared::of(&module);
    let Module { context, globals, metadata, .. } = &mut module;
    let GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    let mut unit = Unit { context, layout: &layout, function, id: None, metadata, declared: &mut declared };
    let mut run = super::Run {
        version: 0,
        promotes: true,
        inputs: true,
        dump: Some(directory.clone()),
        rounds: 0,
        steps: 0,
        skipped: 0,
        fixed: 0,
        billing: false,
        idle: 0,
        useful: 0,
        idle_with_analyses: 0,
        useful_with_analyses: 0,
        classified: unit.function.mark(),
        stretches: Vec::new(),
        unreached: 0,
        slice: None,
        persisted: 0,
        records: unit.context.extension::<crate::settled::Settled>(),
        rerun: false,
        idle_at: Vec::new(),
    };
    assert!(!run.step(&mut Idle, &"idle", &mut unit, &mut analyses));
    let manifest = std::fs::read_to_string(directory.join("manifest.txt")).expect("a manifest");
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(manifest, "idle 1 0001-in-idle.ll 1\n");
}

/// A pass's stage name was formatted into a `String` for every step, 699 times
/// and 0.7 M Ir in d_faces, to be read only by a dump or a log. A step with
/// neither never formats it.
#[test]
fn test_a_step_that_dumps_and_logs_nothing_formats_no_stage_name() {
    use llrm_mir::module::{GlobalKind, Module};
    use llrm_mir::passes::{Analyses, Declared, Outer, Unit};
    let mut module: Module = crate::testing::parsed("define i16 @f() {\nb:\n  ret i16 0\n}\n");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let mut analyses = Analyses::new(std::rc::Rc::new(Outer::of(&module, None)));
    let mut declared = Declared::of(&module);
    let Module { context, globals, metadata, .. } = &mut module;
    let GlobalKind::Function(function) = &mut globals[0].kind else { panic!("a function") };
    let mut unit = Unit { context, layout: &layout, function, id: None, metadata, declared: &mut declared };
    let mut run = super::Run {
        version: 0,
        promotes: true,
        inputs: false,
        dump: None,
        rounds: 0,
        steps: 0,
        skipped: 0,
        fixed: 0,
        billing: false,
        idle: 0,
        useful: 0,
        idle_with_analyses: 0,
        useful_with_analyses: 0,
        classified: unit.function.mark(),
        stretches: Vec::new(),
        unreached: 0,
        slice: None,
        persisted: 0,
        records: unit.context.extension::<crate::settled::Settled>(),
        rerun: false,
        idle_at: Vec::new(),
    };
    struct Probe(std::cell::Cell<usize>);
    impl std::fmt::Display for Probe {
        fn fmt(
            &self,
            formatter: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result {
            self.0.set(self.0.get() + 1);
            formatter.write_str("probe")
        }
    }
    let stage = Probe(std::cell::Cell::new(0));
    assert!(!run.step(&mut Idle, &stage, &mut unit, &mut analyses));
    assert_eq!(stage.0.get(), 0, "the stage name was formatted for nobody");
}

struct Idle;

impl llrm_mir::passes::FunctionPass for Idle {
    fn name(&self) -> &'static str {
        "idle"
    }

    fn run(
        &mut self,
        _: &mut llrm_mir::passes::Unit,
        _: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// Erases the first instruction nothing uses.
struct EraseUnused;

impl llrm_mir::passes::FunctionPass for EraseUnused {
    fn name(&self) -> &'static str {
        "erase-unused"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        _: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        let unused = unit.function.walk().map(|(_, one)| one).find(|&one| {
            unit.function.instruction(one).result.is_some_and(|result| unit.function.users(result).is_empty())
        });
        let Some(unused) = unused else { return llrm_mir::passes::PreservedAnalyses::all() };
        unit.function.erase(unused).unwrap();
        llrm_mir::passes::PreservedAnalyses::none()
    }
}

/// The `runs` lines gave no sizes, so the body a slot enters and leaves was
/// guessed from the IR dumps, which map to the runs lines 12 to 34 on
/// d_faces. A step now says `in=` and `out=` (an idle one the same), and the
/// body line names the function.
#[test]
fn test_the_runs_lines_carry_the_sizes_a_step_enters_and_leaves_and_the_body_name() {
    let mut module = llrm_analysis::testing::parsed(
        "define i16 @f(i16 %x) {\nb0:\n  %d = add i16 %x, 1\n  %y = add i16 %x, 2\n  ret i16 %y\n}\n",
    );
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes = vec![Box::new(EraseUnused)];
    let (_, lines) = pipeline::capturing_runs(|| crate::testing::managed(&mut module, fixed));
    let steps: Vec<&String> = lines.iter().filter(|line| line.starts_with("step ")).collect();
    assert!(steps.first().is_some_and(|line| line.contains(" changed ") && line.ends_with(" in=3 out=2")), "{lines:?}");
    assert!(steps.last().is_some_and(|line| line.contains(" idle ") && line.ends_with(" in=2 out=2")), "{lines:?}");
    assert!(lines.iter().any(|line| line.starts_with("body ") && line.ends_with(" name f")), "{lines:?}");
}

/// Asks the ranger of every value at every block, and the table afresh, and
/// asserts they agree (`LLRM_CHECK_REPLAY`'s check, run as a pass).
struct Agree(&'static str);

impl llrm_mir::passes::FunctionPass for Agree {
    fn name(&self) -> &'static str {
        self.0
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        use llrm_analysis::manager::Ranges;
        if unit.function.layout().is_empty() {
            return llrm_mir::passes::PreservedAnalyses::all();
        }
        let held = Ranges::of(unit.context, unit.layout, unit.function, analyses);
        let seen = held.unit(unit.context, unit.layout, unit.function, analyses.outer());
        let facts = held.registers();
        let ranger = seen.edges.expect("edges").over(&seen, facts);
        let table = llrm_analysis::ranges::edges_solved(&seen, facts, None).unwrap();
        llrm_analysis::ranger::check_against(&ranger, &seen, &table);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// bintree.nib at -O2 -m16 through the LLVM order: after the cleanup stage the
/// ranger, asked of every value before, gave `[0,32767]` of a value at the
/// bounds-failure block where the table had no entry (`LLRM_CHECK_REPLAY`
/// panicked at ranger.rs:925, #2369).
#[test]
fn test_the_ranger_and_the_table_agree_between_the_stages_of_the_llvm_order() {
    let mut module = crate::testing::parsed(m16_ir("target datalayout = \"@m16-datalayout@\"

define internal cc 1005 void @insert(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, i16 %1, i16 %2) addrspace(1) nearcode memory(readwrite, argmem: read) {
b1:
  %3 = alloca i16
  store i16 2, ptr %3, !tbaa !2
  %4 = mul i16 3, %2
  %5 = load i16, ptr addrspace(1) %0, !range !5
  %6 = icmp ult i16 %4, %5
  br i1 %6, label %b2, label %b3

b2:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %8 = load ptr addrspace(1), ptr addrspace(1) %7
  %9 = mul i16 %4, 2
  %10 = getelementptr inbounds i8, ptr addrspace(1) %8, i16 %9
  %11 = load i16, ptr addrspace(1) %10
  %12 = mul i16 3, %1
  %13 = load i16, ptr addrspace(1) %0, !range !6
  %14 = icmp ult i16 %12, %13
  br i1 %14, label %b4, label %b5

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b4:
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %16 = load ptr addrspace(1), ptr addrspace(1) %15
  %17 = mul i16 %12, 2
  %18 = getelementptr inbounds i8, ptr addrspace(1) %16, i16 %17
  %19 = load i16, ptr addrspace(1) %18
  %20 = icmp slt i16 %11, %19
  br i1 %20, label %b6, label %b7

b5:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b6:
  store i16 1, ptr %3, !tbaa !2
  br label %b8

b7:
  br label %b8

b8:
  %21 = mul i16 3, %1
  %22 = load i16, ptr %3, !tbaa !2
  %23 = add i16 %21, %22
  %24 = load i16, ptr addrspace(1) %0, !range !7
  %25 = icmp ult i16 %23, %24
  br i1 %25, label %b9, label %b10

b9:
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %27 = load ptr addrspace(1), ptr addrspace(1) %26
  %28 = mul i16 %23, 2
  %29 = getelementptr inbounds i8, ptr addrspace(1) %27, i16 %28
  %30 = load i16, ptr addrspace(1) %29
  %31 = icmp eq i16 %30, 0
  br i1 %31, label %b11, label %b12

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  %32 = mul i16 3, %1
  %33 = load i16, ptr %3, !tbaa !2
  %34 = add i16 %32, %33
  %35 = load i16, ptr addrspace(1) %0, !range !8
  %36 = icmp ult i16 %34, %35
  br i1 %36, label %b14, label %b15

b12:
  %37 = mul i16 3, %1
  %38 = load i16, ptr %3, !tbaa !2
  %39 = add i16 %37, %38
  %40 = load i16, ptr addrspace(1) %0, !range !9
  %41 = icmp ult i16 %39, %40
  br i1 %41, label %b16, label %b17

b13:
  ret void

b14:
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %43 = load ptr addrspace(1), ptr addrspace(1) %42
  %44 = mul i16 %34, 2
  %45 = getelementptr inbounds i8, ptr addrspace(1) %43, i16 %44
  store i16 %2, ptr addrspace(1) %45
  br label %b13

b15:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  %46 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %47 = load ptr addrspace(1), ptr addrspace(1) %46
  %48 = mul i16 %39, 2
  %49 = getelementptr inbounds i8, ptr addrspace(1) %47, i16 %48
  %50 = load i16, ptr addrspace(1) %49
  call cc 1005 addrspace(1) void @insert(ptr addrspace(1) %0, i16 %50, i16 %2)
  br label %b13

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i32 @walk(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, i16 %1, i16 %2) addrspace(1) nearcode memory(read, inaccessiblemem: readwrite) {
b1:
  %3 = icmp eq i16 %1, 0
  br i1 %3, label %b2, label %b3

b2:
  ret i32 0

b3:
  br label %b4

b4:
  %4 = mul i16 3, %1
  %5 = add i16 %4, 1
  %6 = load i16, ptr addrspace(1) %0, !range !10
  %7 = icmp ult i16 %5, %6
  br i1 %7, label %b5, label %b6

b5:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = mul i16 %5, 2
  %11 = getelementptr inbounds i8, ptr addrspace(1) %9, i16 %10
  %12 = load i16, ptr addrspace(1) %11
  %13 = add i16 %2, 1
  %14 = call cc 1005 addrspace(1) i32 @walk(ptr addrspace(1) %0, i16 %12, i16 %13)
  %15 = mul i16 3, %1
  %16 = load i16, ptr addrspace(1) %0, !range !11
  %17 = icmp ult i16 %15, %16
  br i1 %17, label %b7, label %b8

b6:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b7:
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %19 = load ptr addrspace(1), ptr addrspace(1) %18
  %20 = mul i16 %15, 2
  %21 = getelementptr inbounds i8, ptr addrspace(1) %19, i16 %20
  %22 = load i16, ptr addrspace(1) %21
  %23 = sext i16 %22 to i32
  %24 = sext i16 %2 to i32
  %25 = mul i32 %23, %24
  %26 = add i32 %14, %25
  %27 = mul i16 3, %1
  %28 = add i16 %27, 2
  %29 = load i16, ptr addrspace(1) %0, !range !12
  %30 = icmp ult i16 %28, %29
  br i1 %30, label %b9, label %b10

b8:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b9:
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %32 = load ptr addrspace(1), ptr addrspace(1) %31
  %33 = mul i16 %28, 2
  %34 = getelementptr inbounds i8, ptr addrspace(1) %32, i16 %33
  %35 = load i16, ptr addrspace(1) %34
  %36 = add i16 %2, 1
  %37 = call cc 1005 addrspace(1) i32 @walk(ptr addrspace(1) %0, i16 %35, i16 %36)
  %38 = add i32 %26, %37
  ret i32 %38

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i32 @bench_bintree(i16 %0) addrspace(1) nearcode memory(readwrite, argmem: none) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca [1806 x i8]
  %11 = alloca i16
  store i16 0, ptr %11, !tbaa !2
  store i16 903, ptr %8, !tbaa !2
  store i16 903, ptr %9, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 903, ptr %6, !tbaa !2
  br label %b2

b2:
  %12 = load i16, ptr %7, !tbaa !2
  %13 = load i16, ptr %6, !tbaa !2
  %14 = icmp slt i16 %12, %13
  br i1 %14, label %b3, label %b5

b3:
  %15 = load i16, ptr %7, !tbaa !2
  %16 = load i16, ptr %11, !tbaa !2
  %17 = sub i16 %15, 0
  %18 = getelementptr inbounds i16, ptr %10, i16 %17
  store i16 %16, ptr %18, !tbaa !2
  br label %b4

b4:
  %19 = load i16, ptr %7, !tbaa !2
  %20 = add nsw i16 %19, 1
  store i16 %20, ptr %7, !tbaa !2
  br label %b2

b5:
  %21 = zext i16 %0 to i32
  store i32 %21, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  store i16 301, ptr %3, !tbaa !2
  br label %b6

b6:
  %22 = load i16, ptr %4, !tbaa !2
  %23 = load i16, ptr %3, !tbaa !2
  %24 = icmp slt i16 %22, %23
  br i1 %24, label %b7, label %b9

b7:
  %25 = load i32, ptr %5, !tbaa !2
  %26 = mul i32 %25, 25173
  %27 = add i32 %26, 13849
  %28 = and i32 %27, 65535
  store i32 %28, ptr %5, !tbaa !2
  %29 = load i16, ptr %4, !tbaa !2
  %30 = mul i16 3, %29
  %31 = icmp ult i16 %30, 903
  br i1 %31, label %b10, label %b11

b8:
  %32 = load i16, ptr %4, !tbaa !2
  %33 = add nsw i16 %32, 1
  store i16 %33, ptr %4, !tbaa !2
  br label %b6

b9:
  %34 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 903, ptr %1, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 903, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %34, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %1 to ptr addrspace(1)
  %38 = call cc 1005 addrspace(1) i32 @walk(ptr addrspace(1) %37, i16 1, i16 1)
  ret i32 %38

b10:
  %39 = load i32, ptr %5, !tbaa !2
  %40 = and i32 %39, 4095
  %41 = trunc i32 %40 to i16
  %42 = sub i16 %30, 0
  %43 = getelementptr inbounds i16, ptr %10, i16 %42
  store i16 %41, ptr %43, !tbaa !2
  %44 = load i16, ptr %4, !tbaa !2
  %45 = icmp sgt i16 %44, 1
  br i1 %45, label %b12, label %b13

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b12:
  %46 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 903, ptr %2, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 903, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %46, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %2 to ptr addrspace(1)
  %50 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @insert(ptr addrspace(1) %49, i16 1, i16 %50)
  br label %b14

b13:
  br label %b14

b14:
  br label %b8
}

define cc 1005 i16 @main() addrspace(1) nearcode memory(readwrite, argmem: none) {
b1:
  %0 = alloca i32
  %1 = call cc 1005 addrspace(1) i32 @bench_bintree(i16 1)
  store i32 %1, ptr %0, !tbaa !2
  %2 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI4(i32 %2)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!\"llrm hir\"}
!1 = !{!\"place\", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!\"allocation\", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
!7 = !{i16 0, i16 -32768}
!8 = !{i16 0, i16 -32768}
!9 = !{i16 0, i16 -32768}
!10 = !{i16 0, i16 -32768}
!11 = !{i16 0, i16 -32768}
!12 = !{i16 0, i16 -32768}
"));
    let applied = Applied::default();
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(Agree("input"));
    for phase in [crate::parity::Phase::Early, crate::parity::Phase::Cleanup, crate::parity::Phase::Simplify] {
        manager.add(pipeline::Fixed::linear(&applied, phase));
        manager.add(Agree(match phase {
            crate::parity::Phase::Early => "after early",
            crate::parity::Phase::Cleanup => "after cleanup",
            _ => "after simplify",
        }));
    }
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
}

/// GlobalsAA was made 9 times for one compile of d_faces -O2 (8 for the corpus
/// module below): each program pass dropped it and the next function pass made
/// it again. LLVM makes it twice, at `RequireAnalysisPass<GlobalsAA>` before
/// the inliner and `RecomputeGlobalsAAPass`, and keeps it past every pass
/// between.
#[test]
fn test_the_llvm_order_does_not_make_globals_aa_again_for_each_function_stage() {
    assert!(
        std::env::var_os("LLRM_CHECK_STALE").is_none(),
        "counts recomputes: LLRM_CHECK_STALE recomputes held analyses and adds to them"
    );
    let mut files = corpus_files();
    files.sort_by_key(|(_, path)| std::cmp::Reverse(std::fs::metadata(path).map(|one| one.len()).unwrap_or(0)));
    let (name, path) = files
        .iter()
        .find(|(name, path)| {
            let module = parse_corpus_entry(name, path);
            module.functions().filter(|(_, _, one)| !one.is_declaration()).count() >= 3
        })
        .expect("a corpus module with three bodies");
    let mut module = parse_corpus_entry(name, path);
    let before = llrm_mir::counts::analysed("globals-aa").1;
    let applied = Applied::default();
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
        pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap_or_else(|error| panic!("{name}: {error}"));
    let made = llrm_mir::counts::analysed("globals-aa").1 - before;
    assert_eq!(made, 2, "{name}: globals-aa made other than at LLVM's two points");
}

/// The fixed point hashed the body before it ran even when its list runs once
/// (LLVM's order): a hash per body per stage that nothing read, 1.1 M
/// instructions on d_faces -O2.
#[test]
fn test_the_llvm_order_hashes_no_body_for_a_cycle_history_it_never_reads() {
    let mut files = corpus_files();
    files.sort_by_key(|(_, path)| std::cmp::Reverse(std::fs::metadata(path).map(|one| one.len()).unwrap_or(0)));
    let run = |name: &str, path: &std::path::Path, applied: &Applied| {
        let mut module = parse_corpus_entry(name, path);
        let before = llrm_mir::counts::get(llrm_mir::counts::Event::Fingerprints);
        let done =
            Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
                pipeline::applied(program, applied)
            })
            .and_then(|done| done);
        done.map(|()| llrm_mir::counts::get(llrm_mir::counts::Event::Fingerprints) - before)
    };
    let order = Applied::default();
    assert!(order.options.llvm_order, "the order is the default");
    let (name, path) = files.first().expect("a corpus module");
    assert_eq!(
        run(name, path, &order).unwrap_or_else(|error| panic!("{name}: {error}")),
        0,
        "{name}: the order hashed bodies it never compares"
    );
    let mut rounds = Applied::default();
    rounds.options.llvm_order = false;
    // The rounds panic on a body that oscillates (a separate, older fault in
    // some corpus entries): the smallest entry that converges shows the
    // history.
    let kept = files
        .iter()
        .rev()
        .find_map(
            |(name, path)| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(name, path, &rounds).ok().filter(|hashed| *hashed > 0)
            }))
            .ok()
            .flatten(),
        );
    assert!(kept.is_some(), "the rounds keep the history they compare");
}

/// `loopsimplify` gives a loop a dedicated exit, a jump alone in a block, and
/// `simplifycfg` took it out again: round after round the body came back to
/// the same state, and the `deedlines` demo failed with "cycle after 3 rounds".
#[test]
fn simplifycfg_and_loopsimplify_reach_a_fixed_point_on_a_loop_with_a_shared_exit() {
    let mut module = llrm_analysis::testing::parsed(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b1 ]
  %n = add i16 %i, 1
  %t = icmp ult i16 %n, %x
  br i1 %t, label %b1, label %b2

b2:
  %r = phi i16 [ 0, %b0 ], [ %n, %b1 ]
  ret i16 %r
}
",
    );
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes =
        vec![Box::new(crate::simplifycfg::SimplifyCfg::default()), Box::new(crate::loopsimplify::LoopSimplify)];
    crate::testing::managed(&mut module, fixed);
}
/// d_faces -O2: the interprocedural stage ran body 32 again (`_clip_w`) and its
/// `[runs] body` line named `_d_draw_faces`, the last function the manager's
/// loop had set. A rerun names the function it runs.
#[test]
fn test_a_rerun_of_a_body_names_its_own_function_on_the_runs_line() {
    let mut module = llrm_analysis::testing::parsed(
        "define i16 @f(i16 %x) {\nb0:\n  %d = add i16 %x, 1\n  ret i16 %x\n}\n\ndefine i16 @g(i16 %x) {\nb0:\n  ret i16 %x\n}\n",
    );
    let mut analyses = llrm_mir::passes::ModuleAnalyses::of(&module, std::rc::Rc::new(llrm_x86_m16::Dos::default()));
    let mut fixed = pipeline::Fixed::new(&Applied { only: Some("none".to_owned()), ..Applied::default() });
    fixed.only = false;
    fixed.passes = vec![Box::new(EraseUnused)];
    // The manager's loop last set `g`.
    llrm_mir::passes::set_running_function("g");
    let (_, lines) = pipeline::capturing_runs(|| {
        super::rerun(&mut module, &mut analyses, llrm_mir::context::GlobalId(0), &mut fixed, false).unwrap()
    });
    assert!(lines.iter().any(|line| line.starts_with("body ") && line.ends_with(" name f")), "{lines:?}");
}

/// The rotate slot (the pin's LPM1 `loop-rotate`, L728) used to be left out
/// where unroll-and-jam ran, so -O3 and -Omax ran no loop-rotate that the pin
/// runs. It is in the order at every level.
#[test]
fn test_the_rotate_slot_runs_at_every_level() {
    for options in [pipeline::Options::default(), pipeline::Options::speed(), pipeline::Options::aggressive()] {
        let applied = Applied { options, ..Applied::default() };
        let built = pipeline::llvm_order(&applied, crate::parity::Phase::Simplify);
        assert!(built.iter().any(|one| one.name() == "rotate@728"), "no rotate@728 in the order");
    }
}

/// LPM1 runs `loop-rotate` and then `licm`; the pin has no LoopSimplify or
/// LCSSA between them (the loop adaptor runs them once, before LPM1). We ran
/// both after the rotate: 1.7 M Ir at d_faces -O2, no object byte differed.
#[test]
fn test_no_loopsimplify_or_lcssa_runs_between_the_rotate_and_the_second_licm() {
    for options in [pipeline::Options::default(), pipeline::Options::size(), pipeline::Options::speed()] {
        let applied = Applied { options, ..Applied::default() };
        let names: Vec<&str> =
            pipeline::llvm_order(&applied, crate::parity::Phase::Simplify).iter().map(|one| one.name()).collect();
        let at = names.iter().position(|one| *one == "rotate@728").expect("rotate@728 is in the order");
        assert_eq!(names[at + 1], "licm@730", "{names:?}");
    }
}

/// `llrm-pass --order` could not name a slot of LLVM's order: the oracle ran
/// only the old pass names, so the translated SROA, promote and EarlyCSE slots
/// were never compared with `opt`. A slot is selectable when the level builds
/// it; `gvn@787` is not built at -O1.
#[test]
fn test_a_slot_of_the_order_can_be_run_alone_when_the_level_builds_it() {
    let o2 = Applied::default();
    for slot in ["sroa@1179", "promote@1241", "earlycse@649", "unroll@753", "instsimplify@1662", "memcpyopt@818"] {
        assert!(pipeline::slot_built(&o2, slot), "{slot}");
    }
    assert!(!pipeline::slot_built(&o2, "sroa@1"), "no such slot");
    assert!(!pipeline::slot_built(&o2, "gvn"), "a pass name is no slot");
    let o1 = Applied { options: pipeline::Options { pre: false, ..Default::default() }, ..Applied::default() };
    assert!(!pipeline::slot_built(&o1, "gvn@787"), "-O1 has no GVN");
}

/// The order had no SimplifyCFG after the Simplify stage: L1438 (hoist and
/// sink common code, loops need not stay canonical) and L1682 were Missing.
/// Their slots are built, in the pin's order around `instsimplify@1662`.
#[test]
fn test_the_late_simplifycfg_slots_are_built_in_the_pins_order() {
    let o2 = Applied::default();
    for slot in ["simplifycfg@1438", "simplifycfg@1682"] {
        assert!(pipeline::slot_built(&o2, slot), "{slot}");
    }
    let on = Applied { options: pipeline::Options { llvm_order: true, ..Default::default() }, ..Applied::default() };
    let late = pipeline::llvm_order(&on, crate::parity::Phase::Late).iter().map(|one| one.name()).collect::<Vec<_>>();
    assert_eq!(
        late,
        [
            "float2int@1594",
            "lower-constant-intrinsics@1597",
            "loop-interchange@1631",
            "infer-alignment@1362",
            "loop-load-elim@1400",
            "combine@1403",
            "simplifycfg@1438",
            "slp@1455",
            "combine@1464",
            "unroll@1476",
            "sroa@1494",
            "infer-alignment@1498",
            "combine@1499",
            "licm@1508",
            "instsimplify@1662",
            "divrempairs@1667",
            "tailrec@1677",
            "simplifycfg@1682"
        ]
    );
}

/// d_faces -O2 under the LLVM order ran `hoist` three times in its slots and
/// `sink` (a register-pressure forecast) after the fixed point, where LLVM has
/// no such pass; `sink` is gone and the translated LICM runs in the pin's four
/// slots and not after gcc's header copy, which LLVM's order does not run
/// (`rotate` is not a slot yet).
#[test]
fn test_the_llvm_order_runs_licm_in_the_pins_four_slots_and_not_after_a_header_copy() {
    let text = "define i16 @f(i16 %n, i16 %a) {\nb0:\n  br label %h\nh:\n  %i = phi i16 [ 0, %b0 ], [ %j, %b ]\n  %c = icmp slt i16 %i, %n\n  br i1 %c, label %b, label %x\nb:\n  %k = mul i16 %a, 3\n  %j = add i16 %i, %k\n  br label %h\nx:\n  ret i16 %i\n}\n";
    llrm_mir::counts::per_pass();
    let ran = |licm: bool| {
        let mut module = llrm_analysis::testing::parsed(text);
        let applied = Applied { options: pipeline::Options { licm, ..Default::default() }, ..Applied::default() };
        let before = llrm_mir::counts::lines();
        Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
            pipeline::applied(program, &applied)
        })
        .and_then(|done| done)
        .expect("the pipeline runs");
        // Every slot of the pass is a row of its own (`licm@724`).
        let runs = |lines: &[String], name: &str| {
            lines
                .iter()
                .filter_map(|line| line.strip_prefix("[counts] pass ")?.split_once(' '))
                .filter(|(row, _)| row.split('@').next() == Some(name))
                .filter_map(|(_, rest)| rest.split(' ').next()?.parse::<u64>().ok())
                .sum::<u64>()
        };
        let after = llrm_mir::counts::lines();
        runs(&after, "licm") - runs(&before, "licm")
    };
    assert_eq!(ran(true), 4, "licm@724, licm@730, licm@823 and licm@1508; none after a header copy");
}

/// The write-through `Promote` ran after the translated SROA and mem2reg at
/// every level: 34.6 M instructions of d_faces -O2 (1,894.7 -> 1,860.1 M) for
/// assembly GVN already produces. Only -O1 (no GVN `pre`) keeps it.
#[test]
fn test_the_write_through_promote_runs_after_sroa_only_where_gvn_has_no_pre() {
    let parts = |applied: &Applied| {
        let mut slots = Vec::new();
        for phase in [crate::parity::Phase::Early, crate::parity::Phase::Cleanup, crate::parity::Phase::Simplify] {
            for one in pipeline::llvm_order(applied, phase) {
                if matches!(
                    one.name(),
                    "sroa@1179" | "promote@1241" | "sroa@646" | "sroa@776"
                ) {
                    slots.push((one.name(), one.parts()));
                }
            }
        }
        slots
    };
    let o2 = parts(&Applied::default());
    assert_eq!(o2.len(), 4);
    for (slot, parts) in &o2 {
        assert!(!parts.contains(&"promote") && !parts.contains(&"deadslots"), "{slot} {parts:?}");
    }
    let o1 = Applied { options: pipeline::Options { pre: false, ..Default::default() }, ..Applied::default() };
    for (slot, parts) in &parts(&o1) {
        assert!(parts.contains(&"promote"), "{slot} {parts:?}");
    }
}

/// `llrm-pass -O3 --order SLOT` ran the peeler in the slot's place:
/// `transacted` runs a peeler first when `only`, and `Fixed::slot` kept it.
/// Every oracle comparing a Simplify-phase slot at -O3 or -Omax compared the
/// peeler's output.
#[test]
fn test_a_slot_run_alone_at_o3_is_the_slot_and_not_the_peeler() {
    let text = "define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %body ]
  br i1 %z, label %out, label %test

test:
  %done = icmp eq i16 %i, 0
  br i1 %done, label %out, label %body

body:
  %next = sub i16 %i, 1
  %sum = add i16 %acc, %i
  br label %head

out:
  %r = phi i16 [ %acc, %head ], [ %acc, %test ]
  ret i16 %r
}
";
    let entry_branches = |options: pipeline::Options| {
        let mut module = llrm_analysis::testing::parsed(text);
        let applied = Applied { only: Some("trivialunswitch@732".to_owned()), options, ..Applied::default() };
        assert!(pipeline::slot_built(&applied, "trivialunswitch@732"));
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add(pipeline::Fixed::new(&applied));
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).expect("the slot runs");
        let after = llrm_mir::print::module(&module);
        after[after.find("b0:").expect("b0")..].split("\n\n").next().expect("a block").contains("br i1 %z")
    };
    // -O1 to -Omax: the ones with `peel` on ran the peeler in the slot's place.
    for (level, options) in [
        ("-O1", pipeline::Options::basic()),
        ("-O2", pipeline::Options::standard()),
        ("-Os", pipeline::Options::size()),
        ("-O3", pipeline::Options::speed()),
        ("-Omax", pipeline::Options::aggressive()),
    ] {
        assert!(entry_branches(options), "{level}: the slot did not unswitch the header test");
    }
}

/// recmany -O1 -m32 ran 1500 frames deep (60 KB of stack, DOS/32A exception
/// 06h): the pin's -O1 has TailCallElim only at PassBuilderPipelines.cpp#L1677,
/// a slot ours did not have, so a self-recursive tail call stayed a call. The
/// slot must be built at every level and turn the call into a loop.
#[test]
fn test_the_late_tail_call_slot_makes_a_loop_of_a_self_call_at_every_level() {
    let text = "define i32 @f(i32 %a, i32 %n) {\nb0:\n  %z = icmp eq i32 %n, 0\n  br i1 %z, label %done, label %rec\nrec:\n  %m = sub i32 %n, 1\n  %b = add i32 %a, %n\n  %r = tail call i32 @f(i32 %b, i32 %m)\n  ret i32 %r\ndone:\n  ret i32 %a\n}\n";
    for (level, options) in [
        ("-O1", pipeline::Options::basic()),
        ("-O2", pipeline::Options::standard()),
        ("-Os", pipeline::Options::size()),
        ("-O3", pipeline::Options::speed()),
    ] {
        let mut module = llrm_analysis::testing::parsed(text);
        let applied = Applied { only: Some("tailrec@1677".to_owned()), options, ..Applied::default() };
        assert!(pipeline::slot_built(&applied, "tailrec@1677"), "{level}: no slot");
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add(pipeline::Fixed::new(&applied));
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).expect("the slot runs");
        let after = llrm_mir::print::module(&module);
        assert!(!after.contains("call i32 @f"), "{level}: the self call stayed:\n{after}");
    }
}

/// Drops one of the two inputs of the phi that two edges feed (#3016).
#[cfg(debug_assertions)]
struct ShortPhi;

#[cfg(debug_assertions)]
impl llrm_mir::passes::FunctionPass for ShortPhi {
    fn name(&self) -> &'static str {
        "short-phi"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        _: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        let function = &mut *unit.function;
        let block = function.layout()[1];
        let phi = function.iter_block(block).next().unwrap();
        let first = function.instruction(phi).operands[..2].to_vec();
        function.set_operands(phi, first);
        llrm_mir::passes::PreservedAnalyses::none()
    }
}

/// A slot of several passes names the member that left a phi short of an
/// edge, not the slot: SROA's `Promote` and not "sroa@646".
#[test]
#[cfg(debug_assertions)]
fn a_slot_names_the_member_that_left_a_phi_short() {
    let text = "define i16 @p(i1 %c) {\nentry:\n  br i1 %c, label %b, label %b\nb:\n  %x = phi i16 [ 1, %entry ], [ 1, %entry ]\n  ret i16 %x\n}\n";
    let mut module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.add(super::Together { name: "slot@1", parts: vec![Box::new(ShortPhi)] });
    let error = passes.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap_err();
    assert!(error.starts_with("after short-phi: "), "{error}");
}

/// `ret` took `frame` in, and `selftest` was then given the body `ret` had
/// before, with its calls to `frame`, which had no body left: the load of
/// `b.draw` was folded past them and `selftest` returned 0, not 1. Run at
/// -O2: at -Os `frame` stays a call, for its body is bigger than the call
/// (D11).
#[test]
fn an_inlined_body_is_spliced_as_it_is_after_it_took_its_own_callees_in() {
    let mut module = llrm_analysis::testing::parsed(m16_ir(
        r#"target datalayout = "@m16-datalayout@"

define internal cc 1005 void @"_frame@3"(ptr addrspace(1) nocapture %0, i16 %1) memory(readwrite, inaccessiblemem: none) willreturn nounwind norecurse {
b1:
  %2 = icmp ne i16 %1, 0
  br i1 %2, label %b3, label %b2

b2:
  %3 = load i16, ptr addrspace(1) %0, !tbaa !9
  %4 = and i16 %3, -3
  store i16 %4, ptr addrspace(1) %0, !tbaa !9
  br label %b4

b3:
  %5 = load i16, ptr addrspace(1) %0, !tbaa !9
  %6 = or i16 %5, 2
  store i16 %6, ptr addrspace(1) %0, !tbaa !9
  br label %b4

b4:
  ret void
}

define internal cc 1005 void @"_ret@3"(ptr addrspace(1) nocapture %0, ptr addrspace(1) nocapture %1) memory(readwrite, inaccessiblemem: none) willreturn nounwind norecurse {
b1:
  store i16 3, ptr addrspace(1) %0, !tbaa !9
  call cc 1005 void @"_frame@3"(ptr addrspace(1) %1, i16 0)
  call cc 1005 void @"_frame@3"(ptr addrspace(1) %1, i16 0)
  ret void
}

define internal cc 1005 range(i16 0, 2) i16 @"_selftest@3"() memory(readwrite, inaccessiblemem: none) willreturn nounwind norecurse {
b1:
  %0 = alloca [2 x i8]
  %1 = alloca [2 x i8]
  store i16 1, ptr %1, !tbaa !9
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  call cc 1005 void @"_frame@3"(ptr addrspace(1) %2, i16 -1)
  %3 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1005 void @"_ret@3"(ptr addrspace(1) %3, ptr addrspace(1) %2)
  %4 = load i16, ptr %0, !tbaa !9
  %5 = icmp eq i16 %4, 3
  %6 = load i16, ptr %1
  %7 = icmp eq i16 %6, 1
  %or.cond = select i1 %5, i1 %7, i1 false
  br i1 %or.cond, label %b5, label %b3

b3:
  %8 = phi i16 [ 1, %b5 ], [ 0, %b1 ]
  ret i16 %8

b5:
  br label %b3
}

define i16 @_main() addrspace(1) {
b1:
  %0 = call cc 1005 i16 @"_selftest@3"()
  %1 = sext i16 %0 to i32
  %2 = call cc 1005 addrspace(1) i16 @"_report@3"(i32 %1)
  ret i16 0
}

declare cc 1005 i16 @"_report@3"(i32) addrspace(1)

!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
"#,
    ));
    let applied = Applied { options: pipeline::Options::default(), ..Applied::default() };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("_main".to_owned());
        pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap();
    let printed = llrm_mir::print::module(&module);
    assert!(printed.contains("@\"_report@3\"(i32 1)"), "selftest returned 0, not 1:\n{printed}");
}

/// The pin builds `SimpleLoopUnswitchPass(NonTrivial = Level == O3)`: the slot
/// copied a loop for each way of an invariant branch at -O3 only, where
/// the old `unswitch` pass ran after the fixed point and judged candidates on
/// a pipeline of its own.
#[test]
fn test_the_unswitch_slot_copies_a_loop_for_an_invariant_branch_at_o3_only() {
    let text = "define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %latch ]
  br i1 %z, label %then, label %else

then:
  %a = add i16 %acc, %i
  br label %latch

else:
  %b = mul i16 %acc, 3
  br label %latch

latch:
  %sum = phi i16 [ %a, %then ], [ %b, %else ]
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %out, label %head

out:
  %r = phi i16 [ %sum, %latch ]
  ret i16 %r
}
";
    let loops = |unswitch: bool| {
        let mut module = llrm_analysis::testing::parsed(text);
        let applied = Applied {
            only: Some("trivialunswitch@732".to_owned()),
            options: pipeline::Options { unswitch, ..Default::default() },
            ..Applied::default()
        };
        assert!(pipeline::slot_built(&applied, "trivialunswitch@732"));
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add(pipeline::Fixed::new(&applied));
        manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).expect("the slot runs");
        llrm_mir::print::module(&module).matches("icmp eq i16").count() - 1
    };
    assert_eq!((loops(false), loops(true)), (1, 2));
}

/// A `short` counter, `i = (short)(i + 1)` under `i < N`, stayed an i32 phi
/// with `ashr (shl i, 16), 16` after InstCombine: the pin's IPSCCP marks the
/// `trunc` nsw from the loop's bound first, and `sext (trunc nsw x)` is `x`.
/// SCEV found no trip count in the other form and LoopIdiom made no memmove:
/// 8.8x the instructions of bench `scroll` (166810 against 1466660), matmul -O3
/// unrolled no more.
#[test]
fn a_short_counter_loop_is_still_a_memmove() {
    let mut module = llrm_analysis::testing::parsed(&format!(
        "{}@screen = internal global [2000 x i16] zeroinitializer

define void @f() {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
  %w = sext i16 %i to i32
  %go = icmp slt i32 %w, 1920
  br i1 %go, label %b2, label %b3

b2:
  %up = add nsw i32 %w, 80
  %src = getelementptr inbounds i16, ptr @screen, i32 %up
  %v = load i16, ptr %src
  %dst = getelementptr inbounds i16, ptr @screen, i32 %w
  store i16 %v, ptr %dst
  %next = add nsw i32 %w, 1
  %n = trunc i32 %next to i16
  br label %b1

b3:
  ret void
}}
",
        llrm_analysis::testing::DOS
    ));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    assert!(text.contains("llvm.memmove"), "{text}");
}

/// A four-trip loop with no closed form, through the LLVM order: its text, and
/// whether a loop phi is left.
fn four_trips_under(options: pipeline::Options) -> bool {
    let text = "define i16 @f(i16 %x) {\nb0:\n  br label %h\nh:\n  %i = phi i16 [ 0, %b0 ], [ %j, %h ]\n  %t = phi i16 [ %x, %b0 ], [ %u, %h ]\n  %m = mul i16 %t, 3\n  %u = add i16 %m, %i\n  %j = add i16 %i, 1\n  %c = icmp slt i16 %j, 4\n  br i1 %c, label %h, label %x\nx:\n  ret i16 %u\n}\n";
    let mut module = llrm_analysis::testing::parsed(text);
    let applied = Applied { options: pipeline::Options { llvm_order: true, ..options }, ..Applied::default() };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap_or_else(|error| panic!("{error}"));
    llrm_mir::print::module(&module).contains(" = phi ")
}

/// clang runs the full unroll with `OnlyWhenForced = !UnrollLoops`, and
/// `UnrollLoops` is on above -O1: matmul at -O1 was 379 bytes before the LLVM
/// order and 3197 after, 8x8x8 copied out where `opt -O1` keeps 408. At -Os the
/// option is on and the unroller's size threshold (0) keeps the loop; at -O2 a
/// four-trip loop is gone.
#[test]
fn test_the_full_unroll_takes_a_loop_only_below_o2_when_asked() {
    assert!(four_trips_under(pipeline::Options::basic()), "-O1 kept the loop");
    assert!(four_trips_under(pipeline::Options { loop_unrolling: false, ..pipeline::Options::standard() }));
    assert!(pipeline::Options::size().loop_unrolling);
    assert!(four_trips_under(pipeline::Options::size()), "-Os kept the loop");
    assert!(!four_trips_under(pipeline::Options::standard()), "-O2 unrolls it");
}

/// A loop of a runtime trip count over a `u16` counter, through the LLVM order:
/// whether its body is entered by a guard and leaves by its own back edge (a
/// rotated loop), as opposed to a header that tests first.
fn rotated_under(options: pipeline::Options) -> bool {
    let text = "define i32 @f(i16 %n, ptr %p) {\nb0:\n  br label %h\nh:\n  %i = phi i16 [ 0, %b0 ], [ %j, %b ]\n  %c = icmp ult i16 %i, %n\n  br i1 %c, label %b, label %x\nb:\n  %k = zext i16 %i to i32\n  %a = getelementptr i32, ptr %p, i32 %k\n  store i32 %k, ptr %a\n  %j = add i16 %i, 1\n  br label %h\nx:\n  ret i32 0\n}\n";
    let mut module = llrm_analysis::testing::parsed(text);
    let applied = Applied { options: pipeline::Options { llvm_order: true, ..options }, ..Applied::default() };
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program: &mut Program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .unwrap_or_else(|error| panic!("{error}"));
    let printed = llrm_mir::print::module(&module);
    // A block that branches back to itself on a condition.
    let mut block = "";
    printed
        .lines()
        .any(
            |line| {
                if let Some(name) = line.strip_suffix(':') {
                    block = name;
                    return false;
                }
                !block.is_empty() && line.trim_start().starts_with("br i1") && line.contains(&format!("label %{block}"))
            },
        )
}

/// matmul at -Os was 362 bytes before the LLVM order and 473 after: the loops
/// stayed unrotated (a `jmp` back to a top test, `and ebx,65535; cmp; jae` on
/// the `u16` counters) because the header copy was off at -Os. The pin copies
/// the header of every loop but at -Oz (`hasMinSize`; threshold 16).
#[test]
fn test_the_loop_header_is_copied_at_os_and_not_at_oz() {
    assert!(rotated_under(pipeline::Options::standard()), "-O2 rotates");
    assert!(rotated_under(pipeline::Options::size()), "-Os rotates");
    assert!(!rotated_under(pipeline::Options::min_size()), "-Oz copies no header");
}

/// A counted loop's exit test is `eq` after the pipeline, as the pin's
/// IndVarSimplify (LFTR) leaves it: only an `eq`/`ne` test can be taken
/// against zero by LSR. It stayed `ult` (bench ring: 6 instructions a trip in
/// the fill loop where the pin has 4).
#[test]
fn a_counted_loop_exits_on_an_equality_test() {
    let mut module = llrm_analysis::testing::parsed(&format!(
        "{}define void @f(ptr %buf) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b1 ]
  %p = getelementptr inbounds i16, ptr %buf, i16 %i
  store i16 %i, ptr %p
  %n = add nuw nsw i16 %i, 1
  %go = icmp ult i16 %n, 1024
  br i1 %go, label %b1, label %b2

b2:
  ret void
}}
",
        llrm_analysis::testing::DOS
    ));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    assert!(text.contains("icmp eq") || text.contains("icmp ne"), "{text}");
}

/// jpeg_single's `DECODE%` at -Os: LSR ran on a loop whose exit value reached
/// its reader through a phi that was not LCSSA, so the use counted as outside
/// the loop and got `-1` scales: a second counter stepping down by 4 beside
/// the one stepping up by 4, live across a call (+224 bytes). LSR requires
/// LoopSimplify, and the phis are cleaned by InstSimplify right before it, as
/// the pin's llc takes the IR after opt's closing InstSimplify.
#[test]
fn lsr_runs_on_simplified_loops_after_the_closing_instsimplify() {
    for options in [pipeline::Options::size(), pipeline::Options::basic(), pipeline::Options::default()] {
        let applied = Applied { options, ..Applied::default() };
        let names: Vec<&str> = pipeline::strength_slot(&applied).iter().map(|one| one.name()).collect();
        let at = names.iter().position(|one| *one == "lsr").expect("lsr is in the slot");
        assert_eq!(&names[at.saturating_sub(3)..at], ["loopsimplify", "instsimplify", "canonfreeze"], "{names:?}");
    }
}

/// x_strlen at -O2: the end of a scan was read through an exit phi, which
/// stops IVUsers where the pin's walk goes on, so LSR kept the pointer IV
/// (`lea eax,_buf-1`, a copy of it in the loop) where the pin has an index.
#[test]
fn a_scan_whose_end_is_read_after_the_loop_is_an_index_not_a_pointer() {
    let mut module = llrm_analysis::testing::parsed(&format!(
        "{}@buf = global [64 x i8] zeroinitializer
define i16 @f(i16 %k) {{
b0:
  br label %b1

b1:
  %p = phi ptr [ @buf, %b0 ], [ %n, %b1 ]
  %c = load i8, ptr %p
  %n = getelementptr inbounds i8, ptr %p, i16 1
  %z = icmp eq i8 %c, 0
  br i1 %z, label %b2, label %b1

b2:
  %a = ptrtoint ptr %p to i16
  %e = ptrtoint ptr @buf to i16
  %d = sub i16 %a, %e
  %r = add i16 %d, %k
  ret i16 %r
}}
",
        llrm_analysis::testing::DOS
    ));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        program.exports.entries.insert("f".to_owned());
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let text = llrm_mir::print::module(&module);
    assert!(!text.contains("phi ptr"), "{text}");
}

/// `Spares` and `Homes` are llrm-only (the pin has no counterpart for either)
/// and cost 16.4 M Ir at d_faces; the default pipeline ran both. They are off
/// at every level until `-fllrm-spares` / `-fllrm-homes`
/// (docs/working/homes-spares.md).
#[test]
fn the_llrm_only_module_passes_are_off_by_default_at_every_level() {
    for level in [
        pipeline::Options::none(),
        pipeline::Options::basic(),
        pipeline::Options::standard(),
        pipeline::Options::speed(),
        pipeline::Options::aggressive(),
        pipeline::Options::size(),
        pipeline::Options::min_size(),
    ] {
        assert!(!level.spare_loads && !level.stored_homes && !level.stable_loads);
    }
}

/// The premise of `cgpaddr`'s `optimizeExtUses` (DEVIATE CGP2 closed, #4421):
/// the pin deletes unreachable blocks as CodeGenPrepare starts
/// (`removeUnreachableBlocks`), and `cgpaddr` does not, because the pipeline
/// has done it long before: the first function pass of every level is
/// SimplifyCFG, which deletes a block nothing reaches. End to end, a C function
/// with a labelled block nothing jumps to has it at `--dump`'s first stage
/// (01-globalopt.ll) and not at the second (04-pipeline.ll), at -O1, -O2, -O3
/// and -Os; `-O0` runs no `cgpaddr`.
#[test]
fn test_simplifycfg_runs_first_so_no_unreachable_block_reaches_cgpaddr() {
    use crate::parity::Phase;
    for options in [
        pipeline::Options::basic(),
        pipeline::Options::standard(),
        pipeline::Options::speed(),
        pipeline::Options::size(),
    ] {
        let applied = Applied { options: pipeline::Options { llvm_order: true, ..options }, ..Applied::default() };
        let early: Vec<_> = pipeline::llvm_order(&applied, Phase::Early).iter().map(|one| one.name()).collect();
        let first = early.iter().find(|name| !name.starts_with("lowerexpect")).expect("a pass");
        assert!(first.starts_with("decide@") || first.starts_with("simplifycfg"), "{early:?}");
    }
}

/// A slot the -O1 order leaves out (`pre` off) must be in `ABSENT_AT_O1`, or
/// building the list fails the registry check: `mldst-motion@783` was gated on
/// `pre` without being listed, and every -O1 build panicked (the m64 driver's
/// `a_double_*` tests, `test_o1_states_the_ranges...`).
#[test]
fn test_the_o1_and_o2_simplify_lists_both_match_the_registry() {
    for options in [pipeline::Options::basic(), pipeline::Options::default()] {
        let applied = Applied { options, ..Applied::default() };
        // `linear` debug-asserts the registry check.
        let _ = pipeline::Fixed::linear(&applied, crate::parity::Phase::Simplify);
    }
}

use crate::testing::parsed;

const UNUSED_PARAMETER: &str = "define internal i16 @f(i16 %a, i16 %unused) noinline {
b0:
  %done = icmp sle i16 %a, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %a, 1
  %r = call i16 @f(i16 %m, i16 %unused)
  ret i16 %r
out:
  ret i16 %a
}

define i16 @top(i16 %x) {
b0:
  %r = call i16 @f(i16 %x, i16 12)
  ret i16 %r
}
";

/// The pipeline the compilers run over `text`, the `runs` lines of it.
fn pipeline_runs(
    text: &str,
    applied: &crate::pipeline::Applied,
) -> Vec<String> {
    let mut module = parsed(text);
    crate::pipeline::capturing_runs(|| {
        llrm_mir::program::Program::lend(
            &mut module,
            std::rc::Rc::new(llrm_x86_m16::Dos::default()),
            |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, applied),
        )
        .and_then(|done| done)
        .expect("the pipeline runs");
    })
    .1
}

/// The passes a body ran, per `LLRM_DEBUG=runs` line, for the lines of a run
/// that a stage of the interprocedural step (`ipa-...`) asked for.
fn passes_run_by_ipa_stages(lines: &[String]) -> Vec<(String, usize)> {
    lines
        .iter()
        .filter(|line| line.starts_with("body ") && line.contains("trigger \"ipa-"))
        .map(|line| {
            let runs = line.split(" pass runs").next().and_then(|head| head.rsplit(' ').next());
            let name = line.rsplit("name ").next().unwrap_or_default();
            (name.to_owned(), runs.and_then(|one| one.parse().ok()).expect("a pass-run count"))
        })
        .collect()
}

/// Dropping a parameter nothing reads ran the whole list again over the caller
/// and the callee (`ipa-deadargs.`: 48 passes each on this module), after they
/// had each run it once. LLVM runs deadargelim before the function pipelines
/// and runs a body's pipeline once; -O3 d_faces ran `_d_draw_faces` three times
/// (446, 293 and 287 M instructions).
#[test]
fn test_a_body_changed_by_an_interprocedural_stage_does_not_rerun_the_list() {
    let lines = pipeline_runs(UNUSED_PARAMETER, &crate::pipeline::Applied::default());
    let reruns = passes_run_by_ipa_stages(&lines);
    assert!(reruns.iter().all(|(_, runs)| *runs < 10), "the list ran again: {reruns:?}");
}

const TWO_CONSTANTS: &str = "define internal i16 @g(i16 %k, i16 %x) noinline {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %body ]
  %acc = phi i16 [ 0, %b0 ], [ %acc1, %body ]
  %go = icmp slt i16 %i, %k
  br i1 %go, label %body, label %done

body:
  %m = mul i16 %x, %i
  %acc1 = add i16 %acc, %m
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 %acc
}

define i16 @f(i16 %x) {
b0:
  %a = call i16 @g(i16 4, i16 %x)
  %b = call i16 @g(i16 5, i16 %x)
  %s = add i16 %a, %b
  ret i16 %s
}
";

/// -O3 cloned `g` for each constant after `f` and `g` had run the list, then
/// ran the list over each copy and `f` again (`ipa-cp.`, d_faces: 446, 293 and
/// 287 M instructions for one body). The copies are made before the list.
#[test]
fn test_a_body_cloned_for_a_constant_does_not_rerun_the_list() {
    let o3 = crate::pipeline::Applied {
        options: crate::pipeline::Options {
            inline: crate::inline::Threshold { cp_clone: true, ..crate::inline::Threshold::new(250) },
            ..Default::default()
        },
        ..Default::default()
    };
    let lines = pipeline_runs(TWO_CONSTANTS, &o3);
    let reruns = passes_run_by_ipa_stages(&lines);
    let bodies: Vec<&str> =
        lines.iter().filter(|line| line.starts_with("body ")).map(|line| &line[..50.min(line.len())]).collect();
    assert!(reruns.iter().all(|(_, runs)| *runs < 10), "the list ran again: {reruns:?} {bodies:?}");
}

/// `@eq` loops 40 times over a body of 60 operations: inlined in `@top`'s
/// 8-trip loop, it is too big to unroll.
fn loop_around_a_call() -> String {
    let body: String = (0..60)
        .map(|at| {
            let before = if at == 0 { "%acc".to_owned() } else { format!("%t{}", at - 1) };
            format!("  %t{at} = {} i16 {before}, {}\n", if at % 2 == 0 { "mul" } else { "xor" }, 3 + at)
        })
        .collect();
    format!(
        "define internal i16 @eq(i16 %x) {{
b0:
  br label %head

head:
  %j = phi i16 [ 0, %b0 ], [ %j1, %body ]
  %acc = phi i16 [ %x, %b0 ], [ %t59, %body ]
  %go = icmp slt i16 %j, 40
  br i1 %go, label %body, label %done

body:
{body}  %j1 = add nsw i16 %j, 1
  br label %head

done:
  ret i16 %acc
}}

define i16 @top(i16 %x) {{
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %body ]
  %sum = phi i16 [ 0, %b0 ], [ %sum1, %body ]
  %go = icmp slt i16 %i, 8
  br i1 %go, label %body, label %done

body:
  %a = add i16 %x, %i
  %r = call i16 @eq(i16 %a)
  %sum1 = add i16 %sum, %r
  %i1 = add nsw i16 %i, 1
  br label %head

done:
  ret i16 %sum
}}
"
    )
}

/// The 8-trip loop around a call was unrolled before the call was inlined: 8
/// calls, then 8 inlined copies (keybind's `_action_for`, 325 instructions
/// against LLVM's 55). The loop is simplified with the callee in it, too big
/// to unroll, and so stays a loop.
#[test]
fn test_a_loop_around_a_call_is_unrolled_only_after_the_call_is_inlined() {
    let mut module = parsed(&loop_around_a_call());
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| {
            crate::pipeline::applied(program, &crate::pipeline::Applied::default())
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    let top = text.split("define i16 @top").nth(1).expect("top is kept").split("\n}\n").next().unwrap_or_default();
    // The body has 30 multiplies: one copy, in a loop, and no call left.
    let multiplies = top.matches("mul i16").count();
    assert!(multiplies == 30 && !top.contains("@eq"), "top has {multiplies} multiplies:\n{top}");
}

/// fib was never copied into itself (LLVM refuses a recursive callee): 334157
/// clocks at -O2 against main's 227171, whose gcc-style recursion inlined it
/// eight levels deep. The walk copies it as gcc's `recursive_inlining` does,
/// to gcc's depth and size limits (DEVIATE recursion; tests/llc/fib.c is 802
/// bytes of text against clang's 100).
#[test]
fn test_a_directly_recursive_function_is_copied_into_itself_to_gcc_limits() {
    let mut module = parsed(
        "define i32 @fib(i32 %n) {
b0:
  %small = icmp slt i32 %n, 2
  br i1 %small, label %base, label %rec

base:
  ret i32 %n

rec:
  %a = sub i32 %n, 1
  %b = sub i32 %n, 2
  %x = call i32 @fib(i32 %a)
  %y = call i32 @fib(i32 %b)
  %s = add i32 %x, %y
  ret i32 %s
}

define i32 @main() {
b0:
  %r = call i32 @fib(i32 30)
  ret i32 %r
}
",
    );
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| {
            crate::pipeline::applied(program, &crate::pipeline::Applied::default())
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    let calls = text.matches("call i32 @fib").count();
    assert!(calls > 3, "fib was not copied into itself, {calls} calls of it:\n{text}");
    assert!(calls <= crate::inline::MAX_INLINE_INSNS_RECURSIVE as usize, "fib was copied past gcc's limit, {calls} calls of it");
}

/// A tail-recursive function called once: tail-call elimination makes its
/// loop, and the caller then inlines it (LLVM's CGSCC order, which the pin
/// follows). `recursive` and the call counts were taken before the walk, so
/// the callee stayed a recursive function with two calls and `insert` stayed a
/// call in `bintree` (+36% clocks at -O2).
#[test]
fn test_a_callee_made_a_loop_by_tail_call_elimination_is_inlined_into_its_one_caller() {
    let padding: String = (0..70)
        .map(|at| {
            let before = if at == 0 { "%node".to_owned() } else { format!("%t{}", at - 1) };
            format!("  %t{at} = {} i32 {before}, {}\n", if at % 2 == 0 { "mul" } else { "xor" }, 3 + at)
        })
        .collect();
    let mut module = parsed(
        &"define internal void @ins(ptr %t, i32 %at, i32 %node) {
b1:
  %p = getelementptr inbounds i32, ptr %t, i32 %at
  %v = load i32, ptr %p
  %z = icmp eq i32 %v, 0
  br i1 %z, label %set, label %more

set:
PADDING  store i32 %t69, ptr %p
  br label %done

more:
  call void @ins(ptr %t, i32 %v, i32 %node)
  br label %done

done:
  ret void
}

define i32 @top(ptr %t, i32 %n) {
b1:
  br label %head

head:
  %i = phi i32 [ 2, %b1 ], [ %next, %body ]
  %c = icmp sle i32 %i, %n
  br i1 %c, label %body, label %exit

body:
  call void @ins(ptr %t, i32 1, i32 %i)
  %next = add nsw i32 %i, 1
  br label %head

exit:
  %r = load i32, ptr %t
  ret i32 %r
}
"
        .replace("PADDING", &padding),
    );
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| {
            crate::pipeline::applied(program, &crate::pipeline::Applied::default())
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let top = llrm_mir::print::module(&module);
    let top = top.split("define").find(|one| one.contains("@top")).expect("@top");
    assert!(!top.contains("@ins("), "{top}");
}

/// An internal function inlined at its one call is deleted by the walk, as
/// the inliner's `removeDeadFunctions` does; it stood until globaldce, and
/// the walk priced and simplified it (d_faces -O2: 11 defines after the walk,
/// the pin's 6).
#[test]
fn test_a_callee_inlined_at_its_only_call_is_deleted_by_the_walk() {
    use llrm_mir::program::ProgramPass;
    let mut module = parsed(
        "define internal i32 @leaf(i32 %x) {
b0:
  %y = add i32 %x, 1
  ret i32 %y
}

define internal i32 @mid(i32 %x) {
b0:
  %y = call i32 @leaf(i32 %x)
  ret i32 %y
}

define i32 @top(i32 %x) {
b0:
  %y = call i32 @mid(i32 %x)
  ret i32 %y
}
",
    );
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| {
            let mut walk = crate::walk::Walk { inline: Default::default(), simplify: Box::new(|_, _, _, _| {}) };
            walk.run(program, &mut Default::default())
        },
    )
    .and_then(|done| done)
    .expect("the walk runs");
    let text = llrm_mir::print::module(&module);
    for name in ["@leaf", "@mid"] {
        let body = text.split("define").find(|one| one.contains(&format!("{name}("))).expect(name);
        assert!(!body.contains("add") && !body.contains("call"), "{name} stands:\n{text}");
    }
}

/// Two calls of a function that reads and writes nothing, with the same
/// arguments, are one call. hanoi's merge in the simplifier makes it a loop
/// (188 clocks); copies made before it (two calls in the body) left it
/// 185482.
#[test]
fn test_calls_the_simplifier_merges_are_not_two_recursive_calls_to_copy() {
    let mut module = parsed(
        "define internal i32 @hanoi(i32 %n) memory(none) {
b0:
  %zero = icmp eq i32 %n, 0
  br i1 %zero, label %out, label %rec

rec:
  %m = sub i32 %n, 1
  %x = call i32 @hanoi(i32 %m)
  %p = add i32 %x, 1
  %y = call i32 @hanoi(i32 %m)
  %s = add i32 %p, %y
  ret i32 %s

out:
  ret i32 0
}

define i32 @top(i32 %n) {
b0:
  %r = call i32 @hanoi(i32 %n)
  ret i32 %r
}
",
    );
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| {
            crate::pipeline::applied(program, &crate::pipeline::Applied::default())
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    let calls = text.matches("call i32 @hanoi").count();
    assert!(calls <= 1, "hanoi was copied before its two calls were merged, {calls} calls of it:\n{text}");
}

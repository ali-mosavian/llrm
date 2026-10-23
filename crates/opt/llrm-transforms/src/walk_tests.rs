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
    assert!(
        calls <= crate::inline::MAX_INLINE_INSNS_RECURSIVE as usize,
        "fib was copied past gcc's limit, {calls} calls of it"
    );
}

/// The Simplify list runs GVN before its jump threading, and the copies of a
/// recursion in itself join their paths only in the threading: run once, each
/// copy repeated the calls of the ones that dominate it (fib -O2, i486 rows:
/// 154398 clocks once, 14802 with the list run again). A body with copies runs
/// the list twice, one without once.
#[test]
fn test_a_recursion_with_copies_runs_the_simplify_list_twice() {
    use llrm_mir::program::ProgramPass;
    let runs = |text: &str| {
        let mut module = parsed(text);
        let seen = std::rc::Rc::new(std::cell::Cell::new(0));
        let counted = std::rc::Rc::clone(&seen);
        llrm_mir::program::Program::lend(
            &mut module,
            std::rc::Rc::new(llrm_x86_m16::Dos::default()),
            |program: &mut llrm_mir::program::Program| {
                let mut analyses = Default::default();
                let mut walk = crate::walk::Walk {
                    inline: crate::inline::Threshold { recursive: true, ..Default::default() },
                    simplify: Box::new(|_, _, _, _| {}),
                };
                walk.run(program, &mut analyses)?;
                let mut modules = super::managers(program, &mut analyses);
                let mut late = crate::walk::SelfInline {
                    simplify: Box::new(move |_, _, _, _| counted.set(counted.get() + 1)),
                    for_size: false,
                };
                late.apply(program, &mut modules)
            },
        )
        .and_then(|done| done)
        .expect("the late step runs");
        seen.get()
    };
    let fib = "define internal i32 @fib(i32 %n) {
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

define i32 @top(i32 %n) {
b0:
  %r = call i32 @fib(i32 %n)
  ret i32 %r
}
";
    assert_eq!(runs(fib), 2, "fib has copies");
    assert_eq!(runs(LINEAR), 0, "a count-down waits for nothing");
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
            let mut walk = crate::walk::Walk {
                inline: crate::inline::Threshold { recursive: true, ..Default::default() },
                simplify: Box::new(|_, _, _, _| {}),
            };
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
/// (188 clocks); copies made before it (two calls in the body) peeled it
/// four levels deep (185482 clocks).
#[test]
fn test_calls_the_simplifier_merges_are_not_two_recursive_calls_to_copy() {
    let mut module = parsed(
        "define internal i32 @hanoi(i32 %n) {
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
    let hanoi = text.split("define").find(|one| one.contains("@hanoi(i32 %n)")).expect("@hanoi");
    let peeled = hanoi.matches("icmp eq i32 %n,").count();
    assert_eq!(peeled, 1, "hanoi was copied into itself before its two calls were merged, {peeled} levels:\n{hanoi}");
}

/// gcc does not inline recursively at -Os. fib was copied into itself there
/// as at -O2 (17 calls of it standing).
#[test]
fn test_a_recursive_function_is_not_copied_into_itself_when_sized_for_bytes() {
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
    let applied = crate::pipeline::Applied { options: crate::pipeline::Options::size(), ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    let calls = text.matches("call i32 @fib").count();
    assert!(calls <= 3, "fib was copied into itself at -Os, {calls} calls of it:\n{text}");
}

/// Two calls of itself in exclusive branches (recpow: `b * power(b, e - 1)`
/// when the exponent is odd, `power(b * b, e >> 1)` when even) are less than
/// one call per call: a linear recursion, which is a loop. Copied, it was
/// peeled three levels: +3.7% clocks at -O2 against the base.
#[test]
fn test_two_calls_in_exclusive_branches_are_not_a_tree_to_copy() {
    let mut module = parsed(
        "define internal i32 @power(i32 %b, i32 %e) {
b0:
  %zero = icmp eq i32 %e, 0
  br i1 %zero, label %one, label %more

one:
  ret i32 1

more:
  %bit = and i32 %e, 1
  %odd = icmp ne i32 %bit, 0
  br i1 %odd, label %mul, label %square

mul:
  %e1 = sub i32 %e, 1
  %p = call i32 @power(i32 %b, i32 %e1)
  %m = mul i32 %b, %p
  %r = urem i32 %m, 30011
  ret i32 %r

square:
  %bb = mul i32 %b, %b
  %sq = urem i32 %bb, 30011
  %h = lshr i32 %e, 1
  %q = call i32 @power(i32 %sq, i32 %h)
  ret i32 %q
}

define i32 @top(i32 %b, i32 %e) {
b0:
  %r = call i32 @power(i32 %b, i32 %e)
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
    let power = text.split("define").find(|one| one.contains("@power(")).expect("@power");
    let peeled = power.matches("urem i32").count();
    assert!(peeled <= 2, "power was copied into itself, {peeled} remainders:\n{power}");
}

const CHOP: &str = "@values = internal global [64 x i32] zeroinitializer

define internal i32 @chop(i32 %lo, i32 %hi) {
b0:
  %d = sub i32 %hi, %lo
  %small = icmp slt i32 %d, 2
  br i1 %small, label %out, label %rec

out:
  ret i32 0

rec:
  %s = add i32 %lo, %hi
  %mid = sdiv i32 %s, 2
  br label %head

head:
  %i = phi i32 [ %lo, %rec ], [ %next, %body ]
  %sum = phi i32 [ 0, %rec ], [ %sum2, %body ]
  %c = icmp slt i32 %i, %hi
  br i1 %c, label %body, label %done

body:
  %p = getelementptr inbounds i32, ptr @values, i32 %i
  %v = load i32, ptr %p
  %m = and i32 %v, %mid
  %sum2 = add i32 %sum, %m
  %next = add nsw i32 %i, 1
  br label %head

done:
  %a = call i32 @chop(i32 %lo, i32 %mid)
  %b = call i32 @chop(i32 %mid, i32 %hi)
  %t = add i32 %sum, %a
  %r = add i32 %t, %b
  ret i32 %r
}

define i32 @top() {
b0:
  %r = call i32 @chop(i32 0, i32 64)
  ret i32 %r
}
";

/// A tree recursion waits for its tail-call elimination until its copies are
/// made, and a linear one does not: the mark is on `fib`'s shape after the
/// walk and not on a tail-recursive count-down.
#[test]
fn test_only_a_tree_recursion_waits_for_its_tail_call_elimination() {
    use llrm_mir::program::ProgramPass;
    let marked_with = |text: &str, name: &str, recursive: bool| {
        let mut module = parsed(text);
        llrm_mir::program::Program::lend(
            &mut module,
            std::rc::Rc::new(llrm_x86_m16::Dos::default()),
            |program: &mut llrm_mir::program::Program| {
                let mut walk = crate::walk::Walk {
                    inline: crate::inline::Threshold { recursive, ..Default::default() },
                    simplify: Box::new(|_, _, _, _| {}),
                };
                walk.run(program, &mut Default::default())
            },
        )
        .and_then(|done| done)
        .expect("the walk runs");
        let text = llrm_mir::print::module(&module);
        let body = text.split("define").find(|one| one.contains(&format!("@{name}("))).expect(name).to_owned();
        body.lines().next().unwrap_or_default().contains(crate::tailcallelim::DEFERRED)
    };
    let tree = "define internal i32 @fib(i32 %n) {
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

define i32 @top(i32 %n) {
b0:
  %r = call i32 @fib(i32 %n)
  ret i32 %r
}
";
    let linear = "define internal i32 @down(i32 %n, i32 %acc) {
b0:
  %done = icmp eq i32 %n, 0
  br i1 %done, label %out, label %again

out:
  ret i32 %acc

again:
  %m = sub i32 %n, 1
  %a = add i32 %acc, %n
  %r = call i32 @down(i32 %m, i32 %a)
  ret i32 %r
}

define i32 @top(i32 %n) {
b0:
  %r = call i32 @down(i32 %n, i32 0)
  ret i32 %r
}
";
    let marked = |text: &str, name: &str| marked_with(text, name, true);
    assert!(marked(tree, "fib"), "fib waits for tail-call elimination");
    assert!(!marked(linear, "down"), "a count-down does not");
    assert!(!marked_with(tree, "fib", false), "by default nothing waits: the copies are `-finline-recursive`'s");
}

/// ipa-cp prices the callee whole: copies made in the walk first left
/// recchop -O3 with no clone (4 defines after the interprocedural step against
/// the base's 12) and 6368 clocks against 6209.
#[test]
fn test_ipa_cp_clones_a_tree_recursion_before_it_is_copied_into_itself() {
    let mut module = parsed(CHOP);
    let options = crate::pipeline::Options::speed();
    let applied = crate::pipeline::Applied { options, ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    assert!(text.contains("chop.constprop"), "no clone of chop for its constants:\n{text}");
    assert!(!text.contains(crate::tailcallelim::DEFERRED), "a function still waits:\n{text}");
}

/// The module `text` after the walk alone (no simplifier), `top`'s body.
fn walked_top(text: &str) -> String {
    walked_top_with(text, true)
}

/// `walked_top` with gcc's recursive inlining (`-finline-recursive`) on or off.
fn walked_top_with(
    text: &str,
    recursive: bool,
) -> String {
    use llrm_mir::program::ProgramPass;
    let mut module = parsed(text);
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| {
            let mut walk = crate::walk::Walk {
                inline: crate::inline::Threshold { recursive, ..Default::default() },
                simplify: Box::new(|_, _, _, _| {}),
            };
            walk.run(program, &mut Default::default())
        },
    )
    .and_then(|done| done)
    .expect("the walk runs");
    let text = llrm_mir::print::module(&module);
    text.split("define").find(|one| one.contains("@top(")).expect("@top").to_owned()
}

const LINEAR: &str = "define internal i32 @down(i32 %n) {
b0:
  %done = icmp eq i32 %n, 0
  br i1 %done, label %out, label %again

out:
  ret i32 0

again:
  %m = sub i32 %n, 1
  %r = call i32 @down(i32 %m)
  %s = mul i32 %r, 7919
  ret i32 %s
}

define i32 @top(i32 %n) {
b0:
  %r = call i32 @down(i32 %n)
  ret i32 %r
}
";

/// A recursive callee inlined into a caller has its own call inlined in the
/// copy, to gcc's peeling depth: the probability of reaching the copy (the
/// calls to itself per call, 0.5 here, to the power of the copies deep) below
/// 1 - 1/8, squared per copy only where the body has more than one call
/// (their copies grow as sites to the power of the depth; one call grows them
/// by one a level). Eight copies after the one the cost allowed (hanoi: 180
/// clocks with one copy, 163 with three, 133 with six squared, 103 now).
#[test]
fn test_a_linear_recursive_callee_is_peeled_in_its_caller() {
    let top = walked_top(LINEAR);
    let copies = top.matches("mul i32 %").count();
    assert_eq!(copies, 9, "{copies} copies of down in top:\n{top}");
}

/// The default is the pin's inliner: a recursive callee is copied once into its
/// caller and the copy's own calls are left (hanoi's nine copies and its lookup
/// table, 40 instructions where gcc runs 35,764 and clang 249,852, are
/// `-finline-recursive`, a DEVIATE (recursion) the pin has no counterpart of).
#[test]
fn test_the_default_does_not_peel_a_recursive_callee() {
    let copies = walked_top_with(LINEAR, false).matches("mul i32 %").count();
    assert_eq!(copies, 1, "{copies} copies of down in top by default");
}

/// A tree is not peeled: the copy doubles at each level (bintree -O2 +0.13%
/// clocks, 988 to 1276 bytes, with the peeling of an edge at a time).
#[test]
fn test_a_tree_recursive_callee_is_inlined_once_in_its_caller() {
    let top = walked_top(
        "define internal i32 @down(i32 %n) {
b0:
  %done = icmp slt i32 %n, 2
  br i1 %done, label %out, label %again

out:
  ret i32 %n

again:
  %a = sub i32 %n, 1
  %b = sub i32 %n, 2
  %x = call i32 @down(i32 %a)
  %y = call i32 @down(i32 %b)
  %s = mul i32 %x, 7919
  %t = add i32 %s, %y
  ret i32 %t
}

define i32 @top(i32 %n) {
b0:
  %r = call i32 @down(i32 %n)
  ret i32 %r
}
",
    );
    let copies = top.matches("mul i32 %x").count();
    assert_eq!(copies, 1, "{copies} copies of down in top:\n{top}");
}

/// What the copies a call peels add is capped at gcc's recursive growth
/// limit: two calls in exclusive branches (recpow's shape) double the copies
/// at each level, 32 copies and 8203 bytes at -O1 before the cap, 20 after.
#[test]
fn test_the_copies_a_call_peels_add_at_most_a_functions_growth() {
    let top = walked_top(
        "define internal i32 @down(i32 %n) {
b0:
  %done = icmp eq i32 %n, 0
  br i1 %done, label %out, label %more

out:
  ret i32 1

more:
  %bit = and i32 %n, 1
  %odd = icmp ne i32 %bit, 0
  br i1 %odd, label %odd.arm, label %even.arm

odd.arm:
  %a = sub i32 %n, 1
  %x = call i32 @down(i32 %a)
  %r = urem i32 %x, 30011
  ret i32 %r

even.arm:
  %h = lshr i32 %n, 1
  %y = call i32 @down(i32 %h)
  %q = urem i32 %y, 30013
  ret i32 %q
}

define i32 @top(i32 %n) {
b0:
  %r = call i32 @down(i32 %n)
  ret i32 %r
}
",
    );
    let copies = top.matches("urem i32 %x").count();
    assert!(copies <= 24, "{copies} copies of down in top:\n{top}");
}

/// A callee that loops on two values read through pointers: `parity(count,
/// seed)` as `loop.bas` has it, public so that nothing deletes it.
const BY_REFERENCE: &str = "define i32 @parity(ptr %count, ptr %seed) {
b1:
  %s0 = load i16, ptr %seed
  %seed.w = sext i16 %s0 to i32
  br label %head

head:
  %total = phi i32 [ %seed.w, %b1 ], [ %next, %body ]
  %i = phi i16 [ 0, %b1 ], [ %i.next, %body ]
  %n = load i16, ptr %count
  %go = icmp slt i16 %i, %n
  br i1 %go, label %body, label %out

body:
  %i.w = sext i16 %i to i32
  %m = mul i32 %i.w, %seed.w
  %t = add i32 %total, %m
  %next = add i32 %t, 3
  %i.next = add i16 %i, 1
  br label %head

out:
  ret i32 %total
}
";

/// `@top` after the pipeline sized for bytes over `parity` and a caller that
/// stores `count` and `seed` to stack slots (`first` and `second`: the values
/// it stores) and calls it twice.
fn sized_top(
    count: i16,
    seed: i16,
    constant: bool,
) -> String {
    let stores = if constant {
        format!(
            "  store i16 {count}, ptr %a\n  store i16 {seed}, ptr %b\n  store i16 {seed}, ptr %c\n  store i16 {count}, ptr %d\n"
        )
    } else {
        "  store i16 %x, ptr %a\n  store i16 %y, ptr %b\n  store i16 %y, ptr %c\n  store i16 %x, ptr %d\n".to_owned()
    };
    let mut module = parsed(&format!(
        "{BY_REFERENCE}
define i32 @top(i16 %x, i16 %y) {{
b0:
  %a = alloca i16
  %b = alloca i16
  %c = alloca i16
  %d = alloca i16
{stores}  %p = call i32 @parity(ptr %a, ptr %b)
  %q = call i32 @parity(ptr %d, ptr %c)
  %r = add i32 %p, %q
  ret i32 %r
}}
"
    ));
    let applied = crate::pipeline::Applied { options: crate::pipeline::Options::size(), ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    text.split("define").find(|one| one.contains("@top(")).expect("@top").to_owned()
}

/// `parity(7, 5)` is a constant once the call is inlined (the loop runs seven
/// times on known values) and the call to it costs more bytes than the
/// constant; the cost model sees the loop and two loads through pointers, 40
/// against the -Os bound of 25, so the call stayed (`loop.bas`, 247 bytes
/// against 159).
#[test]
fn test_an_os_call_whose_stack_arguments_make_the_loop_a_constant_is_inlined() {
    let top = sized_top(7, 5, true);
    assert!(!top.contains("call"), "the call to parity stayed:\n{top}");
}

/// The same call with values the caller does not know folds nothing: the
/// trial finds the copy no smaller, and the call stays.
#[test]
fn test_an_os_call_whose_stack_arguments_are_unknown_stays_a_call() {
    let top = sized_top(7, 5, false);
    assert!(top.contains("call i32 @parity"), "parity was inlined for nothing:\n{top}");
}

/// LLRM_CHECK_CALLEES said `the declarations held by the module's analyses
/// differ from a fresh listing` of 26 programs: what `post_order` stated of a
/// callee (`memory(argmem: write)`, `writeonly`) was reported edited, but the
/// held `Declarations` was not looked at again, so the next body's callers
/// priced the callee by the attributes it had before.
#[test]
fn a_callee_the_walk_states_attributes_of_is_held_with_them_for_the_next_body() {
    use llrm_mir::program::ProgramPass;
    let mut module = parsed(
        "define internal void @w(ptr %p) {
b0:
  store i16 1, ptr %p
  ret void
}

define void @c(ptr %p) {
b0:
  call void @w(ptr %p)
  ret void
}
",
    );
    let stale = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = std::rc::Rc::clone(&stale);
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        move |program: &mut llrm_mir::program::Program| {
            let seen = std::rc::Rc::clone(&seen);
            let mut walk = crate::walk::Walk {
                inline: crate::inline::Threshold { limit: 0, ..Default::default() },
                simplify: Box::new(move |module, analyses, _, _| {
                    let held = analyses.get::<llrm_mir::passes::Declarations>(module);
                    if *held != module.declarations() {
                        seen.set(true);
                    }
                }),
            };
            walk.run(program, &mut Default::default())
        },
    )
    .and_then(|done| done)
    .expect("the walk runs");
    assert!(!stale.get(), "the held declarations differ from a fresh listing");
}

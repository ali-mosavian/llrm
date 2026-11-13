use crate::testing::parsed;

/// A recursive function reading a slice descriptor's length: argument
/// promotion takes the length in the descriptor's place (`promote.`), then the
/// descriptor's unused data pointer goes (`ipa-deadargs.`).
const DESCRIPTOR: &str =
    "define internal i16 @sum(ptr addrspace(1) noalias readonly dereferenceable(6) nocapture %d, i16 %n) noinline {
b0:
  %at = getelementptr i8, ptr addrspace(1) %d, i16 4
  %len = load i16, ptr addrspace(1) %at
  %done = icmp sle i16 %n, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %n, 1
  %r = call i16 @sum(ptr addrspace(1) %d, i16 %m)
  %s = add i16 %r, %len
  ret i16 %s
out:
  ret i16 %len
}

define i16 @top(ptr addrspace(1) %p) {
b0:
  %r = call i16 @sum(ptr addrspace(1) %p, i16 3)
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
            std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
            |program: &mut llrm_mir::program::Program| {
                // Without an entry nothing is reachable, and no stage of the
                // interprocedural step has a body to rerun.
                program.exports.entries.insert("top".to_owned());
                crate::pipeline::applied(program, applied)
            },
        )
        .and_then(|done| done)
        .expect("the pipeline runs");
    })
    .1
}

/// The passes a body ran, per `LLRM_DEBUG=runs` line, for the lines of a run
/// that a stage of the interprocedural step (`ipa-...`, `ipsccp.`) asked for.
/// `promote.` keeps the full list on purpose (the minimal one left Nib
/// quicksort's far sort arguments unfolded; `recorded`).
fn passes_run_by_ipa_stages(lines: &[String]) -> Vec<(String, usize)> {
    lines
        .iter()
        .filter(|line| {
            line.starts_with("body ") && ["trigger \"ipa-", "trigger \"ipsccp."].iter().any(|one| line.contains(one))
        })
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
    let lines = pipeline_runs(
        &format!("{}{DESCRIPTOR}", llrm_analysis::testing::DOS),
        &crate::pipeline::Applied { options: crate::pipeline::Options::speed(), ..Default::default() },
    );
    let reruns = passes_run_by_ipa_stages(&lines);
    assert!(reruns.iter().all(|(_, runs)| *runs < 10), "the list ran again: {reruns:?}");
}

/// The late list (loop-load-elim, slp, the late unroll) ran before the
/// interprocedural stages, which then ran the simplification list again over
/// the bodies it had grown: `main` of memory_expansion entered GVN at 6051,
/// 6052 and 6058 instructions, and the whole module summed 30,988 at GVN
/// where the pin's 8,332. The late list is a body's last run.
#[test]
fn test_the_late_list_is_a_bodys_last_run() {
    let lines = pipeline_runs(
        &format!("{}{DESCRIPTOR}", llrm_analysis::testing::DOS),
        &crate::pipeline::Applied { options: crate::pipeline::Options::speed(), ..Default::default() },
    );
    // A run's steps come before its `body` line; the late list has the
    // late unroll's slot.
    let mut late: Vec<String> = Vec::new();
    let mut steps_late = false;
    let mut after_late: Vec<String> = Vec::new();
    for line in &lines {
        if line.starts_with("step ") {
            steps_late |= line.contains("unroll@1476");
        } else if line.starts_with("body ") {
            let name = line.rsplit("name ").next().unwrap_or_default().to_owned();
            if late.contains(&name) {
                after_late.push(name.clone());
            }
            if steps_late {
                late.push(name);
            }
            steps_late = false;
        }
    }
    assert!(!late.is_empty(), "no body ran the late list: {lines:?}");
    assert!(after_late.is_empty(), "bodies run again after the late list: {after_late:?}");
}

/// `check_locals` calls the two functions it is handed through its own
/// stack slots: inlined into `main` the calls become direct only after the
/// simplification, so a second round inlines them (`DevirtSCCRepeatedPass`).
const APPLY_TWO: &str = "@x = internal global [700 x i8] zeroinitializer
@y = internal global [700 x i8] zeroinitializer
@src = internal global [700 x i8] zeroinitializer
declare i16 @memcmp(ptr, ptr, i16)

define internal void @lc(ptr %d, ptr %s) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %sp = getelementptr inbounds i8, ptr %s, i16 %i
  %v = load i8, ptr %sp
  %dp = getelementptr inbounds i8, ptr %d, i16 %i
  store i8 %v, ptr %dp
  %i1 = add nuw nsw i16 %i, 1
  %c = icmp slt i16 %i1, 700
  br i1 %c, label %loop, label %done
done:
  ret void
}

define internal void @lr(ptr %d, ptr %s) {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %i1, %loop ]
  %sp = getelementptr inbounds i8, ptr %s, i16 %i
  %v = load i8, ptr %sp
  %dp = getelementptr inbounds i8, ptr %d, i16 %i
  store i8 %v, ptr %dp
  %i1 = add nuw nsw i16 %i, 1
  %c = icmp slt i16 %i1, 700
  br i1 %c, label %loop, label %done
done:
  ret void
}

define internal i16 @differ(ptr %a, ptr %b, i16 %n) noinline {
  %c = call i16 @memcmp(ptr %a, ptr %b, i16 %n)
  ret i16 %c
}

define internal i16 @check_locals(ptr %f, ptr %g) {
  %f.addr = alloca ptr
  %g.addr = alloca ptr
  store ptr %f, ptr %f.addr
  store ptr %g, ptr %g.addr
  %f1 = load ptr, ptr %f.addr
  call void %f1(ptr @x, ptr @src)
  %g1 = load ptr, ptr %g.addr
  call void %g1(ptr @y, ptr @src)
  %d = call i16 @differ(ptr @x, ptr @y, i16 700)
  ret i16 %d
}

define i16 @top() {
  %r = call i16 @check_locals(ptr @lc, ptr @lr)
  ret i16 %r
}
";

/// A body took the inliner's calls and then the function passes twice when the
/// callee's calls became direct only after the simplification: `main` of
/// memory_expansion ran the list twice where the pin's inliner worklist takes
/// the new calls before the function passes run once.
#[test]
fn test_calls_that_a_splice_exposes_are_inlined_before_the_list_runs() {
    let lines = pipeline_runs(
        &format!("{}{APPLY_TWO}", llrm_analysis::testing::DOS),
        &crate::pipeline::Applied { options: crate::pipeline::Options::speed(), ..Default::default() },
    );
    let walks = |name: &str| {
        lines
            .iter()
            .filter(|line| {
                line.starts_with("body ")
                    && line.contains("trigger \"walk.\"")
                    && line.ends_with(&format!("name {name}"))
            })
            .count()
    };
    assert_eq!(walks("top"), 1, "the list ran again over top: {lines:?}");
}

/// A promotion edited the caller's call site after the caller had run the
/// list, and `promote.` ran the list over it again (`main` of
/// memory_expansion: three runs, 833M instructions in the last). The pin runs
/// ArgumentPromotion per component before its callers are visited.
#[test]
fn test_a_caller_of_a_promoted_function_runs_the_list_once() {
    let lines = pipeline_runs(
        &format!("{}{DESCRIPTOR}", llrm_analysis::testing::DOS),
        &crate::pipeline::Applied { options: crate::pipeline::Options::speed(), ..Default::default() },
    );
    let again: Vec<&String> = lines
        .iter()
        .filter(|line| line.starts_with("body ") && line.contains("trigger \"promote.\"") && line.ends_with("name top"))
        .collect();
    assert!(again.is_empty(), "the caller ran the list again after the promotion: {again:?}");
}

/// The mark that tells the module-wide step the walk promoted a body is read
/// and removed there: it must not reach the module a later stage prints.
#[test]
fn test_the_walks_promotion_mark_does_not_reach_the_printed_module() {
    let mut module = parsed(&format!("{}{DESCRIPTOR}", llrm_analysis::testing::DOS));
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| {
            program.exports.entries.insert("top".to_owned());
            crate::pipeline::applied(
                program,
                &crate::pipeline::Applied { options: crate::pipeline::Options::speed(), ..Default::default() },
            )
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = crate::testing::printed(&module);
    assert!(!text.contains("llrm-promoted-in-walk"), "the mark stayed on a body:\n{text}");
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
            inline: crate::inline::Threshold { cp_clone: Some(true), ..crate::inline::Threshold::new(250) },
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| {
            let mut options = crate::pipeline::Options::default();
            options.inline.recursive = Some(true);
            crate::pipeline::applied(program, &crate::pipeline::Applied { options, ..Default::default() })
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
            std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
            |program: &mut llrm_mir::program::Program| {
                let mut analyses = Default::default();
                let mut walk = crate::walk::Walk {
                    promote_rate: None,
                    rerun_unchanged: true,
                    speed_rate: true,
                    inline: crate::inline::Threshold { recursive: Some(true), ..Default::default() },
                    simplify: Box::new(|_, _, _, _| {}),
                };
                walk.run(program, &mut analyses)?;
                let mut modules = super::managers(program, &mut analyses);
                let mut late = crate::walk::SelfInline {
                    speed_rate: true,
                    simplify: Box::new(move |_, _, _, _| counted.set(counted.get() + 1)),
                    for_size: false,
                    flag: Some(true),
                    o3: false,
                    inlining: true,
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

/// A caller that took a tree recursion's peeled copies is a body with
/// self-copies: the equality fold of a multi-use difference is for it too. It
/// was left out, and recsum -O1 kept the chain `n-1`, `n-2`, ... (m16 +288 B,
/// m32 +203 B).
#[test]
fn test_a_caller_of_a_peeled_recursion_is_a_body_with_self_copies() {
    use llrm_mir::program::ProgramPass;
    let flagged = |name: &str| {
        let mut module = parsed(FIB_TOP);
        llrm_mir::program::Program::lend(
            &mut module,
            std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
            |program: &mut llrm_mir::program::Program| {
                let mut analyses = Default::default();
                let mut walk = crate::walk::Walk {
                    promote_rate: None,
                    rerun_unchanged: true,
                    speed_rate: true,
                    inline: crate::inline::Threshold { recursive: Some(true), ..Default::default() },
                    simplify: Box::new(|_, _, _, _| {}),
                };
                walk.run(program, &mut analyses).expect("the walk runs");
                program
                    .modules[0]
                    .function_mut(name)
                    .expect("the function")
                    .1
                    .attrs
                    .iter()
                    .any(
                        |attr| matches!(
                            attr,
                            llrm_mir::opcode::Attribute::Flag(flag) if flag == crate::walk::SELF_COPIED
                        ),
                    )
            },
        )
        .expect("lent")
    };
    assert!(flagged("top"), "top took fib's peeled copies");
}

const FIB_TOP: &str = "define internal i32 @fib(i32 %n) {
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| {
            let mut walk = crate::walk::Walk {
                promote_rate: None,
                rerun_unchanged: true,
                speed_rate: true,
                inline: crate::inline::Threshold { recursive: Some(true), ..Default::default() },
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
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

// Not internal: an internal table nothing stores to is constant, and its zeros
// fold the loads away.
const CHOP: &str = "@values = global [64 x i32] zeroinitializer

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
            std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
            |program: &mut llrm_mir::program::Program| {
                let mut walk = crate::walk::Walk {
                    promote_rate: None,
                    rerun_unchanged: true,
                    speed_rate: true,
                    inline: crate::inline::Threshold { recursive: recursive.then_some(true), ..Default::default() },
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

/// The pin refuses a callee that calls itself (`IsRecursiveCall`, no
/// `AllowRecursiveCall`): `report(fib(30))` keeps its one call of `fib`. The
/// walk inlined it by default, and the unrolled loop it brought left 15 calls
/// of `fib` in `main`.
#[test]
fn test_a_callee_that_calls_itself_is_not_inlined_unless_recursion_is_on() {
    use llrm_mir::program::ProgramPass;
    let calls_of_fib_in_main = |recursive: bool| {
        let mut module = parsed(
            "define internal i32 @fib(i32 %n) {
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
            std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
            |program: &mut llrm_mir::program::Program| {
                let mut walk = crate::walk::Walk {
                    promote_rate: None,
                    rerun_unchanged: true,
                    speed_rate: true,
                    inline: crate::inline::Threshold { recursive: recursive.then_some(true), ..Default::default() },
                    simplify: Box::new(|_, _, _, _| {}),
                };
                walk.run(program, &mut Default::default())
            },
        )
        .and_then(|done| done)
        .expect("the walk runs");
        let text = llrm_mir::print::module(&module);
        let main = text.split("define").find(|one| one.contains("@main(")).expect("main").to_owned();
        main.matches("call i32 @fib(").count()
    };
    assert_eq!(calls_of_fib_in_main(false), 1, "the pin leaves the call");
    assert_ne!(calls_of_fib_in_main(true), 1, "-finline-recursive inlines it");
}

/// ipa-cp prices the callee whole: copies made in the walk first left
/// recchop -O3 with no clone (4 defines after the interprocedural step against
/// the base's 12) and 6368 clocks against 6209.
#[test]
fn test_ipa_cp_clones_a_tree_recursion_before_it_is_copied_into_itself() {
    let mut module = parsed(CHOP);
    let options = crate::pipeline::Options::aggressive();
    let applied = crate::pipeline::Applied { options, ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    // The chain of clones is whole before the walk, which inlines each into the
    // one before it: no clone is left to name, and no call passes the whole
    // range.
    assert!(!text.contains("@chop(i32 0, i32 64)"), "chop was not cloned for its constants:\n{text}");
    assert!(!text.contains(crate::tailcallelim::DEFERRED), "a function still waits:\n{text}");
}

const ROW_RECURSION: &str = "@q = global [8 x i32] zeroinitializer

define internal i32 @safe(i32 %row, i32 %col) {
b0:
  br label %head

head:
  %r = phi i32 [ 0, %b0 ], [ %r1, %next ]
  %c = icmp slt i32 %r, %row
  br i1 %c, label %body, label %yes

body:
  %p = getelementptr inbounds i32, ptr @q, i32 %r
  %v = load i32, ptr %p
  %d = sub i32 %v, %col
  %z = icmp eq i32 %d, 0
  br i1 %z, label %no, label %next

next:
  %r1 = add nsw i32 %r, 1
  br label %head

yes:
  ret i32 1

no:
  ret i32 0
}

define internal i32 @place(i32 %row, i32 %n) {
b0:
  %done = icmp eq i32 %row, %n
  br i1 %done, label %one, label %loop

one:
  ret i32 1

loop:
  %col = phi i32 [ 0, %b0 ], [ %col1, %latch ]
  %found = phi i32 [ 0, %b0 ], [ %found2, %latch ]
  %c = icmp slt i32 %col, %n
  br i1 %c, label %test, label %exit

test:
  %ok = call i32 @safe(i32 %row, i32 %col)
  %t = icmp ne i32 %ok, 0
  br i1 %t, label %put, label %latch

put:
  %p = getelementptr inbounds i32, ptr @q, i32 %row
  store i32 %col, ptr %p
  %r1 = add nsw i32 %row, 1
  %sub = call i32 @place(i32 %r1, i32 %n)
  %f = add i32 %found, %sub
  br label %latch

latch:
  %found2 = phi i32 [ %found, %test ], [ %f, %put ]
  %col1 = add nsw i32 %col, 1
  br label %loop

exit:
  ret i32 %found
}

define i32 @top(i32 %n) {
b0:
  %r = call i32 @place(i32 0, i32 %n)
  ret i32 %r
}
";

/// A recursion whose argument advances by a constant (`f(row + 1)`) is cloned
/// in a chain before the walk, as gcc runs all of ipa-cp before ipa-inline: the
/// clone for row names the clone for row + 1 and each is inlined into the one
/// before it. Cloned after the walk, seven clones stood, none inlined (m32
/// queens -O3: 160,324 clocks against gcc's 94,851).
#[test]
fn test_a_recursion_with_an_advancing_constant_is_cloned_in_a_chain_before_the_walk() {
    let mut module = parsed(ROW_RECURSION);
    let options = crate::pipeline::Options::aggressive();
    let applied = crate::pipeline::Applied { options, ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    let clones = text.lines().filter(|line| line.starts_with("define") && line.contains("@place.constprop")).count();
    // The chain is eight long (`MAX_CLONES`); the links the walk's threshold
    // stops at stand. Cloned after the walk, all seven stood.
    assert!(clones <= 2, "{clones} clones of place stand, the chain was not inlined link by link:\n{text}");
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| {
            let mut walk = crate::walk::Walk {
                promote_rate: None,
                rerun_unchanged: true,
                speed_rate: true,
                inline: crate::inline::Threshold { recursive: recursive.then_some(true), ..Default::default() },
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

/// The default is the pin's inliner: a callee that calls itself is not copied
/// into its caller (`IsRecursiveCall`; hanoi's nine copies and its lookup
/// table, 40 instructions where gcc runs 35,764 and clang 249,852, are
/// `-finline-recursive`, a DEVIATE (recursion) the pin has no counterpart of).
#[test]
fn test_the_default_does_not_copy_a_recursive_callee() {
    let copies = walked_top_with(LINEAR, false).matches("mul i32 %").count();
    assert_eq!(copies, 0, "{copies} copies of down in top by default");
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
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

/// `sum_three(&a, &b, &c)` on three stack records with far data pointers: the
/// callee leaves `capacity` alone, so its stores go and the kept call counted
/// 10 units against 15 inlined. The call pushes three far pointers and the
/// records need a frame, 114 bytes against 75: parity/sum_three/c -Os, +38 B.
#[test]
fn test_an_os_call_on_stack_records_is_inlined_where_the_bytes_say_so() {
    let top = sized(
        r#"target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

%struct.Slice = type { i16, i16, ptr addrspace(1) }

@first = internal global [4 x i16] [i16 1, i16 2, i16 3, i16 4], align 2
@second_data = internal global [4 x i16] [i16 10, i16 20, i16 30, i16 40], align 2
@third_data = internal global [4 x i16] [i16 100, i16 200, i16 300, i16 400], align 2

define signext i16 @sum(ptr addrspace(1) noundef %0, ptr addrspace(1) noundef %1, ptr addrspace(1) noundef %2) addrspace(1) nounwind optsize memory(read, inaccessiblemem: none) nofree nosync norecurse {
  br label %4

4:
  %5 = phi i16 [ 0, %3 ], [ %25, %9 ]
  %6 = phi i16 [ 0, %3 ], [ %24, %9 ]
  %7 = load i16, ptr addrspace(1) %0, align 1
  %8 = icmp ult i16 %5, %7
  br i1 %8, label %9, label %26

9:
  %10 = getelementptr inbounds nuw i8, ptr addrspace(1) %0, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10, align 1
  %12 = getelementptr inbounds nuw [2 x i8], ptr addrspace(1) %11, i16 %5
  %13 = load i16, ptr addrspace(1) %12, align 2
  %14 = add nsw i16 %6, %13
  %15 = getelementptr inbounds nuw i8, ptr addrspace(1) %1, i16 4
  %16 = load ptr addrspace(1), ptr addrspace(1) %15, align 1
  %17 = getelementptr inbounds nuw [2 x i8], ptr addrspace(1) %16, i16 %5
  %18 = load i16, ptr addrspace(1) %17, align 2
  %19 = add nsw i16 %14, %18
  %20 = getelementptr inbounds nuw i8, ptr addrspace(1) %2, i16 4
  %21 = load ptr addrspace(1), ptr addrspace(1) %20, align 1
  %22 = getelementptr inbounds nuw [2 x i8], ptr addrspace(1) %21, i16 %5
  %23 = load i16, ptr addrspace(1) %22, align 2
  %24 = add nsw i16 %19, %23
  %25 = add i16 %5, 1
  br label %4

26:
  ret i16 %6
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) addrspace(1) nocallback nofree nosync nounwind

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) addrspace(1) nocallback nofree nosync nounwind

define i32 @top() addrspace(1) nounwind optsize {
  %1 = alloca %struct.Slice, align 1
  %2 = alloca %struct.Slice, align 1
  %3 = alloca %struct.Slice, align 1
  call addrspace(1) void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  call addrspace(1) void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  call addrspace(1) void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  store i16 4, ptr %1, align 1
  %4 = getelementptr inbounds nuw i8, ptr %1, i16 2
  store i16 4, ptr %4, align 1
  %5 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store ptr addrspace(1) addrspacecast (ptr @first to ptr addrspace(1)), ptr %5, align 1
  store i16 4, ptr %2, align 1
  %6 = getelementptr inbounds nuw i8, ptr %2, i16 2
  store i16 4, ptr %6, align 1
  %7 = getelementptr inbounds nuw i8, ptr %2, i16 4
  store ptr addrspace(1) addrspacecast (ptr @second_data to ptr addrspace(1)), ptr %7, align 1
  store i16 4, ptr %3, align 1
  %8 = getelementptr inbounds nuw i8, ptr %3, i16 2
  store i16 4, ptr %8, align 1
  %9 = getelementptr inbounds nuw i8, ptr %3, i16 4
  store ptr addrspace(1) addrspacecast (ptr @third_data to ptr addrspace(1)), ptr %9, align 1
  %10 = addrspacecast ptr %1 to ptr addrspace(1)
  %11 = addrspacecast ptr %2 to ptr addrspace(1)
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  %13 = call signext addrspace(1) i16 @sum(ptr addrspace(1) noundef %10, ptr addrspace(1) noundef %11, ptr addrspace(1) noundef %12) nobuiltin optsize
  %14 = sext i16 %13 to i32
  call addrspace(1) void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call addrspace(1) void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  call addrspace(1) void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i32 %14
}
"#,
        "@top(",
    );
    assert!(!top.contains("@sum("), "sum stayed a call:\n{top}");
}

/// `@top` after the pipeline sized for bytes over `text`.
fn sized(
    text: &str,
    name: &str,
) -> String {
    let mut module = parsed(text);
    let applied = crate::pipeline::Applied { options: crate::pipeline::Options::size(), ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let printed = llrm_mir::print::module(&module);
    printed.split("define").find(|one| one.contains(name)).expect("the function").to_owned()
}

/// `step(&value, &delta)` on two private globals the caller has just stored
/// constants to folds to a constant once inlined, and the call (two pushed
/// addresses) costs more bytes than the constant; the pin inlines it at -Os
/// (`cost=-15, threshold=75`). The -Os bound alone refused it, and the trial
/// (D12) looked at stack slots only: a2_small_function_twice.c 14 -> 73 bytes,
/// memory.nib 19 -> 89.
#[test]
fn test_an_os_call_whose_global_arguments_hold_stored_constants_is_inlined() {
    let top = sized(
        "@value = internal global i16 0
@delta = internal global i16 0

define internal void @step(ptr %v, ptr %d) {
b0:
  %a = load i16, ptr %v
  %m = mul i16 %a, 3
  %b = load i16, ptr %d
  %s = add i16 %m, %b
  store i16 %s, ptr %v
  ret void
}

define i16 @a2b() {
b0:
  store i16 7, ptr @value
  store i16 -2, ptr @delta
  call void @step(ptr @value, ptr @delta)
  store i16 -4, ptr @value
  store i16 11, ptr @delta
  call void @step(ptr @value, ptr @delta)
  %r = load i16, ptr @value
  ret i16 %r
}
",
        "@a2b(",
    );
    assert!(!top.contains("call"), "the call to step stayed:\n{top}");
}

/// The same shape with the far pointers a Nib `&mut i16` argument is
/// (`memory.nib`, 19 -> 89 bytes): the arguments are `addrspacecast (ptr @g to
/// ptr addrspace(1))`.
#[test]
fn test_an_os_call_whose_far_global_arguments_hold_stored_constants_is_inlined() {
    let top = sized(
        "@value = internal global i16 0
@delta = internal global i16 0

define internal void @step(ptr addrspace(1) %v, ptr addrspace(1) %d) {
b0:
  %a = load i16, ptr addrspace(1) %v
  %m = mul i16 %a, 3
  %b = load i16, ptr addrspace(1) %d
  %s = add i16 %m, %b
  store i16 %s, ptr addrspace(1) %v
  ret void
}

define i16 @a2f() {
b0:
  store i16 7, ptr @value
  store i16 -2, ptr @delta
  call void @step(ptr addrspace(1) addrspacecast (ptr @value to ptr addrspace(1)), ptr addrspace(1) addrspacecast (ptr @delta to ptr addrspace(1)))
  store i16 -4, ptr @value
  store i16 11, ptr @delta
  call void @step(ptr addrspace(1) addrspacecast (ptr @value to ptr addrspace(1)), ptr addrspace(1) addrspacecast (ptr @delta to ptr addrspace(1)))
  %r = load i16, ptr @value
  ret i16 %r
}
",
        "@a2f(",
    );
    assert!(!top.contains("call"), "the call to step stayed:\n{top}");
}

/// The same two calls with values the caller does not know: the first call's
/// stores are dead, and `step` private with both calls in `a2c` goes with its
/// last, so the copy is smaller than the calls and the body it replaces.
#[test]
fn test_an_os_call_whose_global_arguments_hold_unknown_values_goes_with_its_private_callee() {
    let top = sized(
        "@value = internal global i16 0
@delta = internal global i16 0

define internal void @step(ptr %v, ptr %d) {
b0:
  %a = load i16, ptr %v
  %m = mul i16 %a, 3
  %b = load i16, ptr %d
  %s = add i16 %m, %b
  store i16 %s, ptr %v
  ret void
}

define i16 @a2c(i16 %x, i16 %y) {
b0:
  store i16 %x, ptr @value
  store i16 %y, ptr @delta
  call void @step(ptr @value, ptr @delta)
  store i16 %y, ptr @value
  store i16 %x, ptr @delta
  call void @step(ptr @value, ptr @delta)
  %r = load i16, ptr @value
  ret i16 %r
}
",
        "@a2c(",
    );
    assert!(!top.contains("call"), "step stayed a call:\n{top}");
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
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        move |program: &mut llrm_mir::program::Program| {
            let seen = std::rc::Rc::clone(&seen);
            let mut walk = crate::walk::Walk {
                promote_rate: None,
                rerun_unchanged: true,
                speed_rate: true,
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

/// AM-BD-6: `ModuleIpo` states what the pin's `opt -passes=ipsccp` states.
/// The counted loop's `%r` reaches `@sink` below `%row`, in [0, 8): the pin's
/// `range(i16 -32768, 6)`. The stand-in (`parameter_ranges`) stated
/// [-32768, 32767), and nothing at -O1.
#[test]
fn test_module_ipo_states_the_argument_ranges_the_pins_ipsccp_does() {
    use llrm_mir::program::ProgramPass;
    let mut module = parsed(crate::ipsccp::tests::AM_BD_6);
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| {
            let mut ipo =
                crate::walk::ModuleIpo { inline: crate::inline::Threshold::none(), cloning: Default::default() };
            ipo.run(program, &mut Default::default())
        },
    )
    .and_then(|done| done)
    .expect("ModuleIpo runs");
    let text = crate::testing::printed(&module);
    let header = |name: &str| {
        let at = format!("@{name}(");
        text.lines().find(|line| line.starts_with("define") && line.contains(&at)).unwrap_or_default().to_owned()
    };
    assert!(header("sink").contains("range(i16 -32768, 6) %x"), "{text}");
    assert!(header("place").contains("range(i16 0, 8) %row"), "{text}");
    assert!(text.contains("icmp eq i16 %row, 7"), "{text}");
}

/// A float kernel called once on stack slots: the trial's copy adds x87 loads,
/// multiplies and a compare-and-branch the call did not, and costs 2 bytes more
/// than the call (`qmove`, 147 -> 149 bytes at m16 -Os). The model counted each
/// float instruction as one and tied, so `<=` kept the copy.
#[test]
fn test_an_os_trial_that_adds_x87_code_to_a_stack_slot_float_call_stays_a_call() {
    let top = sized(
        "@vx = internal global float 3.0
@vy = internal global float 4.0
@wx = internal global float 1.0
@speed = internal global float 10.0
@dt = internal global float 0.25

define void @accel(ptr %0, ptr %1, float %2, float %3) {
b1:
  %4 = load float, ptr %0
  %5 = load float, ptr %1
  %6 = fmul float %4, %5
  %7 = getelementptr inbounds nuw i8, ptr %0, i16 4
  %8 = load float, ptr %7
  %9 = getelementptr inbounds nuw i8, ptr %1, i16 4
  %10 = load float, ptr %9
  %11 = fmul float %8, %10
  %12 = fadd float %6, %11
  %13 = fsub float %2, %12
  %14 = fcmp ole float %13, 0.000000e+00
  br i1 %14, label %b4, label %b2

b2:
  %15 = fmul float %2, 1.000000e+01
  %16 = fmul float %15, %3
  %17 = fcmp ogt float %16, %13
  %18 = select i1 %17, float %13, float %16
  %19 = load float, ptr %0
  %20 = load float, ptr %1
  %21 = fmul float %18, %20
  %22 = fadd float %19, %21
  store float %22, ptr %0
  %23 = load float, ptr %7
  %24 = load float, ptr %9
  %25 = fmul float %18, %24
  %26 = fadd float %23, %25
  store float %26, ptr %7
  br label %b4

b4:
  ret void
}

define i32 @bench() {
b1:
  %0 = alloca [12 x i8]
  %1 = alloca [12 x i8]
  %2 = load volatile float, ptr @vx
  store float %2, ptr %0
  %3 = load volatile float, ptr @vy
  %4 = getelementptr inbounds nuw i8, ptr %0, i16 4
  store float %3, ptr %4
  %5 = getelementptr inbounds nuw i8, ptr %0, i16 8
  store float 5.000000e+00, ptr %5
  %6 = load volatile float, ptr @wx
  store float %6, ptr %1
  %7 = getelementptr inbounds nuw i8, ptr %1, i16 4
  store float 0.000000e+00, ptr %7
  %8 = getelementptr inbounds nuw i8, ptr %1, i16 8
  store float 0.000000e+00, ptr %8
  %9 = load volatile float, ptr @dt
  %10 = load volatile float, ptr @speed
  call void @accel(ptr %0, ptr %1, float %10, float %9)
  %11 = load float, ptr %0
  %12 = fptosi float %11 to i32
  %13 = mul nsw i32 %12, 10000
  %14 = load float, ptr %4
  %15 = fptosi float %14 to i32
  %16 = mul nsw i32 %15, 100
  %17 = add nsw i32 %13, %16
  %18 = load float, ptr %5
  %19 = fptosi float %18 to i32
  %20 = add nsw i32 %17, %19
  ret i32 %20
}
",
        "@bench(",
    );
    assert!(top.contains("call void @accel"), "accel was inlined for 2 bytes more:\n{top}");
}

/// `@top` after the -O2 pipeline over `text`.
fn at_o2(
    text: &str,
    name: &str,
) -> String {
    let mut module = parsed(text);
    let applied = crate::pipeline::Applied { options: crate::pipeline::Options::default(), ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let printed = llrm_mir::print::module(&module);
    printed.split("define").find(|one| one.contains(name)).expect("the function").to_owned()
}

/// `@top` after the -O2 pipeline over `text`, on `target`.
fn at_o2_on(
    target: llrm_x86_m16::Dos,
    text: &str,
    name: &str,
) -> String {
    let mut module = parsed(text);
    let applied = crate::pipeline::Applied { options: crate::pipeline::Options::default(), ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(target),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let printed = llrm_mir::print::module(&module);
    printed.split("define").find(|one| one.contains(name)).expect("the function").to_owned()
}

const GROWS: &str = "@g = global i32 0

define i32 @callee(i32 %x) {
b0:
  %a = load i32, ptr @g
  %b = add i32 %a, %x
  %c = add i32 %b, %a
  %d = mul i32 %c, %x
  %e = add i32 %d, %b
  %f = mul i32 %e, %a
  %g = add i32 %f, %c
  %h = mul i32 %g, %d
  %i = add i32 %h, %a
  %j = mul i32 %i, %x
  %k = add i32 %j, %c
  %l = mul i32 %k, %b
  %m = add i32 %l, %e
  store i32 %m, ptr @g
  ret i32 %m
}
";

/// A speed caller at m16 follows the pin's threshold, as main does: a callee
/// that grows the code is copied. A guard that held it to the callee's size
/// kept qlight's four `ls_scale_byte` calls at -O2 (472 clocks against main's
/// 463, and a size bound that priced the copy at 1.4 to 1.8 times its bytes).
#[test]
fn test_an_o2_call_of_a_callee_that_grows_the_code_is_inlined_at_m16_as_the_pin_does() {
    let mut big = String::from("@g = global i32 0\n\ndefine i32 @callee(i32 %x) {\nb0:\n");
    let steps = 10;
    for step in 0..steps {
        let before = if step == 0 { "%x".to_owned() } else { format!("%w{}", step - 1) };
        big.push_str(&format!("  %v{step} = load volatile i32, ptr @g\n  %w{step} = add i32 {before}, %v{step}\n"));
    }
    big.push_str(&format!("  ret i32 %w{}\n}}\n", steps - 1));
    let text = format!("{big}\ndefine i32 @top(i32 %y) {{\nb0:\n  %r = call i32 @callee(i32 %y)\n  ret i32 %r\n}}\n");
    let pin = at_o2_on(llrm_x86_m16::Dos::pin_tuned(), &text, "@top(");
    let m16 = at_o2_on(llrm_x86_m16::Dos::default(), &text, "@top(");
    assert!(!pin.contains("call i32 @callee"), "premise: the pin inlines it:\n{pin}");
    assert!(!m16.contains("call i32 @callee"), "m16 kept the call:\n{m16}");
}

/// A callee that grows the code a little is copied into a speed caller at m16
/// (the 13-operation `GROWS`).
#[test]
fn test_an_o2_call_of_a_callee_whose_clocks_pay_for_its_growth_is_inlined_at_m16() {
    let text = format!("{GROWS}\ndefine i32 @top(i32 %y) {{\nb0:\n  %r = call i32 @callee(i32 %y)\n  ret i32 %r\n}}\n");
    let m16 = at_o2_on(llrm_x86_m16::Dos::default(), &text, "@top(");
    assert!(!m16.contains("call i32 @callee"), "the call stayed:\n{m16}");
}

/// The same callee with a constant argument folds in the caller once inlined
/// (algebra, control, memory: 15 clocks inlined, 143 to 283 as calls), so
/// the blocked site's trial keeps the inline.
#[test]
fn test_an_o2_call_that_folds_once_inlined_is_inlined_at_m16() {
    let text = "define i32 @callee(i32 %n) {
b0:
  br label %h
h:
  %i = phi i32 [ 0, %b0 ], [ %i1, %h ]
  %a = phi i32 [ 1, %b0 ], [ %a1, %h ]
  %m = mul i32 %a, 31
  %x = xor i32 %m, %i
  %a1 = add i32 %x, 7
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, %n
  br i1 %c, label %h, label %e
e:
  ret i32 %a1
}

define i32 @top() {
b0:
  %r = call i32 @callee(i32 6)
  ret i32 %r
}
";
    let m16 = at_o2_on(llrm_x86_m16::Dos::default(), text, "@top(");
    assert!(!m16.contains("call i32 @callee"), "the folding call stayed:\n{m16}");
}

/// qmove c at m16 -O2: `pl_ground_accel` writes `.x` and `.y` of a vector whose
/// `.z` the caller set and reads back through `(long)`. Inlined (the pin does,
/// cost 5 of 225) the read folds to a constant and the call goes: the code
/// grows 30 -> 42 but 43 clocks go (`fld`+`fistp`), worth it at 1 clock = 2
/// bytes. Held to bytes alone it stayed a call: +41 clk against 854.
#[test]
fn test_an_o2_call_that_grows_the_code_but_saves_clocks_is_inlined_at_m16() {
    let text = "@g = global float 3.0

define void @callee(ptr %v, ptr %w, float %s, float %dt) {
b0:
  %x = load float, ptr %v
  %wx = load float, ptr %w
  %vy.p = getelementptr i8, ptr %v, i32 4
  %y = load float, ptr %vy.p
  %wy.p = getelementptr i8, ptr %w, i32 4
  %wy = load float, ptr %wy.p
  %a = fmul float %x, %wx
  %b = fmul float %y, %wy
  %c = fadd float %a, %b
  %add = fsub float %s, %c
  %le = fcmp ole float %add, 0.0
  br i1 %le, label %out, label %go
go:
  %acc = fmul float %s, %dt
  %big = fcmp ogt float %acc, %add
  %use = select i1 %big, float %add, float %acc
  %dx = fmul float %use, %wx
  %nx = fadd float %x, %dx
  store float %nx, ptr %v
  %dy = fmul float %use, %wy
  %ny = fadd float %y, %dy
  store float %ny, ptr %vy.p
  %e1 = fmul float %nx, %ny
  %e2 = fadd float %e1, %dx
  store float %e2, ptr %w
  br label %out
out:
  ret void
}

define i32 @top(float %g0, float %g1, float %s) {
b0:
  %vel = alloca [12 x i8]
  %wish = alloca [12 x i8]
  store float %g0, ptr %vel
  store float %g1, ptr %wish
  %wy.s = getelementptr i8, ptr %wish, i32 4
  store float %g1, ptr %wy.s
  %vy.s = getelementptr i8, ptr %vel, i32 4
  store float %g0, ptr %vy.s
  %vz = getelementptr i8, ptr %vel, i32 8
  store float 5.0, ptr %vz
  call void @callee(ptr %vel, ptr %wish, float %s, float %g1)
  %z = load float, ptr %vz
  %i = fptosi float %z to i32
  %vx = load float, ptr %vel
  %j = fptosi float %vx to i32
  %k = add i32 %i, %j
  %vy.q = getelementptr i8, ptr %vel, i32 4
  %vy = load float, ptr %vy.q
  %l = fptosi float %vy to i32
  %m = add i32 %k, %l
  ret i32 %m
}
";
    let m16 = at_o2_on(llrm_x86_m16::Dos::default(), text, "@top(");
    assert!(!m16.contains("call void @callee"), "the call stayed:\n{m16}");
}

/// qlight c at m16 -O2: `ls_scale_byte` (a clamped multiply and a divide by
/// 255) four times. Kept as calls it ran 472 clocks, above main's 463 (main
/// inlines it); inlined it ran 388 clocks.
#[test]
fn test_an_o2_call_of_a_clamped_divide_by_a_constant_is_inlined_at_m16_where_main_inlines_it() {
    let text = "@raw = global i16 200
@low = global i16 -20
@s1 = global i16 120
@s2 = global i16 60
@s3 = global i16 30

define i16 @scale(i16 %raw, i16 %s) {
b0:
  %a = sext i16 %raw to i32
  %b = sext i16 %s to i32
  %m = mul i32 %a, %b
  %d = sdiv i32 %m, 255
  %hi = icmp sgt i32 %d, 255
  %c1 = select i1 %hi, i32 255, i32 %d
  %lo = icmp slt i32 %c1, 0
  %c2 = select i1 %lo, i32 0, i32 %c1
  %r = trunc i32 %c2 to i16
  ret i16 %r
}

define i32 @top() {
b0:
  %x = load volatile i16, ptr @raw
  %y = load volatile i16, ptr @s1
  %p = call i16 @scale(i16 %x, i16 %y)
  %x2 = load volatile i16, ptr @raw
  %z = load volatile i16, ptr @s2
  %q = call i16 @scale(i16 %x2, i16 %z)
  %x3 = load volatile i16, ptr @raw
  %w = load volatile i16, ptr @s3
  %r = call i16 @scale(i16 %x3, i16 %w)
  %x4 = load volatile i16, ptr @low
  %y4 = load volatile i16, ptr @s1
  %t = call i16 @scale(i16 %x4, i16 %y4)
  %a = sext i16 %p to i32
  %b = sext i16 %q to i32
  %c = sext i16 %r to i32
  %d = sext i16 %t to i32
  %am = mul i32 %a, 1000000
  %bm = mul i32 %b, 1000
  %ab = add i32 %am, %bm
  %cd = add i32 %c, %d
  %s = add i32 %ab, %cd
  ret i32 %s
}
";
    let m16 = at_o2_on(llrm_x86_m16::Dos::default(), text, "@top(");
    assert_eq!(m16.matches("call i16 @scale").count(), 0, "the calls stayed:\n{m16}");
}

/// qbsp/loop/branch at m16 -O2: a call in a loop that saves a few clocks and
/// grows the code a few bytes was refused (+126 B for -298 clk against 854),
/// for its saving was counted once where the site runs per trip. The same
/// callee as the qlight test, called from a loop, is inlined.
#[test]
fn test_an_o2_call_in_a_loop_saves_its_clocks_per_trip_at_m16() {
    let text = "@raw = global i16 200
@s1 = global i16 120

define i16 @scale(i16 %raw, i16 %s) {
b0:
  %a = sext i16 %raw to i32
  %b = sext i16 %s to i32
  %m = mul i32 %a, %b
  %d = sdiv i32 %m, 255
  %hi = icmp sgt i32 %d, 255
  %c1 = select i1 %hi, i32 255, i32 %d
  %lo = icmp slt i32 %c1, 0
  %c2 = select i1 %lo, i32 0, i32 %c1
  %r = trunc i32 %c2 to i16
  ret i16 %r
}

define i32 @top(i32 %n) {
b0:
  br label %h
h:
  %i = phi i32 [ 0, %b0 ], [ %i1, %h ]
  %acc = phi i32 [ 0, %b0 ], [ %acc1, %h ]
  %x = load volatile i16, ptr @raw
  %y = load volatile i16, ptr @s1
  %p = call i16 @scale(i16 %x, i16 %y)
  %e = sext i16 %p to i32
  %acc1 = add i32 %acc, %e
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, %n
  br i1 %c, label %h, label %out
out:
  ret i32 %acc1
}
";
    let m16 = at_o2_on(llrm_x86_m16::Dos::default(), text, "@top(");
    assert!(!m16.contains("call i16 @scale"), "the call stayed:\n{m16}");
}

/// A call that folds once inlined stays a call with inlining off
/// (`-fno-inline-functions`): the trial of a blocked site at a speed caller
/// ignored the switch, and 6 llrm-nib tests lost their out-of-line callees
/// (`_kdiv proc near` not in the listing) and every bench kernel folded into
/// `main`.
#[test]
fn test_an_o2_call_that_folds_once_inlined_stays_a_call_with_inlining_off() {
    let text = "define i32 @callee(i32 %n) {
b0:
  br label %h
h:
  %i = phi i32 [ 0, %b0 ], [ %i1, %h ]
  %a = phi i32 [ 1, %b0 ], [ %a1, %h ]
  %m = mul i32 %a, 31
  %x = xor i32 %m, %i
  %a1 = add i32 %x, 7
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, %n
  br i1 %c, label %h, label %e
e:
  ret i32 %a1
}

define i32 @top() {
b0:
  %r = call i32 @callee(i32 6)
  ret i32 %r
}
";
    let mut module = parsed(text);
    let options = crate::pipeline::Options { inline: crate::inline::Threshold::none(), ..Default::default() };
    let applied = crate::pipeline::Applied { options, ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let printed = llrm_mir::print::module(&module);
    let top = printed.split("define").find(|one| one.contains("@top(")).expect("the function");
    assert!(top.contains("call i32 @callee"), "inlining off, the call was inlined:\n{top}");
}

/// `@top` after the -Os pipeline over `text`.
fn at_os(
    text: &str,
    name: &str,
) -> String {
    let mut module = parsed(text);
    let applied = crate::pipeline::Applied { options: crate::pipeline::Options::size(), ..Default::default() };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let printed = llrm_mir::print::module(&module);
    printed.split("define").find(|one| one.contains(name)).expect("the function").to_owned()
}

/// Two calls of a private tail-recursive `gcd`, one with a constant argument
/// (it folds, so the -Os size bound passes it) and one without (the bound
/// refused it, and the trial of it alone credited no callee body because the
/// other call still counted as a reference): `call _gcd` stayed in
/// `bench_recgcd` (m16 -Os, +20 B), where the pin inlines both and drops
/// `gcd`.
#[test]
fn test_an_os_site_beside_an_inlined_site_of_a_private_callee_gets_the_callee_credit() {
    let top = at_os(
        "define internal i16 @gcd(i16 %a, i16 %b) {
b0:
  %z = icmp eq i16 %b, 0
  br i1 %z, label %done, label %rec

rec:
  %r = srem i16 %a, %b
  %t = call i16 @gcd(i16 %b, i16 %r)
  ret i16 %t

done:
  ret i16 %a
}

define i16 @bench(i16 %n, i16 %m) {
b0:
  %x = mul i16 %n, 7
  %y = add i16 %x, 3
  %p = mul i16 %m, 5
  %q = add i16 %p, 2
  %g1 = call i16 @gcd(i16 %y, i16 %q)
  %s = add i16 %n, 90
  %g2 = call i16 @gcd(i16 %s, i16 360)
  %t = add i16 %g1, %g2
  ret i16 %t
}
",
        "@bench(",
    );
    assert!(!top.contains("@gcd"), "a call of gcd stayed:\n{top}");
}

/// A private callee that is one 16-case switch over values 2..21 (a jump
/// table on x86) was priced as a compare tree, 3n/2-1 compares, because the
/// walk never gave the cost model the target's switch answer: cost 290 against
/// 225, so it stayed a call that the pin inlines (QCport `item_mdl_id` in
/// `mdl_rebind_items`; the pin prices the table, 20 entries, about 175).
#[test]
fn test_an_o2_call_to_a_dense_switch_callee_is_inlined_as_the_pin_does() {
    let cases: Vec<i16> = vec![2, 3, 4, 6, 7, 8, 10, 11, 12, 14, 15, 16, 17, 18, 20, 21];
    let labels: String = cases.iter().map(|c| format!(" i16 {c}, label %c{c}\n")).collect();
    let arms: String =
        cases.iter().map(|c| format!("c{c}:\n  %v{c} = mul i16 %w, {}\n  br label %out\n\n", c * 3 + 1)).collect();
    let incoming: String = cases.iter().map(|c| format!(", [ %v{c}, %c{c} ]")).collect();
    let top = at_o2(
        &format!(
            "define internal i16 @pick(ptr %it, i16 %w) {{
b0:
  %k = load i16, ptr %it
  switch i16 %k, label %out [
{labels}  ]

{arms}out:
  %r = phi i16 [ -1, %b0 ]{incoming}
  ret i16 %r
}}

define i16 @top(ptr %p, ptr %q, i16 %w) {{
b0:
  %a = call i16 @pick(ptr %p, i16 %w)
  %b = call i16 @pick(ptr %q, i16 %a)
  ret i16 %b
}}
"
        ),
        "@top(",
    );
    assert!(!top.contains("@pick"), "the dense switch callee stayed a call:\n{top}");
}

/// A private callee with a stack frame, called once from a recursive function,
/// stayed a call because `fits_with` never copies a body that allocates into a
/// recursive caller; its only call goes with it, so the copy shrinks the code
/// and the recursive path is no deeper than before (QCport
/// `ent_lightning_` into `ent_use_targets_`: the pin inlines it).
#[test]
fn test_an_o2_last_call_to_a_private_callee_with_a_frame_is_inlined_into_a_recursive_caller() {
    let top = at_o2(
        "declare void @sink(ptr)

define internal void @leaf(i16 %v) {
b0:
  %buf = alloca [16 x i8]
  store i16 %v, ptr %buf
  call void @sink(ptr %buf)
  ret void
}

define i16 @rec(i16 %n) {
b0:
  %c = icmp sgt i16 %n, 0
  br i1 %c, label %go, label %out

go:
  call void @leaf(i16 %n)
  %m = sub i16 %n, 1
  %r = call i16 @rec(i16 %m)
  %s = add i16 %r, %n
  ret i16 %s

out:
  ret i16 0
}
",
        "@rec(",
    );
    assert!(!top.contains("@leaf"), "the last call stayed a call:\n{top}");
}

/// A flat x86 target: the neutral machine with the CPU's cost model.
struct FlatX86(&'static str);

impl llrm_mir::target::Machine for FlatX86 {
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        llrm_mir::target::Neutral.foreign_span(selectors, offsets, width)
    }
    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        llrm_mir::target::Neutral.spaces()
    }
    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::Neutral.costs()
    }
    fn registers(&self) -> i64 {
        llrm_mir::target::Neutral.registers()
    }
    fn call_registers(&self) -> i64 {
        llrm_mir::target::Neutral.call_registers()
    }
    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }
    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        Some(std::rc::Rc::new(llrm_x86::tti::X86::of_cpu(self.0).expect("a cpu")))
    }
}

/// tests/run/c/shrink_wrapped_frame.c at m32 -O1: `f` (a 64-byte array) is
/// priced to inline into the recursive `g` (cost 40, threshold 225, as the
/// pin's `-Rpass=inline` says) and is also called from `main`, so it is not a
/// last call. `fits_with` refused every allocating callee in a recursive
/// caller, so `f` stayed a 60-byte call where the pin has the copy (+19 B).
/// The pin allows up to 1024 bytes (`TotalAllocaSizeRecursiveCaller`); a
/// platform that states otherwise (real mode) keeps the refusal.
#[test]
fn test_a_flat_target_inlines_a_small_frame_into_a_recursive_caller_as_the_pin_does() {
    let text = "declare void @sink(ptr)

define internal void @leaf(i32 %v) {
b0:
  %buf = alloca [16 x i32]
  store i32 %v, ptr %buf
  call void @sink(ptr %buf)
  ret void
}

define i32 @rec(i32 %n) {
b0:
  %c = icmp sgt i32 %n, 0
  br i1 %c, label %go, label %out

go:
  call void @leaf(i32 %n)
  %m = sub i32 %n, 1
  %r = call i32 @rec(i32 %m)
  %s = add i32 %r, %n
  ret i32 %s

out:
  ret i32 0
}

define i32 @other() {
b0:
  call void @leaf(i32 3)
  ret i32 0
}
";
    let run = |target: std::rc::Rc<dyn llrm_mir::target::Machine>| {
        let mut module = parsed(text);
        let applied = crate::pipeline::Applied { options: crate::pipeline::Options::default(), ..Default::default() };
        llrm_mir::program::Program::lend(&mut module, target, |program: &mut llrm_mir::program::Program| {
            crate::pipeline::applied(program, &applied)
        })
        .and_then(|done| done)
        .expect("the pipeline runs");
        let printed = llrm_mir::print::module(&module);
        printed.split("define").find(|one| one.contains("@rec(")).expect("the function").to_owned()
    };
    let flat = run(std::rc::Rc::new(FlatX86("i486")));
    assert!(!flat.contains("@leaf"), "the small frame stayed a call in the recursive caller:\n{flat}");
    let real = run(std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()));
    assert!(real.contains("@leaf"), "real mode copied a frame into a recursion:\n{real}");
}

/// The pin's `GlobalOpt` runs between IPSCCP and the walk (pass 20 of
/// `default<O2>` on sum_three): a static a norecurse function writes before it
/// reads is a register when the loop passes run. Ours split and localized after
/// the walk, so `indvars` saw the memory (sum_three.bas kept the `sgt` exit
/// test).
#[test]
fn test_module_ipo_makes_a_static_one_function_writes_first_a_stack_slot() {
    use llrm_mir::program::ProgramPass;
    let mut module = parsed(
        "@n = internal global [2 x i8] zeroinitializer

define i16 @f(i16 %a) norecurse {
b0:
  store i16 %a, ptr @n
  %x = load i16, ptr @n
  %y = add i16 %x, 1
  ret i16 %y
}
",
    );
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| {
            let mut ipo =
                crate::walk::ModuleIpo { inline: crate::inline::Threshold::none(), cloning: Default::default() };
            ipo.run(program, &mut Default::default())
        },
    )
    .and_then(|done| done)
    .expect("ModuleIpo runs");
    let text = crate::testing::printed(&module);
    let body = text.split("define").nth(1).unwrap_or_default();
    assert!(!body.contains("ptr @n") && body.contains("alloca i16"), "{text}");
}

/// far6.bas (`far_poke_scanned_variable`): the split ahead of the walk made a
/// static a stack slot while another body forged a pointer into the data
/// segment (`DEF SEG = VARSEG(w)`) and POKEd it: 21257 where BC printed 100.
#[test]
fn test_module_ipo_keeps_a_static_in_memory_while_a_body_forges_a_selector() {
    use llrm_mir::program::ProgramPass;
    let mut module = parsed(
        "@n = internal global [2 x i8] zeroinitializer

define i16 @f(i16 %a) norecurse {
b0:
  store i16 %a, ptr @n
  %x = load i16, ptr @n
  %y = add i16 %x, 1
  ret i16 %y
}

define i8 @g(i16 %o) {
b0:
  %s = inttoptr i16 %o to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %v = load volatile i8, ptr addrspace(1) %far
  ret i8 %v
}
",
    );
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| {
            let mut ipo =
                crate::walk::ModuleIpo { inline: crate::inline::Threshold::none(), cloning: Default::default() };
            ipo.run(program, &mut Default::default())
        },
    )
    .and_then(|done| done)
    .expect("ModuleIpo runs");
    let text = crate::testing::printed(&module);
    let body = text.split("define").nth(1).unwrap_or_default();
    assert!(body.contains("ptr @n") && !body.contains("alloca i16"), "{text}");
}

/// Every QB program with a dynamic array read its descriptor's segment word
/// through `inttoptr`, which 3c4b31af6 took for a forged selector: no static
/// was split, and sum_three's loop kept `icmp slt` where the pin ends on
/// `icmp eq`. An `inttoptr` the frontend marks `!llrm.descriptor` makes the
/// array's own segment.
#[test]
fn test_module_ipo_splits_a_static_while_a_body_reads_a_descriptor_selector() {
    use llrm_mir::program::ProgramPass;
    let mut module = parsed(
        "@n = internal global [2 x i8] zeroinitializer

define i16 @f(i16 %a) norecurse {
b0:
  store i16 %a, ptr @n
  %x = load i16, ptr @n
  %y = add i16 %x, 1
  ret i16 %y
}

define i8 @g(ptr %d) {
b0:
  %w = getelementptr i8, ptr %d, i16 2
  %o = load i16, ptr %w
  %s = inttoptr i16 %o to ptr addrspace(2), !llrm.descriptor !0
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %v = load volatile i8, ptr addrspace(1) %far
  ret i8 %v
}

!0 = !{}
",
    );
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::pin_tuned()),
        |program: &mut llrm_mir::program::Program| {
            let mut ipo =
                crate::walk::ModuleIpo { inline: crate::inline::Threshold::none(), cloning: Default::default() };
            ipo.run(program, &mut Default::default())
        },
    )
    .and_then(|done| done)
    .expect("ModuleIpo runs");
    let text = crate::testing::printed(&module);
    let body = text.split("define").nth(1).unwrap_or_default();
    assert!(!body.contains("ptr @n") && body.contains("alloca i16"), "{text}");
}

/// -Oz walked its calls as -Os: the threshold said `optsize` and never
/// `minsize`, so a call was held to `OptSizeThreshold` (50), not
/// `OptMinSizeThreshold` (5). `next` of `div_constants_long` was inlined 40
/// times and the object was 12% larger than the pin's.
#[test]
fn test_minsize_is_walked_as_a_minsize_caller() {
    use crate::inlinecost::CallerSize;
    use crate::pipeline::Options;
    assert_eq!(Options::min_size().inline.caller_size(), CallerSize::MinSize);
    assert_eq!(Options::size().inline.caller_size(), CallerSize::OptSize);
    assert_eq!(Options::default().inline.caller_size(), CallerSize::Speed);
}

/// A tree recursion (two calls of itself, a `report` at the leaf) with `extra`
/// more additions in its body.
fn tree_recursion(extra: usize) -> String {
    let mut body = String::new();
    let mut last = "%s".to_owned();
    for at in 0..extra {
        body += &format!("  %q{at} = shl i32 %n, {}\n  %p{at} = xor i32 {last}, %q{at}\n", at + 1);
        last = format!("%p{at}");
    }
    format!(
        "define internal i32 @tree(i32 %n) {{
b0:
  %small = icmp slt i32 %n, 2
  br i1 %small, label %base, label %rec

base:
  ret i32 %n

rec:
  %a = sub i32 %n, 1
  %b = sub i32 %n, 2
  %x = call i32 @tree(i32 %a)
  %y = call i32 @tree(i32 %b)
  %s = add i32 %x, %y
{body}  ret i32 {last}
}}

define i32 @main() {{
b0:
  %r = call i32 @tree(i32 30)
  ret i32 %r
}}
"
    )
}

/// Calls of `tree` after the default pipeline on the real-mode target, tuned
/// (`pin_tuned`: the pin's) or not.
fn calls_of_tree(
    extra: usize,
    pin: bool,
) -> usize {
    calls_of_tree_with(extra, pin, crate::inline::Threshold::default())
}

fn calls_of_tree_with(
    extra: usize,
    pin: bool,
    inline: crate::inline::Threshold,
) -> usize {
    calls_of("tree", &tree_recursion(extra), pin, inline)
}

/// Calls of `@name` in `text` after the default pipeline on the real-mode
/// target.
fn calls_of(
    name: &str,
    text: &str,
    pin: bool,
    inline: crate::inline::Threshold,
) -> usize {
    calls_of_copying(name, text, pin, inline, None)
}

/// The recursion REC-1 took at m16 before the target's entry was none: depth
/// 2, bodies of at most 14 `gcc_insns`.
const REC_1: Option<(u32, i64)> = Some((2, 14));

/// `calls_of` on a target that copies `recursion`.
fn calls_of_copying(
    name: &str,
    text: &str,
    pin: bool,
    inline: crate::inline::Threshold,
    recursion: Option<(u32, i64)>,
) -> usize {
    let mut module = parsed(text);
    let target = if pin { llrm_x86_m16::Dos::pin_tuned() } else { llrm_x86_m16::Dos::default() };
    let target = llrm_x86_m16::Dos { recursive_inlining: recursion, ..target };
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(target),
        |program: &mut llrm_mir::program::Program| {
            let mut applied = crate::pipeline::Applied::default();
            applied.options.inline = inline;
            crate::pipeline::applied(program, &applied)
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    let text = llrm_mir::print::module(&module);
    text.lines().filter(|line| line.contains("call") && line.contains(&format!("@{name}("))).count()
}

/// bench fib, rectwo and frames at m16 -O2 were 152, 259 and 338 bytes with a
/// small recursion copied into itself (REC-1, depth 2), past Borland C's 83,
/// 104 and 251: the real-mode target copies none by default,
/// `-finline-recursive` still does.
#[test]
fn test_the_real_mode_target_copies_no_tree_recursion_by_default() {
    assert!(calls_of_tree(0, false) <= 3, "copies of a tree recursion in itself: {}", calls_of_tree(0, false));
    let flagged = crate::inline::Threshold { recursive: Some(true), ..crate::inline::Threshold::default() };
    assert!(calls_of_tree_with(0, false, flagged) > 3, "control: the flag copies it");
}

/// A tree recursion (`@walk`) with `work` between its entry and its two
/// calls: nothing (`none`), a loop summing `%n` terms (`sum`), or a loop that
/// halves a value until it is below 2 (`halve`).
fn looped_recursion(work: &str) -> String {
    let (head, body) = match work {
        "sum" => (
            "  br label %loop

loop:
  %i = phi i32 [ 0, %rec ], [ %i1, %loop ]
  %t = phi i32 [ 0, %rec ], [ %t1, %loop ]
  %t1 = add i32 %t, %i
  %i1 = add i32 %i, 1
  %more = icmp slt i32 %i1, %n
  br i1 %more, label %loop, label %calls

calls:
",
            "%t",
        ),
        "halve" => (
            "  br label %loop

loop:
  %v = phi i32 [ %n, %rec ], [ %v1, %loop ]
  %v1 = lshr i32 %v, 1
  %more = icmp ugt i32 %v1, 1
  br i1 %more, label %loop, label %calls

calls:
",
            "%v1",
        ),
        _ => ("", "%n"),
    };
    format!(
        "define internal i32 @walk(i32 %n) {{
b0:
  %small = icmp slt i32 %n, 2
  br i1 %small, label %base, label %rec

base:
  ret i32 %n

rec:
{head}  %a = sub i32 %n, 1
  %b = sub i32 %n, 2
  %x = call i32 @walk(i32 %a)
  %y = call i32 @walk(i32 %b)
  %s = add i32 %x, %y
  %r = add i32 %s, {body}
  ret i32 %r
}}

define i32 @main() {{
b0:
  %r = call i32 @walk(i32 30)
  ret i32 %r
}}
"
    )
}

/// bench recchop at m16 -O2 ran 11310 clocks against 10242 without the copy
/// (main 10960): its loop between the entry and the calls is run again in
/// every copy, which saved only the calls. The budget is the callee's work per
/// call (`gcc_work`, a loop at its trips), so a loop-bearing recursion is
/// not copied, whatever the loop, and the same recursion without it is.
#[test]
fn test_the_real_mode_target_does_not_copy_a_recursion_whose_work_is_a_loop() {
    let on = crate::inline::Threshold::new(225);
    assert!(
        calls_of_copying("walk", &looped_recursion("none"), false, on, REC_1) > 3,
        "control: copied without the loop"
    );
    for work in ["sum", "halve"] {
        let calls = calls_of_copying("walk", &looped_recursion(work), false, on, REC_1);
        assert!(calls <= 3, "{work}: a recursion with a loop was copied into itself: {calls} calls");
    }
}

/// bench recchop at m16 -O2 ran 11310 clocks against 10242: the real-mode
/// target deferred its tail-call elimination for copies the callee cap refuses,
/// so the loop it makes was built late and its sum spilled. Only a body within
/// the cap waits.
#[test]
fn test_a_recursion_the_cap_refuses_keeps_its_tail_call_elimination_slot() {
    use llrm_mir::program::ProgramPass;
    let marked = |work: &str| {
        let mut module = parsed(&looped_recursion(work));
        llrm_mir::program::Program::lend(
            &mut module,
            std::rc::Rc::new(llrm_x86_m16::Dos { recursive_inlining: REC_1, ..llrm_x86_m16::Dos::default() }),
            |program: &mut llrm_mir::program::Program| {
                let mut walk = crate::walk::Walk {
                    promote_rate: None,
                    rerun_unchanged: true,
                    speed_rate: true,
                    inline: crate::inline::Threshold::new(225),
                    simplify: Box::new(|_, _, _, _| {}),
                };
                walk.run(program, &mut Default::default())
            },
        )
        .and_then(|done| done)
        .expect("the walk runs");
        llrm_mir::print::module(&module).contains(crate::tailcallelim::DEFERRED)
    };
    assert!(marked("none"), "control: a body within the cap waits");
    assert!(!marked("halve"), "a loop-bearing body waits for copies it will not get");
}

/// `walk` over `n` with `loop_body` per trip of a loop before its two calls
/// (none: fib's), run through the pipeline at the unpinned DOS target, which
/// prices the copies; the calls of it left.
fn calls_left_of_a_priced_recursion(loop_body: Option<&str>) -> usize {
    let Some(loop_body) = loop_body else {
        return calls_left_in(&parsed(FIB_WALK));
    };
    calls_left_in(&parsed(&format!(
        "define i32 @walk(i32 %n, ptr %p) {{
b0:
  %small = icmp slt i32 %n, 2
  br i1 %small, label %base, label %loop

base:
  ret i32 %n

loop:
  %i = phi i32 [ 0, %b0 ], [ %i1, %loop ]
  %acc = phi i32 [ 0, %b0 ], [ %acc1, %loop ]
{loop_body}
  %i1 = add nsw i32 %i, 1
  %more = icmp slt i32 %i1, %n
  br i1 %more, label %loop, label %rec

rec:
  %a = sub i32 %n, 1
  %b = sub i32 %n, 2
  %x = call i32 @walk(i32 %a, ptr %p)
  %y = call i32 @walk(i32 %b, ptr %p)
  %s = add i32 %x, %y
  %t = add i32 %s, %acc1
  ret i32 %t
}}

define i32 @main(ptr %p) {{
b0:
  %r = call i32 @walk(i32 30, ptr %p)
  ret i32 %r
}}
"
    )))
}

const FIB_WALK: &str = "define i32 @walk(i32 %n, ptr %p) {
b0:
  %small = icmp slt i32 %n, 2
  br i1 %small, label %base, label %rec

base:
  ret i32 %n

rec:
  %a = sub i32 %n, 1
  %b = sub i32 %n, 2
  %x = call i32 @walk(i32 %a, ptr %p)
  %y = call i32 @walk(i32 %b, ptr %p)
  %s = add i32 %x, %y
  ret i32 %s
}

define i32 @main(ptr %p) {
b0:
  %r = call i32 @walk(i32 30, ptr %p)
  ret i32 %r
}
";

fn calls_left_in(source: &llrm_mir::module::Module) -> usize {
    let mut module = source.clone();
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos { recursive_inlining: REC_1, ..llrm_x86_m16::Dos::default() }),
        |program: &mut llrm_mir::program::Program| {
            let mut options = crate::pipeline::Options::default();
            options.inline.recursive = Some(true);
            crate::pipeline::applied(program, &crate::pipeline::Applied { options, ..Default::default() })
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    llrm_mir::print::module(&module).matches("call i32 @walk").count()
}

/// A flat 16-bit recursion's copies were sized by the literals 14 and 2: a
/// body with a loop in it was copied like fib (quicksort c: 522 B at 306644
/// clocks against 389 B at 283450 uncopied). A level is made only when the
/// calls it removes are worth four times its bytes, so the loop-bodied
/// recursion keeps its two calls and the call-bound one is copied.
#[test]
fn test_a_recursion_with_a_loop_in_it_is_not_copied_for_its_calls() {
    let flat = calls_left_of_a_priced_recursion(None);
    assert!(flat > 2, "the flat recursion was not copied: {flat} calls");
    let heavy = calls_left_of_a_priced_recursion(Some(
        "  %q = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %q
  %w = mul i32 %v, %i
  %z = xor i32 %w, %acc
  store i32 %z, ptr %q
  %acc1 = add i32 %z, %v",
    ));
    assert!(heavy == 2, "the loop-bodied recursion was copied: {heavy} calls");
}

const CALL_BOUND: &str = "define i32 @fib(i32 %n) {
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
";

const LOOP_BODIED: &str = "define i32 @walk(i32 %n, ptr %p) {
b0:
  %small = icmp slt i32 %n, 2
  br i1 %small, label %base, label %scan

base:
  ret i32 %n

scan:
  %i = phi i32 [ 0, %b0 ], [ %next, %scan ]
  %acc = phi i32 [ 0, %b0 ], [ %sum, %scan ]
  %slot = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %slot
  %sum = add i32 %acc, %v
  %next = add i32 %i, 1
  %more = icmp slt i32 %next, %n
  br i1 %more, label %scan, label %rec

rec:
  %a = sub i32 %n, 1
  %b = sub i32 %n, 2
  %x = call i32 @walk(i32 %a, ptr %p)
  %y = call i32 @walk(i32 %b, ptr %p)
  %s = add i32 %x, %y
  %t = add i32 %s, %sum
  ret i32 %t
}
";

/// The depth of copies `priced_copy_depth` gives `name` in `text`, at m16's
/// sheet and the ceilings (3, 150).
fn priced_depth(
    text: &str,
    name: &str,
) -> usize {
    let mut module = parsed(text);
    let mut depth = 0;
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| {
            let module = &program.modules[0];
            let id = module
                .functions()
                .find(|(_, global, _)| global.name.as_deref() == Some(name))
                .map(|(id, _, _)| id)
                .unwrap();
            let frequencies = crate::walk::self_calls(module, &program.layout, id, false).expect("a tree");
            let tti = program.target.tti().expect("a cost model");
            depth = crate::walk::priced_copy_depth(
                module,
                &program.layout,
                &*program.target,
                &*tti,
                id,
                &frequencies,
                (3, 150),
            );
            Ok::<(), String>(())
        },
    )
    .and_then(|done| done)
    .expect("the program lends");
    depth
}

/// The first level of copies is made where its clocks are worth `COPY_MARGIN`
/// times its bytes: a call-bound tree (fib) takes copies, a body whose time is
/// a loop (the call overhead is a small share of it) takes none.
#[test]
fn test_the_priced_depth_copies_a_call_bound_tree_and_not_a_loop_bodied_one() {
    let (bound, looped) = (priced_depth(CALL_BOUND, "fib"), priced_depth(LOOP_BODIED, "walk"));
    assert!(bound >= 1, "a call-bound tree got {bound} levels");
    assert_eq!(looped, 0, "a loop-bodied tree got {looped} levels");
}

/// parity/branch and parity/loop c at m16 -O2 kept a static scratch variable
/// (`demo_value`, 2 and 4 bytes of .bss against 0 in main) because the call
/// that took its address stayed a call; inlined, the variable is promoted and
/// the global is gone.
#[test]
fn test_an_o2_inline_at_m16_leaves_no_global_for_a_scratch_variable_whose_address_was_passed() {
    let text = "@demo = internal global i16 0
@low = internal global i16 -13
@high = internal global i16 21

define i32 @kernel(ptr %p) {
b0:
  %v = load i16, ptr %p
  %neg = icmp slt i16 %v, 0
  %w = sext i16 %v to i32
  br i1 %neg, label %n, label %q
n:
  %a = mul i32 %w, 7
  %r1 = add i32 %a, 3
  ret i32 %r1
q:
  %b = mul i32 %w, 5
  %r2 = sub i32 %b, 9
  ret i32 %r2
}

define i32 @top() {
b0:
  %x = load volatile i16, ptr @low
  store i16 %x, ptr @demo
  %first = call i32 @kernel(ptr @demo)
  %y = load volatile i16, ptr @high
  store i16 %y, ptr @demo
  %second = call i32 @kernel(ptr @demo)
  %m = mul i32 %first, 1000
  %s = add i32 %m, %second
  ret i32 %s
}
";
    let m16 = at_o2_on(llrm_x86_m16::Dos::default(), text, "@top(");
    assert!(!m16.contains("call i32 @kernel"), "the calls stayed:\n{m16}");
    let mut module = parsed(text);
    llrm_mir::program::Program::lend(
        &mut module,
        std::rc::Rc::new(llrm_x86_m16::Dos::default()),
        |program: &mut llrm_mir::program::Program| {
            let mut options = crate::pipeline::Options::default();
            options.inline.recursive = None;
            crate::pipeline::applied(program, &crate::pipeline::Applied { options, ..Default::default() })
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    assert!(!llrm_mir::print::module(&module).contains("@demo"), "demo is still a global");
}

/// Calls of `@fib` after the default pipeline on x86-m32 with `options`.
fn m32_fib_calls(options: crate::pipeline::Options) -> usize {
    m32_tree_calls(0, options)
}

fn m32_tree_calls(
    extra: usize,
    options: crate::pipeline::Options,
) -> usize {
    let mut module =
        parsed(&tree_recursion(extra).replace("@tree", "@fib").replace("define internal i32 @fib", "define i32 @fib"));
    llrm_mir::program::Program::lend(
        &mut module,
        crate::indvars::tests::m32(),
        |program: &mut llrm_mir::program::Program| {
            crate::pipeline::applied(program, &crate::pipeline::Applied { options, ..Default::default() })
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    llrm_mir::print::module(&module).matches("call i32 @fib").count()
}

/// bench fib at m32 -O2 was 334151 clocks against gcc's 210636: the pin's
/// inliner copies no recursive callee. gcc's `recursive_inlining` copies it to
/// depth 8 (REC-2); m32 stops at depth 2 at -O2 (REC-7: fib 99618 -> 19993
/// clocks), so 6 calls remain.
#[test]
fn test_m32_copies_a_tree_recursion_into_itself_at_speed() {
    let calls = m32_fib_calls(crate::pipeline::Options::default());
    assert_eq!(calls, 6, "-O2 left {calls} calls of fib");
}

/// `-fno-inline-recursive` copied nothing on x86-m32 and changed no byte: the
/// target states bounds, so `Recursion::of` took them whatever the flag said
/// (fib -O2: the 6 calls of the default stood with the flag off, AM-8's D10-off
/// A/B had no switch). Off leaves the calls the pin's inliner leaves; the
/// default and `-finline-recursive` still copy.
#[test]
fn test_no_inline_recursive_copies_no_recursion_where_the_target_copies_by_default() {
    let with = |flag: Option<bool>| {
        let mut options = crate::pipeline::Options::default();
        options.inline.recursive = flag;
        m32_fib_calls(options)
    };
    assert_eq!(with(None), 6, "the default copies");
    assert_eq!(with(Some(true)), 6, "-finline-recursive is unchanged where the target copies");
    let off = with(Some(false));
    assert!(off < 6, "-fno-inline-recursive left {off} calls of fib, the default's 6");
}

/// Calls of `@paths` after the default pipeline on x86-m32: bench rectwo's
/// lattice-path count, `paths(n-1, m) + paths(n, m-1)` with two ways out.
fn m32_paths_calls(options: crate::pipeline::Options) -> usize {
    let mut module = parsed(
        "define i32 @paths(i32 %n, i32 %m) {
b0:
  %zn = icmp eq i32 %n, 0
  %zm = icmp eq i32 %m, 0
  %done = or i1 %zn, %zm
  br i1 %done, label %base, label %rec

base:
  ret i32 1

rec:
  %a = sub i32 %n, 1
  %b = sub i32 %m, 1
  %x = call i32 @paths(i32 %a, i32 %m)
  %y = call i32 @paths(i32 %n, i32 %b)
  %s = add i32 %x, %y
  ret i32 %s
}

define i32 @main() {
b0:
  %r = call i32 @paths(i32 9, i32 9)
  ret i32 %r
}
",
    );
    llrm_mir::program::Program::lend(
        &mut module,
        crate::indvars::tests::m32(),
        |program: &mut llrm_mir::program::Program| {
            crate::pipeline::applied(program, &crate::pipeline::Applied { options, ..Default::default() })
        },
    )
    .and_then(|done| done)
    .expect("the pipeline runs");
    llrm_mir::print::module(&module).matches("call i32 @paths").count()
}

/// bench rectwo at m32 -O3 ran 1230664 clocks against gcc's 555904: the copies
/// were made of the two-call body and kept 43 calls; gcc eliminates the tail
/// call first and nests the loop it leaves (466986 clocks that way). The body
/// here is the same shape. -O2 keeps the tree's copies (fib's pure calls merge
/// across them: 99618 clocks against 228901).
#[test]
fn test_m32_o3_paths_is_copied_as_nested_loops_not_as_a_tree_of_calls() {
    let calls = m32_paths_calls(crate::pipeline::Options::speed());
    assert!(calls <= 8, "paths kept {calls} calls of itself");
}

/// fib at m32 -O2 was copied into itself to the 150 `gcc_insns` the pass
/// allowed and ran 240256 clocks against gcc's 210636 (gcc's cap is 450; -O2
/// takes it, 99687 clocks). -O3 took the same copies with the loop first and
/// depth 8 and lost to -O2 on fib, frames and rectwo (REC-8): it copies as -O2
/// does.
#[test]
fn test_m32_o3_copies_fib_as_o2_does() {
    let o2 = m32_fib_calls(crate::pipeline::Options::default());
    let o3 = m32_fib_calls(crate::pipeline::Options::speed());
    assert_eq!(o2, o3, "-O2 made {o2} calls of fib, -O3 {o3}");
}

/// fib at -O1 was copied into itself (334152 -> 424014 clocks, 69 -> 649 B):
/// gcc inlines no function at -O1, recursive ones included.
#[test]
fn test_m32_copies_no_recursion_at_o1() {
    let calls = m32_fib_calls(crate::pipeline::Options::basic());
    assert!(calls <= 3, "fib was copied into itself at -O1, {calls} calls of it");
}

/// -Os is strict bytes: quicksort, bintree and hanoi are the same object with
/// D10 on or off at m32 and x86_64 -Os (247, 229, 47 B), so no recursive copy
/// is made there (the rate levels' 1261 B shape never reaches -Os).
#[test]
fn test_m32_copies_no_recursion_at_os() {
    let calls = m32_tree_calls(6, crate::pipeline::Options::size());
    assert!(calls <= 3, "a recursion was copied into itself at -Os, {calls} calls of it");
}

/// gcc rejects the recursive edge of quicksort and bintree at -O2
/// ("max-inline-insns-auto limit reached", growth 25-28 and 15-22 against 15)
/// and takes it at -O3 (30): a body that grows past 15 is copied at -O3 only.
#[test]
fn test_m32_copies_a_recursion_whose_edge_grows_past_15_only_at_o3() {
    let o2 = m32_tree_calls(6, crate::pipeline::Options::default());
    let o3 = m32_tree_calls(6, crate::pipeline::Options::speed());
    assert!(o2 <= 3, "copied at -O2, {o2} calls");
    assert!(o3 >= 8, "not copied at -O3, {o3} calls");
}

/// gcc's dump sizes fib at 12 (edge growth 6; 13 and 7 here, before the
/// pipeline merges its returns): the bar of the recursive edge
/// is read in those units, not in `gcc_insns` (fib 8, quicksort 18 against
/// gcc's 34, which let quicksort and bintree grow 490 -> 1218 and 449 -> 1464
/// bytes at m32 -O2).
#[cfg(feature = "gcc-gpl")]
#[test]
fn test_gcc_size_counts_a_branch_a_call_and_its_arguments_as_gcc_does() {
    let module = parsed(&tree_recursion(0).replace("@tree", "@fib"));
    let (_, _, fib) =
        module.functions().find(|(_, _, f)| !f.is_declaration() && f.parameters().len() == 1).expect("fib");
    assert_eq!((llrm_gpl_gcc::gcc_size(fib), llrm_gpl_gcc::recursive_edge_growth(fib)), (13, 7));
}

/// bench bintree's `insert` (three parameters, gcc size 22, edge growth 16 by
/// gcc's dump) was peeled into its caller at m32 -O2 (code 257 -> 299 bytes)
/// where gcc rejects it ("max-inline-insns-auto limit reached"): the call
/// term of the bar scaled with the parameters, 10 for three, which gave it
/// growth 12.
#[cfg(feature = "gcc-gpl")]
#[test]
fn test_the_edge_growth_of_a_three_parameter_recursion_is_its_size_less_the_call() {
    let module = parsed(
        "define void @insert(ptr %t, i32 %at, i32 %node) {
b0:
  %n3 = mul i32 %node, 3
  %p = getelementptr i32, ptr %t, i32 %n3
  %k = load i32, ptr %p
  %a3 = mul i32 %at, 3
  %q = getelementptr i32, ptr %t, i32 %a3
  %k2 = load i32, ptr %q
  %lt = icmp slt i32 %k, %k2
  %side = select i1 %lt, i32 1, i32 2
  %i = add i32 %a3, %side
  %s = getelementptr i32, ptr %t, i32 %i
  %c = load i32, ptr %s
  %z = icmp eq i32 %c, 0
  br i1 %z, label %put, label %rec

put:
  store i32 %node, ptr %s
  ret void

rec:
  call void @insert(ptr %t, i32 %c, i32 %node)
  ret void
}
",
    );
    let (_, _, insert) = module.functions().next().expect("insert");
    let size = llrm_gpl_gcc::gcc_size(insert);
    assert_eq!(llrm_gpl_gcc::recursive_edge_growth(insert), size - 6);
    assert!(size - llrm_gpl_gcc::recursive_edge_growth(insert) == 6, "the call term scales with parameters");
}

/// gcc-derived code (`llrm-gpl-gcc`, GPL-3.0-or-later) is optional and off: a
/// default build must link none of it. Reads the manifests: the dependency is
/// `optional`, no `default` feature set names `gcc-gpl`, and no other crate
/// depends on the crate.
#[test]
fn test_the_default_build_links_no_gcc_derived_crate() {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = here.join("../../..");
    let transforms = std::fs::read_to_string(here.join("Cargo.toml")).unwrap();
    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let dependency =
        transforms.lines().find(|line| line.starts_with("llrm-gpl-gcc = ")).expect("the optional dependency");
    assert!(dependency.contains("optional = true"), "llrm-gpl-gcc is a plain dependency: {dependency}");
    for (name, text) in [("llrm-transforms", &transforms), ("the workspace root", &workspace)] {
        let default = text.lines().find(|line| line.trim_start().starts_with("default = ")).unwrap_or("");
        assert!(!default.contains("gcc-gpl"), "{name} enables gcc-gpl by default: {default}");
    }
    for entry in std::fs::read_dir(root.join("crates"))
        .unwrap()
        .flatten()
        .flat_map(|group| std::fs::read_dir(group.path()).unwrap().flatten())
    {
        let manifest = entry.path().join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else { continue };
        let owner = entry.file_name();
        if owner == "llrm-gpl-gcc" || owner == "llrm-transforms" {
            continue;
        }
        assert!(!text.contains("llrm-gpl-gcc"), "{} depends on the GPL crate", manifest.display());
    }
}

/// The bar of the recursive edge (DEVIATE REC-2) as the 12 self-recursive
/// functions of the corpus meet it: gcc 15 copies fib, hanoi, recpow and
/// rectwo into themselves at -O2 (edge growth <= 15) and at -O3 (<= 30) also
/// bintree `walk`, recchop `chop`, x_hanoi2 `mv`, quicksort `sort` and queens
/// `place`, never x_mergesort `msort` (growth 67). Fitted on these, `gcc_insns`
/// of queens 22 sits on the m32 O3 edge.
#[test]
fn test_the_m32_edge_bar_makes_gccs_decision_for_every_corpus_recursion() {
    // (module, function, copied at -O2, copied at -O3)
    let table: [(&str, &str, &str, bool, bool); 10] = [
        ("fib", include_str!("testdata/rec2/fib.ll"), "fib_", true, true),
        ("hanoi", include_str!("testdata/rec2/hanoi.ll"), "hanoi_", true, true),
        ("recpow", include_str!("testdata/rec2/recpow.ll"), "power_", true, true),
        ("rectwo", include_str!("testdata/rec2/rectwo.ll"), "paths_", true, true),
        ("bintree", include_str!("testdata/rec2/bintree.ll"), "walk_", false, true),
        ("recchop", include_str!("testdata/rec2/recchop.ll"), "chop_", false, true),
        ("x_hanoi2", include_str!("testdata/rec2/x_hanoi2.ll"), "mv_", false, true),
        ("quicksort", include_str!("testdata/rec2/quicksort.ll"), "sort_", false, true),
        ("queens", include_str!("testdata/rec2/queens.ll"), "place_", false, true),
        ("x_mergesort", include_str!("testdata/rec2/x_mergesort.ll"), "msort_", false, false),
    ];
    let m32 = crate::indvars::tests::m32();
    let (o2_bar, o3_bar) = m32.tti().and_then(|tti| tti.recursive_edge_growth()).expect("m32 states the bar");
    for (name, text, function, o2, o3) in table {
        let module = parsed(text);
        let (_, _, body) = module
            .functions()
            .find(|(_, global, _)| global.name.as_deref() == Some(function))
            .unwrap_or_else(|| panic!("{name}: no {function}"));
        let insns = crate::inline::recursive_edge_growth(body);
        assert_eq!(
            (insns <= o2_bar, insns <= o3_bar),
            (o2, o3),
            "{name} {function}: gcc_insns {insns}, bars {o2_bar}/{o3_bar}"
        );
    }
}

/// A speed trial of three calls to one dying callee credited the callee's bytes
/// three times (the sum ran over the sites): 7 calls to a 139-byte callee
/// counted 973 bytes gone, the trial read -338 B of growth where the built
/// code grew 496, and the priced m68k rule lost 22 KB at -O2.
#[test]
fn test_a_trial_credits_a_dying_callee_once_however_many_sites_it_inlines() {
    let module = parsed(
        "define internal i32 @h(i32 %x) {
b1:
  %a = add i32 %x, 1
  %b = mul i32 %a, 3
  ret i32 %b
}

define i32 @f(i32 %x) {
b1:
  %p = call i32 @h(i32 %x)
  %q = call i32 @h(i32 %p)
  %r = call i32 @h(i32 %q)
  ret i32 %r
}
",
    );
    let h = module.globals.iter().position(|global| global.name.as_deref() == Some("h")).expect("h");
    let h = llrm_mir::context::GlobalId(h as u32);
    let one = super::dying_callees_bytes(&module, &[h], |body| body.walk().count() as i64);
    let three = super::dying_callees_bytes(&module, &[h, h, h], |body| body.walk().count() as i64);
    assert_eq!(one, 3);
    assert_eq!(three, one, "the callee goes once");
}

/// m68k sfm16_float -Os: five calls to `canon`, 60 instructions against 57 and
/// a credit of 4, so the count accepted the copies; the byte estimate said +28
/// B (built +28). A callee that goes with its last calls must not grow the
/// bytes.
#[test]
fn test_an_os_trial_that_grows_the_bytes_is_refused_though_the_count_accepts_it() {
    assert!(!super::trial_accepted(true, false, true, false, false, 60, 57, 4, 0, 28, 0, 756));
    assert!(
        super::trial_accepted(true, false, true, false, false, 60, 57, 4, 0, -4, 0, 756),
        "a copy that shrinks the code stays"
    );
    assert!(
        super::trial_accepted(false, false, true, false, false, 60, 57, 4, 0, 28, 0, 756),
        "off the gate's targets the count decides"
    );
    assert!(
        super::trial_accepted(true, false, true, false, false, 57, 57, 4, 0, 12, 0, 756),
        "copies no larger than the caller are decided by the count"
    );
}

/// z80 frame_address_stack_args -O2: `kernel` (external, so it stays) copied
/// into `line` saved 123 clocks and 63 B, and the priced trial refused it for
/// want of a callee that goes (455 T became 7,594 T). A copy that shrinks the
/// module and the clocks stays; one that grows it without a callee going is
/// still refused (jpeg `decode`, +441k instructions).
#[test]
fn test_a_priced_trial_that_shrinks_the_module_stays_without_a_dying_callee() {
    assert!(super::trial_accepted(true, false, true, true, false, 72, 79, 0, 123, -63, 3000, 756));
    assert!(!super::trial_accepted(true, false, true, true, false, 72, 79, 0, 123, 10, 3000, 756));
    assert!(
        !super::trial_accepted(true, false, true, true, false, 72, 79, 0, 0, -63, 3000, 756),
        "no clocks saved, no copy"
    );
}

/// arm64 -O3 jpeg: a priced trial kept copies that left the module no larger
/// without a callee going, 5,828 -> 7,324 B and +1.2M instructions. Only a
/// target that asks keeps them.
#[test]
fn test_a_shrinking_copy_without_a_dying_callee_stays_only_where_the_target_asks() {
    assert!(super::trial_accepted(true, false, true, true, false, 72, 79, 0, 123, -63, 3000, 756));
    assert!(!super::trial_accepted(true, false, false, true, false, 72, 79, 0, 123, -63, 3000, 756));
}

/// Whether `self_calls` takes `name` of `text` as a tree recursion.
fn is_tree(
    text: &str,
    name: &str,
) -> bool {
    let mut module = parsed(text);
    let mut tree = false;
    llrm_mir::program::Program::lend(
        &mut module,
        crate::indvars::tests::m32(),
        |program: &mut llrm_mir::program::Program| {
            let module = &program.modules[0];
            let id = module
                .functions()
                .find(|(_, global, _)| global.name.as_deref() == Some(name))
                .map(|(id, _, _)| id)
                .unwrap();
            tree = crate::walk::self_calls(module, &program.layout, id, false).is_some();
            Ok::<(), String>(())
        },
    )
    .and_then(|done| done)
    .expect("the program lends");
    tree
}

/// bench queens at m32 -O3 ran 154117 clocks against gcc's 94851: `place`
/// calls itself once, in a loop, and was not a tree, so it was never copied
/// into itself. `f(n - 1) + n` (one call, once per entry) stays linear.
#[test]
fn test_one_self_call_in_a_loop_is_a_tree_recursion_and_one_per_entry_is_not() {
    assert!(is_tree(include_str!("testdata/rec2/queens.ll"), "place_"), "place_ is not a tree");
    let linear = "define i32 @sum(i32 %n) {
b0:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %base, label %rec

base:
  ret i32 0

rec:
  %a = sub i32 %n, 1
  %x = call i32 @sum(i32 %a)
  %s = add i32 %x, %n
  ret i32 %s
}
";
    assert!(!is_tree(linear, "sum"), "sum is a tree");
}

/// A target that has not opted in prices `llvm.lifetime.*` as the calls they
/// are in the IR (the inlining bars of x86 m16 were fit so; pricing them 0
/// there stopped `sum` being inlined into `top` in
/// `an_os_call_on_stack_records`), one that has prices them 0.
#[test]
fn test_free_intrinsics_are_priced_only_where_a_target_asks() {
    let module = parsed(
        "declare void @llvm.lifetime.start.p0(i64, ptr)\ndefine i32 @f() {\n  %a = alloca i32\n  call void @llvm.lifetime.start.p0(i64 4, ptr %a)\n  ret i32 0\n}\n",
    );
    let function = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap().2;
    let layout = llrm_mir::datalayout::DataLayout::default();
    let priced = |free_intrinsics| {
        let costs = llrm_mir::target::OperationCosts { call: 5, argument: 2, free_intrinsics, ..Default::default() };
        crate::walk::function_bytes(&module, &layout, &costs, function)
    };
    assert_eq!(priced(false) - priced(true), 5 + 2 * 2, "the marker call and its two arguments");
}

/// `llvm.smul.fix.sat` was priced as a call (12 B) at every CPU while it
/// expands to a widening multiply, shifts, compares and selects: fixed_point's
/// objects are 2.9 kB at the 68000 and 1.6 kB at the 68020 against an estimate
/// of 1.6 kB at both. A target that opts in prices it on its own table, the
/// product from `multiply_widening`.
#[test]
fn test_fixed_point_intrinsics_are_priced_as_their_expansion_where_a_target_asks() {
    let module = parsed(
        "declare i32 @llvm.smul.fix.sat.i32(i32, i32, i32 immarg)\ndefine i32 @f(i32 %a, i32 %b) {\n  %r = call i32 @llvm.smul.fix.sat.i32(i32 %a, i32 %b, i32 8)\n  ret i32 %r\n}\n",
    );
    let function = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap().2;
    let layout = llrm_mir::datalayout::DataLayout::parse("E-m:e-p:32:16:32-i32:16:32-n8:16:32").unwrap();
    let priced = |expanded_intrinsics, multiply_widening| {
        let costs = llrm_mir::target::OperationCosts {
            call: 5,
            argument: 2,
            add: 2,
            shift: 2,
            extend: 2,
            branch: 2,
            r#move: 2,
            multiply: 14,
            multiply_widening,
            expanded_intrinsics,
            ..Default::default()
        };
        crate::walk::function_bytes(&module, &layout, &costs, function)
    };
    let as_a_call = priced(false, 0);
    assert!(priced(true, 8) > as_a_call, "the expansion is more than a call and its three arguments");
    assert_eq!(priced(true, 160) - priced(true, 8), 160 - 8, "the product is the table's `multiply_widening`");
}

/// A product of two values extended from the word is the CPU's widening
/// multiply, not a double-word product: the walker prices `mul i64 (sext i32),
/// (sext i32)` at `multiply_widening` and a plain `mul i64` at `multiply_wide`.
#[test]
fn test_a_product_of_two_extended_words_is_the_widening_product() {
    let module = parsed(
        "define i64 @f(i32 %a, i32 %b) {\n  %x = sext i32 %a to i64\n  %y = sext i32 %b to i64\n  %r = mul i64 %x, %y\n  ret i64 %r\n}\ndefine i64 @g(i64 %a, i64 %b) {\n  %r = mul i64 %a, %b\n  ret i64 %r\n}\n",
    );
    let by_name =
        |name: &str| module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).unwrap().2;
    let layout = llrm_mir::datalayout::DataLayout::parse("E-m:e-p:32:16:32-i32:16:32-n8:16:32").unwrap();
    let costs =
        llrm_mir::target::OperationCosts { extend: 2, multiply_wide: 100, multiply_widening: 10, ..Default::default() };
    let (widening, wide) = (
        crate::walk::function_bytes(&module, &layout, &costs, by_name("f")),
        crate::walk::function_bytes(&module, &layout, &costs, by_name("g")),
    );
    assert_eq!(widening - wide, 2 * 2 + 10 - 100, "two extensions and the widening product, against the wide product");
}

/// jpeg_fixed Nib at m16 -O2: `next_byte` is a few hot instructions and a long
/// cold refill block; inlined at 36 loop sites (#7817) the code went 4305 ->
/// 9398 B and the clocks rose 561k, for the call it saved. The cold block's
/// bytes buy no clocks: the call stays.
#[test]
fn test_a_helper_whose_bulk_is_a_cold_block_stays_a_call_at_m16_o2() {
    let text = "@g = global i16 0

define i16 @next_byte(i16 %x) {
b0:
  %p = load volatile i16, ptr @g
  %z = icmp ne i16 %p, 0
  br i1 %z, label %hot, label %cold

hot:
  ret i16 %p

cold:
  %c0 = load volatile i16, ptr @g
  %c1 = load volatile i16, ptr @g
  %c2 = load volatile i16, ptr @g
  %c3 = load volatile i16, ptr @g
  %c4 = load volatile i16, ptr @g
  %c5 = load volatile i16, ptr @g
  %c6 = load volatile i16, ptr @g
  %c7 = load volatile i16, ptr @g
  %c8 = load volatile i16, ptr @g
  %c9 = load volatile i16, ptr @g
  %c10 = load volatile i16, ptr @g
  %c11 = load volatile i16, ptr @g
  %c12 = load volatile i16, ptr @g
  %c13 = load volatile i16, ptr @g
  %s0 = add i16 %x, %c0
  %s1 = add i16 %s0, %c1
  %s2 = add i16 %s1, %c2
  %s3 = add i16 %s2, %c3
  %s4 = add i16 %s3, %c4
  %s5 = add i16 %s4, %c5
  %s6 = add i16 %s5, %c6
  %s7 = add i16 %s6, %c7
  %s8 = add i16 %s7, %c8
  %s9 = add i16 %s8, %c9
  %s10 = add i16 %s9, %c10
  %s11 = add i16 %s10, %c11
  %s12 = add i16 %s11, %c12
  %s13 = add i16 %s12, %c13
  ret i16 %s13
}

define i16 @top(i16 %n) {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %j, %loop ]
  %a = call i16 @next_byte(i16 %i)
  %b = call i16 @next_byte(i16 %a)
  %c = call i16 @next_byte(i16 %b)
  store volatile i16 %c, ptr @g
  %j = add i16 %i, 1
  %k = icmp slt i16 %j, %n
  br i1 %k, label %loop, label %done

done:
  ret i16 %c
}";
    let m16 = at_o2_on(llrm_x86_m16::Dos::default(), text, "@top(");
    assert!(m16.contains("call i16 @next_byte"), "the calls were inlined:\n{m16}");
}

/// A target stating a size view: `Viewed(has_view, is_view, measured)`. The
/// view prices a memory access at seven units where the target prices one.
struct Viewed(bool, bool, bool);

impl llrm_mir::tti::Tti for Viewed {
    fn inline_size_view(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        self.0.then(|| std::rc::Rc::new(Viewed(false, true, false)) as std::rc::Rc<dyn llrm_mir::tti::Tti>)
    }

    fn inline_size_view_is_measured(&self) -> bool {
        self.2
    }

    fn cmp_sel_cost(
        &self,
        kind: llrm_mir::tti::CmpSel,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cmp_sel_cost(kind, ty, cost)
    }

    fn memory_op_cost(
        &self,
        op: llrm_mir::tti::MemoryOp,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
        volatile: bool,
    ) -> llrm_mir::tti::Cost {
        let one = llrm_mir::tti::Basic.memory_op_cost(op, ty, cost, volatile);
        if self.1 { one * 7 } else { one }
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        value: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.int_imm_cost(value, bits)
    }

    fn int_imm_cost_inst(
        &self,
        user: llrm_mir::tti::ImmUser,
        index: u32,
        value: i128,
        bits: u32,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.int_imm_cost_inst(user, index, value, bits)
    }

    fn is_truncate_free(
        &self,
        from: llrm_mir::tti::Ty,
        to: llrm_mir::tti::Ty,
    ) -> bool {
        llrm_mir::tti::Basic.is_truncate_free(from, to)
    }

    fn is_legal_addressing_mode(
        &self,
        global: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        llrm_mir::tti::Basic.is_legal_addressing_mode(global, offset, base, scale)
    }

    fn cast_cost(
        &self,
        kind: llrm_mir::tti::CastKind,
        from: llrm_mir::tti::Ty,
        to: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.cast_cost(kind, from, to, cost)
    }

    fn arithmetic_cost(
        &self,
        op: llrm_mir::tti::ArithOp,
        ty: llrm_mir::tti::Ty,
        cost: llrm_mir::tti::CostKind,
        lhs: llrm_mir::tti::OperandInfo,
        rhs: llrm_mir::tti::OperandInfo,
    ) -> llrm_mir::tti::Cost {
        llrm_mir::tti::Basic.arithmetic_cost(op, ty, cost, lhs, rhs)
    }
}

/// The SZ2 trials priced a body in any target's size view, the 6502's and
/// m68k's included: `tail_calls` -Os at 6502 and 65C02 came out different
/// from the tip (a `_even -> _odd` site kept or refused on other sizes, the
/// object changed). Only a measured view, for a speed caller, prices a trial.
#[test]
fn test_a_trial_body_is_priced_in_a_view_only_when_it_is_measured_and_the_caller_is_speed() {
    let module = parsed(
        "define i32 @f(ptr %p) {
b0:
  %v = load i32, ptr %p
  ret i32 %v
}
",
    );
    let id = module.named("f").unwrap();
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap_or("")).unwrap();
    let size = |tti: &Viewed, speed: bool| super::body_size(&module, &layout, tti, id, speed);
    let own = size(&Viewed(false, false, false), true);
    assert_eq!(size(&Viewed(true, false, false), true), own, "an unmeasured view is not used (6502, m68k)");
    assert_eq!(size(&Viewed(true, false, false), false), own);
    assert_eq!(size(&Viewed(true, false, true), false), own, "a measured view is not used for a size caller");
    assert!(size(&Viewed(true, false, true), true) > own, "a measured view serves a speed caller");
}

/// `memcpy_libcall` -Os on the 6502: both sites of a private `same` were
/// refused by three instruction units (78 against 62 + 13 for the callee) while
/// a byte model said the copies shrink the module.
#[test]
fn a_trial_decided_by_bytes_keeps_the_copies_that_shrink_the_module() {
    assert!(
        super::trial_accepted(false, true, false, false, false, 78, 62, 13, 0, -23, 0, 400),
        "smaller: kept whatever the units say"
    );
    assert!(
        !super::trial_accepted(false, true, false, false, false, 60, 62, 13, 0, 4, 0, 400),
        "larger: refused whatever the units say"
    );
    assert!(!super::trial_accepted(false, true, false, false, false, 62, 62, 13, 0, 0, 0, 400), "no gain: refused");
}

/// Two Os trials that tie to the byte were both refused, whatever the copies
/// save: a tied pair is decided by the clocks, and a tie that saves none stays
/// as written. Bytes never lose to clocks.
#[test]
fn a_trial_whose_bytes_tie_is_decided_by_the_clocks_it_saves() {
    assert!(super::trial_accepted(false, true, false, false, false, 62, 62, 13, 40, 0, 0, 400), "tied, faster: kept");
    assert!(
        !super::trial_accepted(false, true, false, false, false, 62, 62, 13, 0, 0, 0, 400),
        "tied, no faster: refused"
    );
    assert!(
        !super::trial_accepted(false, true, false, false, false, 62, 62, 13, 9999, 4, 0, 400),
        "larger by a byte: refused however fast"
    );
}

/// DEVIATE INL-SPEED-OS: jpeg_fixed -Os `receive` and `extend` (two sites each,
/// 200,699 runs a call, +1 and -1 bytes): the latency model priced each call as
/// its callee's body, so the trial read 0 clocks saved and stayed refused, 375
/// k calls left. The call and the return the copy drops count; a cold site, or
/// one that grows the code past 4 B a site, still does not.
#[test]
fn a_hot_copy_the_model_prices_at_nothing_is_kept_for_the_call_it_drops() {
    use super::speed_keeps_at_size;
    assert!(speed_keeps_at_size(16, 200_699.5, 0, 2, 1));
    assert!(speed_keeps_at_size(16, 200_699.5, 0, 2, -1));
    assert!(!speed_keeps_at_size(16, 1.0, 0, 2, 1), "a cold site");
    assert!(!speed_keeps_at_size(16, 200_699.5, 0, 2, 9), "9 B for two sites");
    assert!(!speed_keeps_at_size(16, 200_699.5, -4, 2, 1), "the copy costs clocks");
}

fn m32_costs() -> std::rc::Rc<dyn llrm_mir::target::Machine> {
    use llrm_target::Target;
    let prices = llrm_target::CpuPrices {
        costs: Vec::new(),
        prefix: 1,
        address_stall: 0,
        registers: 6,
        call_registers: 3,
        float_registers: 0,
        address_forms: Vec::new(),
        operations: llrm_mir::target::OperationCosts::default(),
        spaces: llrm_x86_m32::layout().spaces.roles,
        private: None,
        calling: None,
        foreign: llrm_target::Foreign::none(),
        ports: llrm_x86_m32::M32.flat_ports(),
    };
    (llrm_x86_m32::M32.cost_model())(&prices)
}

/// x_idioms -Os (m32): `sumlong(L, 40)` called twice. IPSCCP leaves both
/// parameters unread and `deadargelim` drops them after the walk (the pin's
/// order), so the -Os byte trial must price the calls as they ship: priced
/// with the pushes it inlined both (+5 B at m32, +10 B at m68k 68000; gcc
/// keeps two calls).
#[test]
fn test_the_size_trial_prices_calls_without_the_parameters_nothing_reads() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
@L = global [40 x i32] zeroinitializer
define internal i32 @sumlong(ptr %p, i16 %n) {
b0:
  br label %head
head:
  %i = phi i16 [ 0, %b0 ], [ %i1, %body ]
  %s = phi i32 [ 0, %b0 ], [ %s1, %body ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %body, label %out
body:
  %x = sext i16 %i to i32
  %w = add i32 %x, 1
  %a = getelementptr i32, ptr %p, i32 %x
  %v = load i32, ptr %a
  %m = mul i32 %w, %v
  %s1 = add i32 %s, %m
  %i1 = add i16 %i, 1
  br label %head
out:
  ret i32 %s
}

define i32 @run() {
b0:
  %a = call i32 @sumlong(ptr @L, i16 40)
  store i32 7, ptr @L
  %b = call i32 @sumlong(ptr @L, i16 40)
  %r = add i32 %a, %b
  ret i32 %r
}
";
    let mut module = parsed(text);
    let applied = crate::pipeline::Applied { options: crate::pipeline::Options::size(), ..Default::default() };
    llrm_mir::program::Program::lend(&mut module, m32_costs(), |program: &mut llrm_mir::program::Program| {
        crate::pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .expect("the pipeline runs");
    let printed = llrm_mir::print::module(&module);
    let run = printed.split("define").find(|one| one.contains("@run(")).expect("run");
    assert_eq!(run.matches("@sumlong(").count(), 2, "{printed}");
}

/// hanoi -O2: its three pegs are permuted and never read. The recursion
/// copier works on the body without them, as gcc's IPA-SRA leaves it: copies
/// merge the two calls into one and the function becomes a closed form (261 B
/// at m32). With the pegs in place through the walk it kept two calls to
/// itself (+23 B at m32, +2358 B at 6502).
#[test]
fn test_a_recursion_loses_its_unread_parameters_before_it_is_copied() {
    let text = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define internal i32 @hanoi(i32 %n, i32 %a, i32 %b, i32 %c) {
b0:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %out, label %rec
rec:
  %m = sub i32 %n, 1
  %x = call i32 @hanoi(i32 %m, i32 %a, i32 %c, i32 %b)
  %y = call i32 @hanoi(i32 %m, i32 %c, i32 %b, i32 %a)
  %s = add i32 %x, %y
  %t = add i32 %s, 1
  ret i32 %t
out:
  ret i32 0
}

define i32 @top(i32 %n) {
b0:
  %r = call i32 @hanoi(i32 %n, i32 1, i32 3, i32 2)
  ret i32 %r
}
";
    let mut module = parsed(text);
    let applied = crate::pipeline::Applied { options: crate::pipeline::Options::standard(), ..Default::default() };
    llrm_mir::program::Program::lend(&mut module, m32_costs(), |program: &mut llrm_mir::program::Program| {
        crate::pipeline::applied(program, &applied)
    })
    .and_then(|done| done)
    .expect("the pipeline runs");
    let printed = llrm_mir::print::module(&module);
    let top = printed.split("define").find(|one| one.contains("@top(")).expect("top");
    assert!(top.matches("call ").count() <= 1, "{printed}");
}

/// arm64 queens -O2: `place` was cloned per constant `row` and the clones
/// inlined (252 -> 2,260 B; -O3 308 -> 832 B), 18/20 cells at or below the
/// bars against 19/19 without, and the clone made frames worse on both axes.
/// The target states no default clones: the output is what `-fno-ipa-cp-clone`
/// gives.
#[test]
fn test_arm64_makes_no_ipa_cp_clones_unless_asked() {
    let run = |cp_clone: Option<bool>| {
        let mut module = parsed(ROW_RECURSION);
        let mut options = crate::pipeline::Options::standard();
        options.inline.cp_clone = cp_clone;
        let applied = crate::pipeline::Applied { options, ..Default::default() };
        llrm_mir::program::Program::lend(
            &mut module,
            std::rc::Rc::new(llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux }),
            |program: &mut llrm_mir::program::Program| crate::pipeline::applied(program, &applied),
        )
        .and_then(|done| done)
        .expect("the pipeline runs");
        llrm_mir::print::module(&module)
    };
    let asked_not = run(Some(false));
    assert_eq!(run(None), asked_not);
    assert_ne!(run(Some(true)), asked_not, "the premise: the clones change this module");
}

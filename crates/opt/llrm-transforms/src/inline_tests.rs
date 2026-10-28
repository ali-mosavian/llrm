//! Adapted from llrm-codegen's `optimize/inline_tests.rs`, the port of
//! `tests/test_inline.py`, each body now MIR text.
//!
//! Skipped: `test_inline_refuses_a_live_unmodelled_call_result`, since a call
//! has one result, what `ret` returns. The old file skipped
//! `test_small_private_pure_helpers_inline_in_mir` and
//! `test_tiny_private_leaf_inlines_at_two_call_sites` for want of the cfront
//! optimizer; both now read as MIR below.

use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::interpret::{self, Val};
use llrm_mir::module::Module;

use super::*;
use crate::testing::{parsed, printed};

fn id(
    module: &Module,
    name: &str,
) -> GlobalId {
    module.named(name).unwrap_or_else(|| panic!("no @{name}"))
}

/// Every call in `caller` that `candidates` admits, inlined.
fn inline_into(
    module: &mut Module,
    caller: &str,
    call: i64,
) -> bool {
    inline_with(module, caller, call, Threshold::default())
}

fn inline_with(
    module: &mut Module,
    caller: &str,
    _call: i64,
    _threshold: Threshold,
) -> bool {
    let layout = DataLayout::default();
    let by = Caller {
        layout: &layout,
        recursive: recursive(module).contains(&id(module, caller)),
        self_inlining: None,
        peeling: false,
    };
    let mut changed = false;
    loop {
        let available = every_body(module, &layout);
        let mut declared = llrm_mir::passes::Declared::of(module);
        let (context, function, metadata) = module.function_and_metadata_mut(caller).unwrap();
        let spliced = expanded_all(context, function, &by, &available, None, &mut declared, metadata).unwrap();
        declared.place(module).unwrap();
        if spliced == 0 {
            return changed;
        }
        changed = true;
    }
}

/// Every defined, non-recursive body as a candidate: what `inlinecost::sites`
/// decides is the walk's, these tests are of the splice.
fn every_body(
    module: &Module,
    layout: &DataLayout,
) -> IndexMap<GlobalId, Candidate> {
    let recursive = recursive(module);
    module
        .functions()
        .filter(|(id, _, function)| !function.is_declaration() && !recursive.contains(id))
        .map(|(id, _, function)| {
            (
                id,
                Candidate {
                    body: std::rc::Rc::new(function.clone()),
                    frame: grown(&module.context, layout, function),
                    moved: false,
                    last_call: false,
                    callee: None,
                    self_calls: None,
                    self_sites: 0,
                },
            )
        })
        .collect()
}

fn run(
    module: &Module,
    name: &str,
    arguments: &[u128],
) -> Val {
    let arguments = arguments.iter().map(|&bits| Val::Int { bits, width: 16 }).collect();
    interpret::run(module, name, arguments, 10_000).expect("runs")
}

const LEAF: &str = "define internal i16 @leaf() {
b1:
  ret i16 37
}
";

#[test]
fn test_inline_splices_return_before_the_original_successor_phi() {
    let mut module = parsed(&format!(
        "{LEAF}
define i16 @main() {{
b1:
  %result = call i16 @leaf()
  br label %b4

b4:
  %joined = phi i16 [ %result, %b1 ]
  ret i16 %joined
}}
"
    ));
    assert!(inline_into(&mut module, "main", 1));
    assert_eq!(
        printed(&module),
        "define internal i16 @leaf() {
b1:
  ret i16 37
}

define i16 @main() {
b1:
  br label %0

0:
  br label %b4

b4:
  %joined = phi i16 [ 37, %0 ]
  ret i16 %joined
}
"
    );
}

/// x_reverse m16 -O1 grew 3 bytes (AM-IP-7): the callee's return block, laid
/// out before its loop, stayed as an empty jump, and `merged` later pulled the
/// code after the call into it, ahead of the loop. `InlineFunction` splices a
/// single return into the code after the call, which keeps its place.
#[test]
fn test_inline_keeps_the_code_after_the_call_after_the_callee_loop() {
    let mut module = parsed(
        "define internal void @spin(i16 %n) {
b1:
  br label %b3

b2:
  ret void

b3:
  %i = phi i16 [ 0, %b1 ], [ %j, %b3 ]
  %j = add i16 %i, 1
  %c = icmp ne i16 %j, %n
  br i1 %c, label %b3, label %b2
}

define i16 @main(i16 %n) {
b1:
  call void @spin(i16 %n)
  %r = add i16 %n, 7
  ret i16 %r
}
",
    );
    assert!(inline_into(&mut module, "main", 1));
    crate::cfg::merged(module.function_mut("main").unwrap().1);
    let text = printed(&module);
    let main = &text[text.find("define i16 @main").expect("main")..];
    let (looped, after) = (main.find("phi i16").expect("the loop"), main.find("add i16 %n, 7").expect("the add"));
    assert!(looped < after, "{main}");
    assert_eq!(run(&module, "main", &[3]), Val::Int { bits: 10, width: 16 });
}

/// Value ids number each function's arena from zero: the caller's actual
/// and the callee's unrelated definition share an id.
#[test]
fn test_inline_binds_an_actual_whose_id_is_a_callee_definitions() {
    let mut module = parsed(
        "define internal i16 @leaf(i16 %formal) {
b1:
  %unrelated = add i16 99, 0
  ret i16 %formal
}

define i16 @main(i16 %p) {
b1:
  %actual = add i16 %p, 7
  %result = call i16 @leaf(i16 %actual)
  ret i16 %result
}
",
    );
    assert!(inline_into(&mut module, "main", 1));
    assert!(printed(&module).contains("  ret i16 %actual\n"), "{}", printed(&module));
    assert_eq!(run(&module, "main", &[5]), Val::Int { bits: 12, width: 16 });
}

const HELPERS: &str = "define internal i16 @scale(i16 %x) {
b1:
  %y = shl i16 %x, 2
  %z = add i16 %y, 1
  ret i16 %z
}

define internal i16 @clamp(i16 %x) {
b1:
  %big = icmp sgt i16 %x, 100
  br i1 %big, label %b2, label %b3

b2:
  ret i16 100

b3:
  ret i16 %x
}

define i16 @main(i16 %a, i16 %b) {
b1:
  %s = call i16 @scale(i16 %a)
  %c = call i16 @clamp(i16 %s)
  %t = call i16 @scale(i16 %b)
  %sum = add i16 %c, %t
  ret i16 %sum
}
";

#[test]
fn test_small_private_pure_helpers_inline_and_keep_their_results() {
    let original = parsed(HELPERS);
    let mut module = parsed(HELPERS);
    assert!(inline_into(&mut module, "main", 4));
    let text = printed(&module);
    assert!(!text.contains("call "), "{text}");
    // Two returns meet in a phi at the continuation.
    assert!(text.contains("phi i16 [ 100, "), "{text}");
    for (a, b) in [(0, 0), (3, 4), (30, 7), (65535, 1)] {
        assert_eq!(run(&module, "main", &[a, b]), run(&original, "main", &[a, b]), "{a} {b}");
    }
}

#[test]
fn test_a_callee_writing_through_an_argument_inlines_with_its_store() {
    let text = "define internal void @put(ptr %p, i16 %v) {
b1:
  %w = add i16 %v, 1
  store i16 %w, ptr %p
  ret void
}

define i16 @main(i16 %a) {
b1:
  %cell = alloca i16
  store i16 0, ptr %cell
  call void @put(ptr %cell, i16 %a)
  %r = load i16, ptr %cell
  ret i16 %r
}
";
    let original = parsed(text);
    let mut module = parsed(text);
    assert!(inline_into(&mut module, "main", 4));
    assert!(!printed(&module).contains("call "), "{}", printed(&module));
    for a in [0, 7, 0xffff] {
        assert_eq!(run(&module, "main", &[a]), run(&original, "main", &[a]), "{a}");
    }
}

#[test]
fn test_a_callee_frame_cell_inlined_into_a_loop_lands_in_the_callers_entry() {
    let text = "define internal i16 @bump(i16 %x) {
b1:
  %cell = alloca i16
  store i16 %x, ptr %cell
  %v = load i16, ptr %cell
  %w = add i16 %v, 3
  ret i16 %w
}

define i16 @main(i16 %n) {
b1:
  br label %b2

b2:
  %i = phi i16 [ 0, %b1 ], [ %next, %b2 ]
  %next = call i16 @bump(i16 %i)
  %go = icmp ult i16 %next, %n
  br i1 %go, label %b2, label %b3

b3:
  ret i16 %next
}
";
    let original = parsed(text);
    let mut module = parsed(text);
    assert!(inline_into(&mut module, "main", 4));
    let text = printed(&module);
    let main = text.split("define i16 @main").nth(1).unwrap();
    assert!(
        main.split("b2:").next().unwrap().contains("alloca"),
        "a frame cell in the loop grows the stack each trip: {text}"
    );
    assert_eq!(main.matches("alloca").count(), 1);
    for n in [0, 5, 300] {
        assert_eq!(run(&module, "main", &[n]), run(&original, "main", &[n]), "{n}");
    }
}

#[test]
fn test_inlining_inside_a_loop_retargets_the_self_edge() {
    let text = "define internal i16 @step(i16 %x) {
b1:
  %y = add i16 %x, 3
  ret i16 %y
}

define i16 @main(i16 %n) {
b1:
  br label %b2

b2:
  %i = phi i16 [ 0, %b1 ], [ %next, %b2 ]
  %next = call i16 @step(i16 %i)
  %go = icmp ult i16 %next, %n
  br i1 %go, label %b2, label %b3

b3:
  ret i16 %next
}
";
    let original = parsed(text);
    let mut module = parsed(text);
    assert!(inline_into(&mut module, "main", 1));
    assert!(!printed(&module).contains("call "));
    for n in [0, 1, 10, 300] {
        assert_eq!(run(&module, "main", &[n]), run(&original, "main", &[n]), "{n}");
    }
}

#[test]
fn test_call_counts_count_direct_calls_to_defined_functions() {
    let module = parsed(&format!(
        "declare i16 @external()\n\n{HELPERS}\ndefine i16 @more() {{\nb1:\n  %x = call i16 @external()\n  %y = call i16 @scale(i16 %x)\n  ret i16 %y\n}}\n"
    ));
    let counts = call_counts(&module);
    assert_eq!(counts.get(&id(&module, "scale")), Some(&3));
    assert_eq!(counts.get(&id(&module, "clamp")), Some(&1));
    assert_eq!(counts.get(&id(&module, "external")), None);
}

/// A callee whose frame is `size` bytes, called from `main`, which the loop
/// calls `sites` times.
fn framed(
    size: u32,
    sites: usize,
) -> String {
    let calls: String = (0..sites).map(|at| format!("  %r{at} = call i16 @peek(i16 {at})\n")).collect();
    let sum: String = (1..sites)
        .map(|at| {
            format!("  %s{at} = add i16 {}, %r{at}\n", if at == 1 { "%r0".to_owned() } else { format!("%s{}", at - 1) })
        })
        .collect();
    format!(
        "define i16 @peek(i16 %k) {{\nb1:\n  %buf = alloca [{size} x i8]\n  %p = getelementptr i8, ptr %buf, i16 %k\n  store i8 1, ptr %p\n  %v = load i8, ptr %p\n  %w = zext i8 %v to i16\n  ret i16 %w\n}}\n\ndefine i16 @main() {{\nb1:\n{calls}{sum}  ret i16 %s{}\n}}\n",
        sites - 1
    )
}

#[test]
fn test_a_frame_is_not_copied_into_a_recursive_function() {
    // Each level of a recursive walk reserved the callee's 300 bytes.
    let text = framed(8, 1).replace("define i16 @main() {\nb1:\n  %r0 = call i16 @peek(i16 0)\n  ret i16 %s0\n}", "define i16 @main(i16 %n) {\nb1:\n  %r0 = call i16 @peek(i16 %n)\n  %m = call i16 @main(i16 %n)\n  %t = add i16 %r0, %m\n  ret i16 %t\n}");
    let mut module = parsed(&text);
    assert!(!inline_into(&mut module, "main", 40), "{}", printed(&module));
}

/// A callee that only reads memory takes a `byval` argument as the caller's own
/// pointer (LLVM's HandleByValArgument): the call is gone. One that may write
/// it gets a copy of the object: the pointer would be the caller's object, not
/// the callee's own.
#[test]
fn test_a_byval_argument_inlines_into_a_callee_that_only_reads_and_not_into_one_that_writes() {
    let text = |attrs: &str, body: &str| {
        format!(
            "define internal i16 @peek(ptr byval([4 x i8]) %p) {attrs} {{
b1:
{body}
}}

define i16 @main() {{
b1:
  %cell = alloca [4 x i8]
  store i16 3, ptr %cell
  %r = call i16 @peek(ptr byval([4 x i8]) %cell)
  ret i16 %r
}}
"
        )
    };
    let reads = text("memory(read)", "  %v = load i16, ptr %p\n  ret i16 %v");
    let mut module = parsed(&reads);
    assert!(inline_into(&mut module, "main", 8));
    assert!(!printed(&module).contains("call "), "{}", printed(&module));
    // One that may write it is inlined on a copy: an alloca the object is
    // memcpy'd into, which the body writes instead.
    let writes = text("", "  store i16 9, ptr %p\n  %v = load i16, ptr %p\n  ret i16 %v");
    let mut module = parsed(&writes);
    assert!(inline_into(&mut module, "main", 8));
    let printed = printed(&module);
    assert!(!printed.contains("call i16 @peek("), "{printed}");
    assert!(printed.contains("llvm.memcpy"), "{printed}");
    assert_eq!(printed.matches("alloca").count(), 2, "the object and its copy: {printed}");
}

/// gcc's size unit (`estimate_num_insns`, `eni_size_weights`): a compare only a
/// conditional branch reads is that branch, an address only an access reads
/// is the access, a return is one, a phi and a plain jump none. Counting the
/// compare and its branch as two put `chop` (gcc: growth 14) at 17 against a
/// limit of 15.
#[test]
fn test_gcc_insns_counts_a_compare_and_its_branch_once_and_a_return() {
    let module = parsed(
        "define i32 @f(i32 %n, ptr %p) {
b0:
  %a = icmp slt i32 %n, 2
  br i1 %a, label %b1, label %b2
b1:
  ret i32 0
b2:
  %g = getelementptr inbounds i32, ptr %p, i32 %n
  %v = load i32, ptr %g
  %b = icmp eq i32 %v, 5
  br i1 %b, label %b3, label %b4
b3:
  ret i32 1
b4:
  %c = icmp eq i32 %v, 6
  %z = zext i1 %c to i32
  ret i32 %z
}
",
    );
    let body = module.global(llrm_mir::context::GlobalId(0)).function().expect("a body");
    // br, ret, load, br, ret, icmp (read by a zext), zext, ret
    assert_eq!(super::gcc_insns(body), 8);
    let chain = parsed(
        "define i32 @g(i32 %n) {
b0:
  %a = icmp slt i32 %n, 2
  br i1 %a, label %b1, label %b2
b1:
  ret i32 0
b2:
  %b = icmp slt i32 %n, 4
  br i1 %b, label %b3, label %b4
b3:
  ret i32 1
b4:
  %c = icmp slt i32 %n, 6
  br i1 %c, label %b5, label %b6
b5:
  ret i32 2
b6:
  ret i32 3
}
",
    );
    let body = chain.global(llrm_mir::context::GlobalId(0)).function().expect("a body");
    // Three branches and four returns; the ordinary count has three compares
    // and three branches and no return.
    assert_eq!((super::gcc_insns(body), super::operations(body)), (7, 6));
}

/// Jump threading counts the blocks it copies in the same unit: the same rules
/// over a chosen set of instructions, not the body.
#[test]
fn test_gcc_insns_in_counts_the_chosen_instructions_by_the_same_rules() {
    let module = parsed(
        "define i32 @f(i32 %n, ptr %p) {
b0:
  %a = icmp slt i32 %n, 2
  br i1 %a, label %b1, label %b2
b1:
  ret i32 0
b2:
  %g = getelementptr inbounds i32, ptr %p, i32 %n
  %v = load i32, ptr %g
  ret i32 %v
}
",
    );
    let body = module.global(llrm_mir::context::GlobalId(0)).function().expect("a body");
    let blocks: Vec<_> = {
        let mut seen = Vec::new();
        for (block, _) in body.walk() {
            if !seen.contains(&block) {
                seen.push(block);
            }
        }
        seen
    };
    let of = |index: usize| {
        let insts: Vec<_> = body.walk().filter(|&(block, _)| block == blocks[index]).map(|(_, inst)| inst).collect();
        super::gcc_insns_in(body, insts)
    };
    assert_eq!((of(0), of(1), of(2)), (1, 1, 2));
}

/// A routine that frees its argument (`releases`) is copied only where the
/// caller owns what it passes: a local temporary, not a pointer it was given
/// (the copy would free it twice, once as the caller's and once as the
/// callee's).
#[test]
fn a_routine_that_frees_a_temporary_is_not_inlined_at_a_call_of_one() {
    let text = "define internal i16 @take(ptr releases %p) {
b1:
  store i16 1, ptr %p
  ret i16 0
}

define i16 @given(ptr %pp) {
b1:
  %q = load ptr, ptr %pp
  %r = call i16 @take(ptr %q)
  ret i16 %r
}

define i16 @local() {
b1:
  %t = alloca i16
  %r = call i16 @take(ptr %t)
  ret i16 %r
}
";
    let mut module = parsed(text);
    assert!(!inline_into(&mut module, "given", 1), "{}", printed(&module));
    assert!(inline_into(&mut module, "local", 1), "{}", printed(&module));
}

/// `noipa` keeps a body out of every copy.
#[test]
fn a_noipa_routine_is_never_inlined() {
    let text = |attribute: &str| format!("define internal i16 @leaf() {attribute} {{\nb1:\n  ret i16 37\n}}\n");
    let plain = parsed(&text(""));
    let noipa = parsed(&text("noipa"));
    let leaf = |module: &Module| module.functions().next().expect("@leaf").2.clone();
    assert!(copyable(&plain, &leaf(&plain)), "premise: the same routine without noipa is copyable");
    assert!(!copyable(&noipa, &leaf(&noipa)));
}

/// `noinline` outranks `alwaysinline` when the language states both.
#[test]
fn test_a_callee_the_language_says_never_inline_stays_even_if_always_is_stated_too() {
    let text = |attribute: &str| format!("define internal i16 @leaf() {attribute} {{\nb1:\n  ret i16 37\n}}\n");
    let leaf = |module: &Module| module.functions().next().expect("@leaf").2.clone();
    let always = parsed(&text("alwaysinline"));
    assert!(copyable(&always, &leaf(&always)), "premise: alwaysinline alone is copyable");
    let both = parsed(&text("alwaysinline noinline"));
    assert!(!copyable(&both, &leaf(&both)));
}

/// A function counting down by recursion, `ops` extra operations in its body.
fn countdown(ops: usize) -> String {
    let padding: String = (0..ops)
        .map(|at| {
            let before = if at == 0 { "%n".to_owned() } else { format!("%t{}", at - 1) };
            format!("  %t{at} = mul i32 {before}, 12345\n")
        })
        .collect();
    let last = if ops == 0 { "%n".to_owned() } else { format!("%t{}", ops - 1) };
    format!(
        "define i32 @down(i32 %n) {{
b0:
  %done = icmp eq i32 %n, 0
  br i1 %done, label %out, label %again

again:
{padding}  %m = sub i32 {last}, 1
  %r = call i32 @down(i32 %m)
  %s = add i32 %r, 7
  ret i32 %s

out:
  ret i32 0
}}
"
    )
}

/// `down` with the copies of itself `expanded_all` makes, the `add` in the
/// body one per copy and the original.
fn self_inlined(ops: usize) -> (usize, i64) {
    let mut module = parsed(&countdown(ops));
    let layout = DataLayout::default();
    let before = gcc_insns(module.function_mut("down").unwrap().1);
    let name = id(&module, "down");
    let itself = {
        let (context, function) = module.function_mut("down").unwrap();
        Candidate {
            body: std::rc::Rc::new(function.clone()),
            frame: grown(context, &layout, function),
            moved: false,
            last_call: false,
            callee: None,
            self_calls: None,
            self_sites: 0,
        }
    };
    let available = [(name, itself)].into_iter().collect();
    let frequencies = SparseIdMap::default();
    let by = Caller { layout: &layout, recursive: true, self_inlining: Some(&frequencies), peeling: false };
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function, metadata) = module.function_and_metadata_mut("down").unwrap();
    expanded_all(context, function, &by, &available, None, &mut declared, metadata).unwrap();
    declared.place(&mut module).unwrap();
    let text = printed(&module);
    (text.matches("add i32").count(), before)
}

/// Copies of a function in itself stop at gcc's `max-inline-recursive-depth`
/// (8): a small body gets 8 copies, 9 `add`s with the original. fib was
/// refused outright (base) or, once allowed, copied until 75 calls stood.
#[test]
fn test_a_small_recursive_function_is_copied_into_itself_to_the_depth_limit() {
    let (adds, _) = self_inlined(0);
    assert_eq!(adds, 1 + MAX_INLINE_RECURSIVE_DEPTH, "copies of down in down");
}

/// A body too big for 8 copies stops at `max-inline-insns-recursive` (450
/// `gcc_insns`): each copy adds its size less the call it replaces.
#[test]
fn test_copies_of_a_function_in_itself_stop_at_the_size_limit() {
    let (adds, before) = self_inlined(60);
    let copies = (MAX_INLINE_INSNS_RECURSIVE - before) / (before - 1);
    assert!(copies < MAX_INLINE_RECURSIVE_DEPTH as i64, "premise: the size, not the depth, stops it ({copies})");
    assert_eq!(adds as i64, 1 + copies, "copies of down in down, body of {before} insns");
}

/// A body's `tail` call rests on the body's own frame: inlined at a site that
/// is not `tail` it is not either. A `tail` call in `place` that took the
/// caller's array, once inlined, let BasicAA say it touched no slot of the
/// frame, and DSE deleted the stores the call read (queens -O3 ran 51% longer
/// on a wrong answer).
#[test]
fn a_tail_call_in_an_inlined_body_stays_tail_only_where_the_site_is() {
    let text = |site: &str| {
        format!(
            "declare void @g(ptr)

define void @callee(ptr %p) {{
b0:
  tail call void @g(ptr %p)
  ret void
}}

define void @f(ptr %q) {{
b0:
  {site}call void @callee(ptr %q)
  ret void
}}
"
        )
    };
    let in_f = |site: &str| {
        let mut module = parsed(&text(site));
        assert!(inline_into(&mut module, "f", 0), "premise: the call is inlined");
        let printed = printed(&module);
        printed[printed.find("define void @f(").expect("@f")..].to_owned()
    };
    assert!(!in_f("").contains("tail call void @g"), "{}", in_f(""));
    assert!(in_f("tail ").contains("tail call void @g"), "{}", in_f("tail "));
    // A `notail` call stays so (`ChildTCK != TCK_NoTail`).
    let mut module = parsed(&text("").replace("  tail call void @g", "  notail call void @g"));
    assert!(inline_into(&mut module, "f", 0));
    let printed = printed(&module);
    assert!(printed[printed.find("define void @f(").unwrap()..].contains("notail call void @g"), "{printed}");
}

/// An inlined body kept the callee's own locations, so a debugger saw `sq`'s
/// lines as `f`'s, with no inlined call: no `DW_TAG_inlined_subroutine`.
#[test]
fn test_inlined_instructions_are_located_at_the_call() {
    let mut module = parsed(
        "define internal i32 @sq(i32 %x) {
b1:
  %m = mul i32 %x, %x, !dbg !5
  ret i32 %m
}

define i32 @f(i32 %a) {
b1:
  %r = call i32 @sq(i32 %a), !dbg !6
  ret i32 %r
}

!1 = distinct !DISubprogram(name: \"sq\")
!2 = distinct !DISubprogram(name: \"f\")
!5 = !DILocation(line: 1, column: 40, scope: !1)
!6 = !DILocation(line: 2, column: 48, scope: !2)
",
    );
    assert!(inline_into(&mut module, "f", 0));
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    let mul = function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Binary { .. }))
        .unwrap();
    let (_, node) = function.instruction(mul).metadata.iter().find(|(kind, _)| kind == "dbg").unwrap();
    let special = &module.special[node];
    let at = special.fields.iter().position(|one| one == "inlinedAt").expect("an inlinedAt");
    let llrm_mir::MetadataOperand::Node(call) = module.metadata[node.0 as usize].operands[at] else { panic!() };
    let line = module.special[&call].fields.iter().position(|one| one == "line").expect("a line");
    let llrm_mir::MetadataOperand::Constant(line) = module.metadata[call.0 as usize].operands[line] else { panic!() };
    assert!(matches!(module.context.get(line).kind, llrm_mir::ConstantKind::Int(2)), "inlined at the call's line");
}

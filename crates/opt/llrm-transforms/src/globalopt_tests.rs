use llrm_mir::program::Program;

use crate::pipeline::{self, Applied};
use crate::testing::{parsed, printed, results};

/// A global only loaded is constant, and a load of it its initializer;
/// one another body stores stays a variable. fpdeep's BC_CN literals were
/// loaded, never folded.
#[test]
fn a_global_only_loaded_is_its_initializer() {
    let text = format!(
        "{}@c = internal global [2 x i16] [i16 5, i16 12]
@w = internal global i16 7

define i16 @f(i16 %x) {{
b0:
  %at = getelementptr i8, ptr @c, i16 2
  %a = load i16, ptr %at
  %b = load i16, ptr @w
  %s = add i16 %a, %b
  ret i16 %s
}}

define void @g(i16 %x) {{
b0:
  store i16 %x, ptr @w
  ret void
}}
",
        llrm_analysis::testing::DOS
    );
    let mut module = parsed(&text);
    let before = results(&module, &[&[0]]);
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[0]]), before, "{after}");
    // @c, folded into its one load, is dead and gone.
    assert!(!after.contains("@c = internal global") && after.contains("@w = internal unnamed_addr global"), "{after}");
    assert!(!after.contains("load i16, ptr %at") && after.contains("load i16, ptr @w"), "{after}");
}

/// `module` through the pipeline, printed.
fn piped(text: &str) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        pipeline::applied(program, &Applied::default())
    })
    .and_then(|done| done)
    .unwrap();
    printed(&module)
}

/// A global stored to and never loaded, forwarding having taken its one load,
/// kept its stores and itself (#1621): LLVM's GlobalOpt and gcc's ipa find it
/// dead. Both bodies lose the stores and the variable goes.
#[test]
fn a_global_only_stored_loses_its_stores_and_itself() {
    let after = piped(
        "@t = internal global [2 x i8] zeroinitializer
@out = global i8 0

define void @f() {
b0:
  store i8 8, ptr @t
  %at = getelementptr i8, ptr @t, i16 1
  store i8 9, ptr %at
  %a = load i8, ptr @t
  store volatile i8 %a, ptr @out
  ret void
}

define void @g(i8 %x) {
b0:
  store i8 %x, ptr @t
  ret void
}
",
    );
    assert!(!after.contains("@t"), "a store or the variable is left: {after}");
}

/// What something loads, escapes or must see keeps its stores.
#[test]
fn a_global_that_is_loaded_escapes_or_is_stored_volatile_keeps_its_stores() {
    let loaded = piped(
        "@t = internal global i8 0
@out = global i8 0

define void @f(i8 %x) {
b0:
  store i8 %x, ptr @t
  ret void
}

define void @g() {
b0:
  %a = load i8, ptr @t
  store volatile i8 %a, ptr @out
  ret void
}
",
    );
    assert!(loaded.contains("store i8 %x, ptr @t"), "{loaded}");
    let escaped = piped(
        "@t = internal global i8 0
@slot = global ptr null
declare void @use(ptr)

define void @f(i8 %x) {
b0:
  store i8 %x, ptr @t
  call void @use(ptr @t)
  ret void
}
",
    );
    assert!(escaped.contains("store i8 %x, ptr @t"), "{escaped}");
    let volatile = piped(
        "@t = internal global i8 0

define void @f(i8 %x) {
b0:
  store volatile i8 %x, ptr @t
  ret void
}
",
    );
    assert!(volatile.contains("store volatile i8 %x, ptr @t"), "{volatile}");
}

/// tests/run/c/local_initializers.c: a local aggregate's initializer is a
/// `memcpy` from an internal global (the C front end's `.X`), which no load
/// took until SROA split the copy; GlobalOpt had run by then, so the global
/// stayed a variable and `main` kept its 100 instructions (58 -> 354 B at -O2)
/// where it was 12 before #3621. A `memcpy` from it is a read
/// (`AnalyzeGlobal`).
#[test]
fn a_global_only_copied_from_is_constant_and_its_copy_folds() {
    let after = piped(
        "@x = internal global [4 x i8] c\"\\05\\00\\0c\\00\"

declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)

define i16 @f() {
b0:
  %s = alloca [4 x i8]
  call void @llvm.memcpy.p0.p0.i16(ptr %s, ptr @x, i16 4, i1 false)
  %p = getelementptr i8, ptr %s, i16 2
  %v = load i16, ptr %p
  ret i16 %v
}
",
    );
    assert!(after.contains("ret i16 12") && !after.contains("call void @llvm.memcpy"), "{after}");
}

/// Each global asked about scanned every body again: `never_stored` made
/// globals x functions scans, 5.4 M of a `d_faces` compile in `reach` over 253
/// runs. One scan of a body serves every global.
#[test]
fn the_uses_of_every_global_are_found_in_one_scan_of_each_body() {
    let module = parsed(&format!(
        "{}@a = internal global i16 1
@b = internal global i16 2
@c = internal global i16 3

define i16 @f() {{
b0:
  %x = load i16, ptr @a
  %y = load i16, ptr @b
  %s = add i16 %x, %y
  ret i16 %s
}}

define i16 @g() {{
b0:
  %z = load i16, ptr @c
  ret i16 %z
}}
",
        llrm_analysis::testing::DOS
    ));
    let before = crate::globalopt::scans();
    let found = crate::globalopt::never_stored(&module);
    assert_eq!(found.len(), 3, "the three globals are only loaded");
    assert_eq!(crate::globalopt::scans() - before, 2, "one scan for each of the two bodies");
}

/// QB's `PTS$descriptor` names `PTS`, and nothing takes the descriptor: a
/// variable only an unreached variable names was still "named by another
/// initializer", so it kept its stores and its loads (udt.bas -Os m16: six
/// `mov dword ptr PTS+n, imm`, 24 bytes of data and `push dword ptr PTS+n`
/// where main pushes constants, +66 B).
#[test]
fn a_variable_only_an_unreached_descriptor_names_still_loses_its_stores() {
    let after = piped(
        "@a = internal global [8 x i8] zeroinitializer
@a.d = internal constant <{ ptr }> <{ ptr @a }>

define void @f() {
b0:
  store i32 5, ptr @a
  ret void
}
",
    );
    assert!(!after.contains("@a"), "the variable or its store stayed:\n{after}");
}

/// A descriptor some code takes keeps what it names.
#[test]
fn a_variable_a_reached_descriptor_names_keeps_its_stores() {
    let after = piped(
        "@a = internal global [8 x i8] zeroinitializer
@a.d = internal constant <{ ptr }> <{ ptr @a }>
@out = global ptr null

define void @f() {
b0:
  store i32 5, ptr @a
  store volatile ptr @a.d, ptr @out
  ret void
}
",
    );
    assert!(after.contains("store i32 5, ptr @a"), "a store to a variable a live descriptor names went:\n{after}");
}

fn marked(text: &str) -> String {
    let mut module = parsed(&format!("{}{}", llrm_analysis::testing::DOS, text));
    Program::lend(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default()), |program| {
        let mut manager = llrm_mir::passes::PassManager::default();
        manager.add_module(crate::globalopt::GlobalOpt);
        manager.run(program)
    })
    .and_then(|done| done)
    .unwrap();
    printed(&module)
}

/// A static table nothing compares or lets escape stayed `internal constant`:
/// the pin's GlobalOpt marks it `unnamed_addr`, and without that mark
/// ConstantMerge merges nothing (QCport's duplicate tables stayed two).
#[test]
fn test_a_table_only_loaded_is_marked_unnamed_addr() {
    let after = marked(
        "@t = internal constant [2 x i16] [i16 5, i16 12]
@w = internal global i16 7

define i16 @f(i16 %x) {
b0:
  %at = getelementptr i8, ptr @t, i16 2
  %a = load i16, ptr %at
  store i16 %a, ptr @w
  ret i16 %a
}
",
    );
    assert!(after.contains("@t = internal unnamed_addr constant"), "{after}");
    assert!(after.contains("@w = internal unnamed_addr global"), "{after}");
}

/// A compared address, an address passed to a call or stored, an address
/// another global holds, and an external global are not marked.
#[test]
fn test_a_compared_escaping_or_external_global_is_not_marked() {
    let after = marked(
        "@cmp = internal constant [2 x i16] [i16 1, i16 2]
@esc = internal constant [2 x i16] [i16 3, i16 4]
@sto = internal constant [2 x i16] [i16 5, i16 6]
@held = internal constant [2 x i16] [i16 7, i16 8]
@ext = constant [2 x i16] [i16 9, i16 10]
@holder = constant [1 x ptr] [ptr @held]
@slot = internal global ptr null
declare void @take(ptr)

define i1 @f(ptr %p) {
b0:
  %c = icmp eq ptr @cmp, %p
  call void @take(ptr @esc)
  store ptr @sto, ptr @slot
  %x = load i16, ptr @ext
  ret i1 %c
}
",
    );
    for name in ["cmp", "esc", "sto", "held", "ext"] {
        let line = after.lines().find(|one| one.starts_with(&format!("@{name} "))).unwrap_or_else(|| panic!("{after}"));
        assert!(!line.contains("unnamed_addr"), "{line}\n{after}");
    }
}

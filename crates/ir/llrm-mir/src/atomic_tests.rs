use super::{decode, hidden, lowered, lowered_single_thread, restored};
use crate::module::Module;
use crate::opcode::{AtomicKind, AtomicOrdering, Opcode};
use crate::parse;

const TEXT: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16\"
@flag = global i32 0
define i32 @f(i32 %v) {
entry:
  %a = load atomic i32, ptr @flag acquire, align 4
  store atomic i32 %v, ptr @flag release, align 4
  ret i32 %a
}
";

fn opcodes(module: &Module) -> Vec<Opcode> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    function.walk().map(|(_, inst)| function.instruction(inst).opcode.clone()).collect()
}

/// The pipeline saw `load atomic` and `store atomic` as themselves, which no
/// pass classifies: any of them could be dropped as pure. Hidden, each is a
/// call of `llrm.atomic.*`; the name says what it was.
#[test]
fn a_hidden_atomic_is_a_call_whose_name_says_what_it_was() {
    let mut module = parse::module(TEXT).unwrap();
    hidden(&mut module).unwrap();
    assert!(opcodes(&module).iter().all(|opcode| !matches!(opcode, Opcode::Atomic(_))));
    let names: Vec<&str> = module.globals.iter().filter_map(|global| global.name.as_deref()).collect();
    let load = names.iter().find(|name| name.starts_with("llrm.atomic.load")).copied().expect("a load");
    let info = decode(load).expect("decodes");
    assert_eq!((info.kind, info.ordering, info.align), (AtomicKind::Load, AtomicOrdering::Acquire, Some(4)));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
}

/// At x86 a load and a store short of `seq_cst` are the plain access, volatile
/// so nothing after the pipeline moves them.
#[test]
fn an_x86_atomic_load_and_store_lower_to_volatile_accesses() {
    let mut module = parse::module(TEXT).unwrap();
    hidden(&mut module).unwrap();
    lowered(&mut module, 32, true, false, false).unwrap();
    let kinds = opcodes(&module);
    assert!(kinds.contains(&Opcode::Load { align: Some(4), volatile: true }), "{kinds:?}");
    assert!(kinds.contains(&Opcode::Store { align: Some(4), volatile: true }), "{kinds:?}");
    assert!(!kinds.iter().any(|opcode| matches!(opcode, Opcode::Call(_))), "{kinds:?}");
    // llrm-llc stopped at "@llrm.atomic.load.acquire.-.sys.nv.a2.t0 is no
    // assembler symbol": the declaration stayed.
    assert!(
        module
            .globals
            .iter()
            .all(|global| !global.name.as_deref().is_some_and(|name| name.starts_with("llrm.atomic.")))
    );
}

/// Where the target has no acquire or release access, an ordered atomic is
/// refused: a plain `ldr` would not be one.
#[test]
fn an_ordered_atomic_is_refused_where_the_target_has_no_lowering() {
    let mut module = parse::module(TEXT).unwrap();
    hidden(&mut module).unwrap();
    let refused = lowered(&mut module, 32, false, false, false).unwrap_err();
    assert!(refused.contains("load acquire"), "{refused}");
}

const WIDE: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16\"
@w = global i64 0
define i64 @f(i64 %v, i64 %e) {
entry:
  %a = load atomic i64, ptr @w monotonic, align 8
  store atomic i64 %v, ptr @w release, align 8
  %b = atomicrmw add ptr @w, i64 %v seq_cst
  %c = cmpxchg ptr @w, i64 %e, i64 %v acq_rel monotonic
  %d = extractvalue { i64, i1 } %c, 0
  %s = add i64 %a, %b
  %t = add i64 %s, %d
  ret i64 %t
}
";

/// A 64-bit atomic at a target whose widest is 32 was refused ("`load
/// monotonic` of 64 bits is not supported": QuickrBasic's `pmou.c` did not
/// compile on m32). It is the runtime's `__atomic_*_8` call, as the pin's `llc`
/// makes it on a 486 (`AtomicExpandPass::expandAtomicOpToLibcall`), never two
/// plain loads.
#[test]
fn a_wide_atomic_is_a_libcall_not_a_refusal_or_a_split() {
    let mut module = parse::module(WIDE).unwrap();
    hidden(&mut module).unwrap();
    lowered(&mut module, 32, true, false, false).unwrap();
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    let called: Vec<&str> = module
        .globals
        .iter()
        .filter_map(|global| global.name.as_deref())
        .filter(|name| name.starts_with("__atomic_"))
        .collect();
    for want in ["__atomic_load_8", "__atomic_store_8", "__atomic_fetch_add_8", "__atomic_compare_exchange_8"] {
        assert!(called.contains(&want), "{want} in {called:?}");
    }
    // the compare-exchange's slot is read back by a plain load; no volatile one
    // stands for an atomic
    assert!(
        !opcodes(&module)
            .iter()
            .any(|opcode| matches!(opcode, Opcode::Load { volatile: true, .. } | Opcode::Atomic(_)))
    );
}

/// What the runtime has no function for (`umax`) stays refused by name: the
/// pass's compare-exchange loop is not translated.
#[test]
fn a_wide_unsigned_max_is_refused_by_name() {
    let text = "target datalayout = \"e-p:16:16-i32:16-i64:16\"
@w = global i64 0
define i64 @f(i64 %v) {
entry:
  %a = atomicrmw umax ptr @w, i64 %v monotonic
  ret i64 %a
}
";
    let mut module = parse::module(text).unwrap();
    hidden(&mut module).unwrap();
    let refused = lowered(&mut module, 32, true, false, false).unwrap_err();
    assert!(refused.contains("64 bits"), "{refused}");
}

const SWAP: &str = "target datalayout = \"e-p:16:16-i32:16-i64:16\"
@flag = global i32 0
define i32 @f(i32 %v) {
entry:
  %a = atomicrmw xchg ptr @flag, i32 %v acquire
  store atomic i32 %v, ptr @flag seq_cst, align 4
  fence acq_rel
  ret i32 %a
}
";

/// An exchange and a `seq_cst` store were refused at x86 ("is not lowered for
/// this target yet"): the runtime's `atomic_exchange_explicit` did not
/// compile. Where the target selects `xchg` they stay calls; the fence short
/// of `seq_cst` is gone.
#[test]
fn an_x86_exchange_and_a_seq_cst_store_stay_for_the_target_to_select() {
    let mut module = parse::module(SWAP).unwrap();
    hidden(&mut module).unwrap();
    lowered(&mut module, 32, true, true, false).unwrap();
    let calls = opcodes(&module).iter().filter(|opcode| matches!(opcode, Opcode::Call(_))).count();
    assert_eq!(calls, 2);
    let mut refused = parse::module(SWAP).unwrap();
    hidden(&mut refused).unwrap();
    assert!(lowered(&mut refused, 32, true, false, false).unwrap_err().contains("not lowered"));
}

/// The shadow harness handed the pin's `llc` the hidden calls, so it compared
/// our `ldar` with its `bl` to a function that does not exist. Restored, the
/// module prints as it was parsed.
#[test]
fn a_restored_module_prints_as_it_was() {
    let mut module = parse::module(TEXT).unwrap();
    let before = crate::print::module(&module);
    hidden(&mut module).unwrap();
    restored(&mut module).unwrap();
    assert!(opcodes(&module).iter().filter(|opcode| matches!(opcode, Opcode::Atomic(_))).count() == 2);
    let after = crate::print::module(&module);
    assert!(
        after.contains("load atomic i32, ptr @flag acquire")
            && after.contains("store atomic i32 %v, ptr @flag release"),
        "{after}"
    );
    assert!(!after.contains("call ") && before.contains("load atomic"), "{after}");
}

/// A target with no atomics (wasm without the feature) runs `LowerAtomicPass`:
/// wasm's selector has no atomic nodes, so the atomics stayed and selection
/// refused them.
#[test]
fn without_atomics_a_load_and_a_store_are_plain_and_a_fence_is_nothing() {
    let text = TEXT.replace("  ret i32 %a", "  fence seq_cst\n  ret i32 %a");
    let mut module = parse::module(&text).unwrap();
    assert!(opcodes(&module).iter().any(|opcode| matches!(opcode, Opcode::Atomic(_))), "the premise");
    lowered_single_thread(&mut module).unwrap();
    let kinds = opcodes(&module);
    assert!(kinds.contains(&Opcode::Load { align: Some(4), volatile: false }), "{kinds:?}");
    assert!(kinds.contains(&Opcode::Store { align: Some(4), volatile: false }), "{kinds:?}");
    assert!(!kinds.iter().any(|opcode| matches!(opcode, Opcode::Atomic(_))), "{kinds:?}");
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
}

/// A read-modify-write is not translated yet: refused by name, not lowered
/// wrongly.
#[test]
fn without_atomics_a_read_modify_write_is_refused() {
    let text = "target datalayout = \"e-p:32:32\"\n@flag = global i32 0\ndefine i32 @f(i32 %v) {\n  %a = atomicrmw add ptr @flag, i32 %v seq_cst\n  ret i32 %a\n}\n";
    let mut module = parse::module(text).unwrap();
    let why = lowered_single_thread(&mut module).unwrap_err();
    assert!(why.contains("is not lowered without atomics yet"), "{why}");
}

const LOCKED: &str = "target datalayout = \"e-p:32:32-i32:32\"
@w = global i32 0
define i32 @f(i32 %v, i32 %e) {
entry:
  %a = atomicrmw add ptr @w, i32 %v seq_cst
  %b = atomicrmw sub ptr @w, i32 %v monotonic
  atomicrmw or ptr @w, i32 %v monotonic
  %c = cmpxchg ptr @w, i32 %e, i32 %v seq_cst monotonic
  %d = extractvalue { i32, i1 } %c, 0
  %s = add i32 %a, %b
  %t = add i32 %s, %d
  ret i32 %t
}
";

fn kept_atomics(module: &Module) -> Vec<String> {
    let mut names: Vec<String> = module
        .globals
        .iter()
        .filter_map(|global| global.name.as_deref().map(str::to_owned))
        .filter(|name| decode(name).is_some())
        .collect();
    names.sort();
    names
}

/// x86 selected only `xchg`: `atomicrmw add` and `cmpxchg` were "not lowered
/// for this target yet", so QuickrBasic's `pmou.c` (`__atomic_add_fetch`,
/// `__atomic_compare_exchange_n`) did not compile on m32 or m64. At a 486 and
/// later they stay for the target to select as `lock xadd`, `lock add` and
/// `lock cmpxchg` (`X86TargetLowering::shouldExpandAtomicRMWInIR`: no
/// expansion).
#[test]
fn an_x86_add_sub_unread_or_and_cmpxchg_stay_for_the_target_to_select() {
    let mut module = parse::module(LOCKED).unwrap();
    hidden(&mut module).unwrap();
    lowered(&mut module, 32, true, true, true).unwrap();
    let kept = kept_atomics(&module);
    for want in ["rmwadd", "rmwsub", "rmwor", "cmpxchg"] {
        assert!(kept.iter().any(|name| name.starts_with(&format!("llrm.atomic.{want}."))), "{want} in {kept:?}");
    }
    // without the lock forms (16-bit code) the same is refused, as before
    let mut old = parse::module(LOCKED).unwrap();
    hidden(&mut old).unwrap();
    assert!(lowered(&mut old, 32, true, true, false).unwrap_err().contains("not lowered"));
}

/// An `and`, `or` or `xor` whose result is read, and a `nand`, `max` or `min`,
/// are `AtomicExpandPass`'s compare-exchange loop: refused by name, never a
/// plain access.
#[test]
fn an_x86_read_modify_write_that_needs_a_cmpxchg_loop_is_refused_by_name() {
    for op in ["or", "and", "xor"] {
        let text = format!(
            "target datalayout = \"e-p:32:32-i32:32\"\n@w = global i32 0\ndefine i32 @f(i32 %v) {{\nentry:\n  %a = atomicrmw {op} ptr @w, i32 %v monotonic\n  ret i32 %a\n}}\n"
        );
        let mut module = parse::module(&text).unwrap();
        hidden(&mut module).unwrap();
        let why = lowered(&mut module, 32, true, true, true).unwrap_err();
        assert!(why.contains("compare-exchange loop"), "{op}: {why}");
    }
    let text = "target datalayout = \"e-p:32:32-i32:32\"\n@w = global i32 0\ndefine void @f(i32 %v) {\nentry:\n  atomicrmw nand ptr @w, i32 %v monotonic\n  ret void\n}\n";
    let mut module = parse::module(text).unwrap();
    hidden(&mut module).unwrap();
    assert!(lowered(&mut module, 32, true, true, true).unwrap_err().contains("compare-exchange loop"));
}

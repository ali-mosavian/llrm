use super::{decode, hidden, lowered, restored};
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
    lowered(&mut module, 32, true, false).unwrap();
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
    let refused = lowered(&mut module, 32, false, false).unwrap_err();
    assert!(refused.contains("load acquire"), "{refused}");
}

/// A 64-bit atomic at a 32-bit target needs `cmpxchg8b`: refused, never two
/// plain loads.
#[test]
fn a_wide_atomic_is_refused_not_split() {
    let text = "target datalayout = \"e-p:16:16-i32:16-i64:16\"
@w = global i64 0
define i64 @f() {
entry:
  %a = load atomic i64, ptr @w monotonic, align 8
  ret i64 %a
}
";
    let mut module = parse::module(text).unwrap();
    hidden(&mut module).unwrap();
    let refused = lowered(&mut module, 32, true, false).unwrap_err();
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
    lowered(&mut module, 32, true, true).unwrap();
    let calls = opcodes(&module).iter().filter(|opcode| matches!(opcode, Opcode::Call(_))).count();
    assert_eq!(calls, 2);
    let mut refused = parse::module(SWAP).unwrap();
    hidden(&mut refused).unwrap();
    assert!(lowered(&mut refused, 32, true, false).unwrap_err().contains("not lowered"));
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
    assert!(after.contains("load atomic i32, ptr @flag acquire") && after.contains("store atomic i32 %v, ptr @flag release"), "{after}");
    assert!(!after.contains("call ") && before.contains("load atomic"), "{after}");
}

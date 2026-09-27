use llrm_mir::Module;

use super::prepared;

const DECLARED: &str = r#"
declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)
declare cc1000 void @llrm.qb.B$CEND() addrspace(1) noreturn nounwind
declare cc1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind
declare i32 @llrm.qb.personality(...) addrspace(1)
"#;

/// Prints A&+1 after registering the handler; the handler keeps ERR and
/// what `%x` was, then resumes after the faulting statement.
const HANDLED: &str = r#"
@"A&" = internal global [4 x i8] zeroinitializer
@"CAUGHT%" = internal global [2 x i8] zeroinitializer

define void @main() addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  %site = alloca i16
  call cc1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  %x = load i32, ptr @"A&"
  %y = add i32 %x, 1
  store i16 1, ptr %site
  invoke cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %y) to label %next unwind label %landing
next:
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
landing:
  %e = landingpad { ptr, i32 } catch ptr null
  %err = extractvalue { ptr, i32 } %e, 1
  %code = trunc i32 %err to i16
  store i16 %code, ptr @"CAUGHT%"
  store i32 %x, ptr @"A&"
  %k = load i16, ptr %site
  switch i16 %k, label %next [ i16 1, label %next ]
}
"#;

fn parsed(text: &str) -> Module {
    let layout = llrm_hir::mir::DATALAYOUT;
    llrm_mir::parse::module(&format!("target datalayout = \"{layout}\"\n{text}{DECLARED}")).expect("parses")
}

fn printed(module: &Module) -> String {
    llrm_mir::print::module(module)
}

#[test]
fn the_handler_registered_is_the_landing_and_err_is_what_it_kept() {
    let mut module = parsed(HANDLED);
    prepared(&mut module).expect("prepared");
    let errors = llrm_mir::verify::verify(&module);
    assert!(errors.is_empty(), "{errors:?}");
    let text = printed(&module);
    assert!(text.contains("@llrm.qb.B$OEGA(ptr addrspace(1) @$QB$LANDING)"), "{text}");
    assert!(!text.contains("void @llrm.qb.onerror(i1 true)"), "{text}");
    assert!(!text.contains("extractvalue"), "{text}");
    assert!(text.contains("load i16, ptr @$QB$LANDED"), "{text}");
}

/// The pad is entered with every register the runtime's: `%x`, made before
/// the call and read in the handler, must come from memory there.
#[test]
fn a_value_live_into_the_pad_is_read_back_from_its_slot() {
    let mut module = parsed(HANDLED);
    prepared(&mut module).expect("prepared");
    let text = printed(&module);
    let landing = &text[text.find("landing:").expect("the pad")..];
    let landing = &landing[..landing.find("\n\n").unwrap_or(landing.len())];
    assert!(landing.contains("%reloaded"), "{landing}");
    assert!(!landing.contains("store i32 %x"), "{landing}");
}

#[test]
fn an_error_a_sub_raises_is_refused() {
    let sub = r#"
define void @SUB() addrspace(1) {
entry:
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 1)
  ret void
}
"#;
    let mut module = parsed(&format!("{HANDLED}{sub}"));
    let refused = prepared(&mut module).expect_err("refused");
    assert!(refused.contains("errors raised inside SUBs"), "{refused}");
}

#[test]
fn an_error_the_handler_raises_is_refused() {
    let nested = HANDLED.replace("  %k = load i16, ptr %site", "  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 2)\n  %k = load i16, ptr %site");
    let mut module = parsed(&nested);
    let refused = prepared(&mut module).expect_err("refused");
    assert!(refused.contains("errors raised in a handler"), "{refused}");
}

/// Code only a RESUME reaches is the body's, not the handler's, though the
/// pad dominates it.
#[test]
fn a_statement_only_resume_reaches_may_raise() {
    let resumed = HANDLED.replace("  switch i16 %k, label %next [ i16 1, label %next ]", "  br label %after\nafter:\n  invoke cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 3) to label %next unwind label %landing");
    let mut module = parsed(&resumed);
    prepared(&mut module).expect("prepared");
}

/// The pad selects as a second root of its function, laid out last so it
/// is past every call the runtime resumes from, and starts by clearing DF.
#[test]
fn the_pad_is_selected_last_and_the_landing_is_its_own_procedure() {
    let next = "next:\n  call cc1000 addrspace(1) void @llrm.qb.B$CEND()\n  unreachable\n";
    let early = HANDLED.replace(next, "").replace("\n}\n", &format!("\n{next}}}\n"));
    assert!(early.find("landing:") < early.find("next:"));
    let mut module = parsed(&early);
    prepared(&mut module).expect("prepared");
    let abi = crate::abi::qb::HirAbi { runtime: crate::hir::model::RuntimeProfile::Qb45, objects: Default::default() };
    let assembled = super::super::assemble::assembled(&module, &abi, "CODE", super::super::cpu::ProfileOrName::Name("486"), &super::super::target::BUILT_IN).expect("assembled");
    let main = assembled.procedures.iter().find(|one| one.name.contains("main")).expect("main");
    let last = main.body.blocks.last().expect("blocks");
    let first = last.insns.iter().find_map(|insn| main.callees.get(&insn.at)).expect("a call");
    assert_eq!(first.name, "__LANDING");
    assert!(assembled.procedures.iter().any(|one| one.name == "$QB$LANDING"));
}

/// ON ERROR where no call may raise, as GOSUBERR's: the optimizer drops
/// the unreachable pad, and nothing can land. Was refused as "a
/// personality but no landing pad".
#[test]
fn a_handler_nothing_raises_into_registers_nothing() {
    let quiet = r#"
define void @main() addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  call cc1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}
"#;
    let mut module = parsed(quiet);
    prepared(&mut module).expect("prepared");
    let text = printed(&module);
    assert!(!text.contains("call cc1000 addrspace(1) void @llrm.qb.onerror") && !text.contains("B$OEGA"), "{text}");
}

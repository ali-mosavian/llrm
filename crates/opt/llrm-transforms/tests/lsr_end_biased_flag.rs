//! The driver builds a function both ways where LSR's end-biased formulae
//! (DEVIATE LG3) differ from the pin's: LSR marks such a function
//! `llrm-end-biased`, and makes none for one marked `llrm-no-end-biased`.

use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use llrm_transforms::lsr_pass::LoopStrengthReduce;

/// A flat x86 target: the neutral machine with the CPU's LSR model.
struct Flat(&'static str);

impl llrm_mir::target::Machine for Flat {
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

    fn tti(&self) -> Option<Rc<dyn llrm_mir::tti::Tti>> {
        Some(Rc::new(llrm_x86::tti::X86::of_cpu(self.0).unwrap()))
    }
}

fn reduced(text: &str) -> String {
    reduced_with(text, None)
}

fn reduced_with(text: &str, flag: Option<&str>) -> String {
    let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{why:?}"));
    if let Some(flag) = flag {
        let (_, function) = module.function_mut("f").expect("f");
        function.attrs.push(llrm_mir::opcode::Attribute::Flag(flag.into()));
    }
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopStrengthReduce { size: false, twin: llrm_transforms::lsr_pass::Twin::Off });
    manager.run_module(&mut module, Rc::new(Flat("i486"))).expect("runs");
    llrm_mir::print::module(&module)
}

fn shape() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ivshare/shapes/B09.p32.ll");
    fs::read_to_string(path).unwrap()
}

/// A counted loop of one array at i486 takes the end form (the exit counter
/// shared with the address); nothing said so, and a driver could not tell
/// whether to build the function the pin's way too.
#[test]
fn a_function_with_end_biased_formulae_is_marked() {
    let out = reduced(&shape());
    assert!(out.contains("llrm-end-biased"), "{out}");
    assert!(out.contains("ptr %p0, i32 400"), "{out}");
}

/// A function marked `llrm-no-end-biased` gets the pin's formulae only.
#[test]
fn a_function_marked_no_end_biased_gets_the_pins_solution() {
    let out = reduced_with(&shape(), Some(llrm_transforms::lsr_pass::NO_END_BIASED));
    assert!(!out.contains("llrm-end-biased"), "{out}");
    assert!(!out.contains("ptr %p0, i32 400"), "{out}");
}

fn reduced_twice(twin: &[llrm_transforms::lsr_pass::Twin]) -> String {
    let mut module = llrm_mir::parse::module(&shape()).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    for &twin in twin {
        manager.add(LoopStrengthReduce { size: false, twin });
    }
    manager.run_module(&mut module, Rc::new(Flat("i486"))).expect("runs");
    llrm_mir::print::module(&module)
}

/// The driver had to run the whole mid-end twice (1.94x Ir at -O2) to get the
/// pin's version of a marked function. Now the function leaves a twin at LSR:
/// `@f` end-biased, `@f.llrm-twin` the pin's, each reduced once.
#[test]
fn a_marked_function_leaves_a_twin_with_the_pins_formulae() {
    use llrm_transforms::lsr_pass::Twin;
    let out = reduced_twice(&[Twin::Make, Twin::Only]);
    let (main, twin) = out.split_once("@f.llrm-twin").unwrap_or_else(|| panic!("no twin:\n{out}"));
    assert!(main.contains("ptr %p0, i32 400"), "{out}");
    assert!(!twin.contains("ptr %p0, i32 400"), "{out}");
    assert!(!twin.contains("llrm-end-biased") && !out.contains("llrm-twin-pending"), "{out}");
}

/// No twin unless asked: a target whose driver does not pick would carry a
/// function nothing calls.
#[test]
fn no_twin_unless_asked() {
    use llrm_transforms::lsr_pass::Twin;
    assert!(!reduced_twice(&[Twin::Off]).contains("llrm-twin"));
}

/// Two loops: a counting loop LSR leaves alone (reduced first), then the
/// array loop whose result differs.
fn two_loops() -> String {
    let one = shape();
    let (head, rest) = one.split_once("loop:").unwrap();
    let _ = head;
    let body = rest.replace("label %exit", "label %mid").replace("exit:\n  ret void", "");
    let body = body.trim_end().trim_end_matches('}');
    format!(
        "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n\
define void @f(ptr %p0, ptr %p1, ptr %p2) {{\nentry:\n  br label %loop\nloop:{body}mid:\n  br label %count\n\
count:\n  %j = phi i32 [ 0, %mid ], [ %jn, %count ]\n  %jn = add nuw i32 %j, 1\n  %d = icmp eq i32 %jn, 50\n\
  br i1 %d, label %exit, label %count\nexit:\n  ret void\n}}\n"
    )
}

fn function_text<'a>(out: &'a str, name: &str) -> &'a str {
    let start = out.find(&format!("@{name}(")).unwrap_or_else(|| panic!("no @{name}:\n{out}"));
    let rest = &out[start..];
    &rest[..rest.find("\n}").unwrap_or(rest.len())]
}

/// The twin takes up at the loop that differs; the loops before it are the
/// function's. It must be what reducing the function from the start without the
/// formulae makes, or the driver compares a function with another.
#[test]
fn a_twin_taking_up_at_the_loop_is_the_function_reduced_without_the_formulae() {
    use llrm_transforms::lsr_pass::{NO_END_BIASED, Twin};
    let text = two_loops();
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    for twin in [Twin::Make, Twin::Only] {
        manager.add(LoopStrengthReduce { size: false, twin });
    }
    manager.run_module(&mut module, Rc::new(Flat("i486"))).expect("runs");
    let out = llrm_mir::print::module(&module);
    let from_start = reduced_with(&text, Some(NO_END_BIASED));
    let strip = |text: &str| text.lines().skip(1).collect::<Vec<_>>().join("\n");
    assert_eq!(
        strip(function_text(&out, "f.llrm-twin")),
        strip(function_text(&from_start, "f")),
        "{out}\n{from_start}"
    );
    assert!(function_text(&out, "f").contains("i32 400"), "{out}");
}

/// Two stack arrays xored into a global, two bytes a trip. The end-biased space
/// picks the pin's formulae here, so the function was marked and twinned for
/// nothing: memory_expansion made 65 twins at -O2, 63 of them this (+44% Ir).
const SAME_AS_THE_PINS: &str = r#"target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

@x = internal global [700 x i8] zeroinitializer, align 16

define void @f() {
entry:
  %a = alloca [700 x i8], align 16
  %b = alloca [700 x i8], align 16
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %n, %loop ]
  %pa = getelementptr inbounds nuw i8, ptr %a, i64 %i
  %va = load i8, ptr %pa
  %pb = getelementptr inbounds nuw i8, ptr %b, i64 %i
  %vb = load i8, ptr %pb
  %v = xor i8 %vb, %va
  %px = getelementptr inbounds nuw i8, ptr @x, i64 %i
  store i8 %v, ptr %px
  %j = or disjoint i64 %i, 1
  %qa = getelementptr inbounds nuw i8, ptr %a, i64 %j
  %wa = load i8, ptr %qa
  %qb = getelementptr inbounds nuw i8, ptr %b, i64 %j
  %wb = load i8, ptr %qb
  %w = xor i8 %wb, %wa
  %qx = getelementptr inbounds nuw i8, ptr @x, i64 %j
  store i8 %w, ptr %qx
  %n = add nuw nsw i64 %i, 2
  %c = icmp eq i64 %n, 700
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
"#;

#[test]
fn a_function_whose_end_biased_solution_is_the_pins_is_not_marked() {
    use llrm_transforms::lsr_pass::Twin;
    let mut module = llrm_mir::parse::module(SAME_AS_THE_PINS).unwrap_or_else(|why| panic!("{why:?}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    for twin in [Twin::Make, Twin::Only] {
        manager.add(LoopStrengthReduce { size: false, twin });
    }
    manager.run_module(&mut module, Rc::new(Flat("x86-64"))).expect("runs");
    let out = llrm_mir::print::module(&module);
    assert!(!out.contains("llrm-end-biased") && !out.contains("llrm-twin"), "{out}");
}

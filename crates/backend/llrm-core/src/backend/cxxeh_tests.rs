use llrm_mir::Module;

use super::prepared;

/// Two pads: the first catches an `int` or passes on, the second cleans up and passes on.
const TWO_PADS: &str = r#"
@_ZTIi = external constant ptr

declare i32 @__gxx_personality_v0(...)
declare i16 @risky(i16)
declare void @cleanup()
declare i32 @llvm.eh.typeid.for.p0(ptr)

define i16 @f(i16 %x) personality ptr @__gxx_personality_v0 {
entry:
  %a = invoke i16 @risky(i16 %x) to label %ok unwind label %pad1
ok:
  %b = invoke i16 @risky(i16 %a) to label %done unwind label %pad2
done:
  ret i16 %b
pad1:
  %e1 = landingpad { ptr, i32 } catch ptr @_ZTIi
  %s1 = extractvalue { ptr, i32 } %e1, 1
  %id = call i32 @llvm.eh.typeid.for.p0(ptr @_ZTIi)
  %m = icmp eq i32 %s1, %id
  br i1 %m, label %caught, label %pass
caught:
  ret i16 %x
pass:
  resume { ptr, i32 } %e1
pad2:
  %e2 = landingpad { ptr, i32 } cleanup
  call void @cleanup()
  resume { ptr, i32 } %e2
}
"#;

fn parsed(text: &str) -> Module {
    let layout = llrm_x86_m16::layout().datalayout;
    llrm_mir::parse::module(&format!("target datalayout = \"{layout}\"\n{text}")).expect("parses")
}

fn prepared_text(text: &str) -> String {
    let mut module = parsed(text);
    prepared(&mut module).expect("prepared");
    let errors = llrm_mir::verify::verify(&module);
    assert!(errors.is_empty(), "{errors:?}");
    llrm_mir::print::module(&module)
}

/// The backend refused clang's C++ output at the first `resume` and at every pad value it read: "a landing pad's value
/// read after the backend prepared it". A pad's exception pointer and selector come from what the unwinder kept.
#[test]
fn a_pad_reads_what_the_unwinder_kept_and_resume_calls_the_runtime() {
    let text = prepared_text(TWO_PADS);
    assert!(text.contains("load i16, ptr @__eh_selector"), "{text}");
    assert!(text.contains("load ptr, ptr @__eh_exception"), "{text}");
    assert!(!text.contains("resume {"), "{text}");
    assert_eq!(text.matches("call void @__unwind_resume(").count(), 2, "{text}");
}

/// `llvm.eh.typeid.for` names the selector the function's own table gives a type: its place among the function's catch
/// clauses, counted from 1.
#[test]
fn a_types_id_is_its_place_in_the_functions_catch_clauses() {
    let text = prepared_text(TWO_PADS);
    assert!(!text.contains("@llvm.eh.typeid.for.p0(ptr @_ZTIi)\n"), "{text}");
    assert!(text.contains("icmp eq i32 %") && text.contains(", 1\n"), "{text}");
}

#[test]
fn a_module_without_the_personality_is_left_alone() {
    let plain = "define i16 @g(i16 %x) {\n  ret i16 %x\n}\n";
    let mut module = parsed(plain);
    let before = llrm_mir::print::module(&module);
    prepared(&mut module).expect("prepared");
    assert_eq!(llrm_mir::print::module(&module), before);
}

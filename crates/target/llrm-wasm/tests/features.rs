//! A function's `"target-cpu"` and `"target-features"` decide what it selects.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::assembly;

fn text(attributes: &str) -> String {
    let ir = format!(
        "define i32 @f(ptr %a, ptr %b, i32 %n, i32 %x) #0 {{
  call void @llvm.memcpy.p0.p0.i32(ptr %a, ptr %b, i32 %n, i1 false)
  %low = trunc i32 %x to i8
  %wide = sext i8 %low to i32
  ret i32 %wide
}}
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
attributes #0 = {{ {attributes} }}
"
    );
    assembly(&ir, Width::W32, Options::default()).unwrap()
}

/// Every function got the generic CPU's bulk memory and sign extension, so
/// `-mcpu=mvp` output called `memory.copy` and `i32.extend8_s`, which an MVP
/// engine rejects: the module did not validate.
#[test]
fn mvp_has_neither_bulk_memory_nor_sign_extension() {
    let generic = text("\"target-cpu\"=\"generic\"");
    assert!(generic.contains("memory.copy"), "{generic}");
    assert!(generic.contains("i32.extend8_s"), "{generic}");
    let mvp = text("\"target-cpu\"=\"mvp\"");
    assert!(mvp.contains("call\tmemcpy"), "{mvp}");
    assert!(!mvp.contains("memory.copy") && !mvp.contains("extend8_s"), "{mvp}");
}

/// `+sign-ext` on top of `mvp` is the generic output for it, and
/// `-bulk-memory-opt` takes `bulk-memory` with it.
#[test]
fn features_apply_in_order_after_the_cpu() {
    let some = text("\"target-cpu\"=\"mvp\" \"target-features\"=\"+sign-ext\"");
    assert!(some.contains("i32.extend8_s") && some.contains("call\tmemcpy"), "{some}");
    let less = text("\"target-cpu\"=\"generic\" \"target-features\"=\"-bulk-memory-opt\"");
    assert!(less.contains("call\tmemcpy") && less.contains("i32.extend8_s"), "{less}");
}

/// A parameter first used after a variable-length `memcpy` was an `ARGUMENT`
/// in the block that follows the length test, not in the entry block: the
/// printer stopped with "opcode 427 has no assembly string".
#[test]
fn a_parameter_used_after_a_guarded_memcpy_is_read_in_the_entry_block() {
    let generic = text("\"target-cpu\"=\"generic\"");
    assert!(generic.contains("memory.copy") && generic.contains("local.get\t3"), "{generic}");
}

fn has_section(
    attributes: &str,
    section: &str,
) -> bool {
    let ir = format!("define i32 @f() #0 {{\n  ret i32 0\n}}\nattributes #0 = {{ {attributes} }}\n");
    let object = llrm_wasm::driver::object(&ir, Width::W32, Options::default()).unwrap();
    object.windows(section.len()).any(|window| window == section.as_bytes())
}

/// `-mcpu=mvp` objects carried the generic CPU's `target_features` section,
/// so a linker took the module for one that needs bulk memory and sign
/// extension.
#[test]
fn an_mvp_object_names_no_target_features() {
    assert!(has_section("\"target-cpu\"=\"generic\"", "target_features"));
    assert!(!has_section("\"target-cpu\"=\"mvp\"", "target_features"));
}

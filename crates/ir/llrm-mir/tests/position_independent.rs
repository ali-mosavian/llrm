//! `Module::position_independent` reads clang's "PIC Level" module flag.
//! A `-fno-pic` module has none, and llrm-cc ignored `-fno-pic` at m64: a
//! global array indexed in a loop was an `lea sym(%rip)` and a register, where
//! the pin's static code is `sym(,%idx,8)`.

use llrm_mir::parse::module;

const PIC: &str = r#"
@a = global i32 0
!llvm.module.flags = !{!0, !1}
!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"uwtable", i32 2}
"#;

const STATIC: &str = r#"
@a = global i32 0
!llvm.module.flags = !{!0}
!0 = !{i32 7, !"uwtable", i32 2}
"#;

#[test]
fn test_a_module_with_the_pic_level_flag_is_position_independent() {
    assert!(module(PIC).unwrap().position_independent());
}

#[test]
fn test_a_clang_module_without_the_pic_level_flag_is_static() {
    assert!(!module(STATIC).unwrap().position_independent());
}

#[test]
fn test_a_module_with_no_flags_stays_position_independent() {
    assert!(module("@a = global i32 0\n").unwrap().position_independent());
}

//! A runtime routine called once in a module is expanded at its call site. Left as `jp __mulqi3` it cost the
//! `mul8` row 17 B against SDCC's 15; expanded it is 14.

use llrm_z80::RUNTIME;
use llrm_z80::inline_rt::inline_sole_uses;

#[test]
fn a_sole_tail_jump_to_a_leaf_routine_becomes_its_body() {
    let out = inline_sole_uses("_f::\n\tjp __mulqi3\n", RUNTIME);
    assert!(!out.contains("__mulqi3\n") && !out.contains("jp\t__mulqi3"), "{out}");
    assert!(out.contains("\tsrl\tl\n") && out.trim_end().ends_with("ret"), "{out}");
}

#[test]
fn a_sole_call_loses_the_routines_return() {
    let out = inline_sole_uses("_f::\n\tcall __mulqi3\n\tinc a\n\tret\n", RUNTIME);
    assert!(!out.contains("call"), "{out}");
    assert!(out.contains("\tjr\tnc") || out.contains("\tjr\tnz"), "{out}");
    assert_eq!(out.matches("\tret\n").count(), 1, "only the caller's ret is left:\n{out}");
    assert!(out.contains("\tinc a\n"), "{out}");
}

#[test]
fn a_routine_used_twice_stays_a_call() {
    let module = "_f::\n\tcall __mulqi3\n\tcall __mulqi3\n\tret\n";
    assert_eq!(inline_sole_uses(module, RUNTIME), module);
}

#[test]
fn a_routine_that_calls_another_stays_a_call() {
    let module = "_f::\n\tjp __udivqi3\n";
    assert_eq!(inline_sole_uses(module, RUNTIME), module);
}

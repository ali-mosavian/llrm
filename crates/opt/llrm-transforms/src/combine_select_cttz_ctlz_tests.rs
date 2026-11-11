//! `foldSelectCttzCtlz` against `opt`'s output on the same text.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::InstId;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;

use crate::combine::{Combiner, Visit, combine_with_declared};
use crate::testing::{parsed, printed};

/// `visit` that does only `visitSelectInst`.
fn visit(
    c: &mut Combiner,
    inst: InstId,
) -> Visit {
    if c.function.instruction(inst).opcode == Opcode::Select { c.visit_select(inst) } else { Visit::None }
}

fn combined(
    params: &str,
    ret: &str,
    body: &str,
) -> String {
    let text = format!(
        "declare i32 @llvm.cttz.i32(i32, i1 immarg)\ndeclare i32 @llvm.ctlz.i32(i32, i1 immarg)\n\
         define {ret} @f({params}) {{\nb0:\n{body}}}\n"
    );
    let mut module = parsed(&text);
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).expect("placed");
    printed(&module)
}

/// m68k `lead0`/`trail0` (`x ? clz(x) : 32`) kept a compare and a branch on
/// every target: the pin has the bare intrinsic with its zero case defined.
#[test]
fn a_zero_guarded_count_is_the_intrinsic_with_zero_defined() {
    for (name, cmp, arms) in [
        ("ctlz", "ne", "i32 %z, i32 32"),
        ("ctlz", "eq", "i32 32, i32 %z"),
        ("cttz", "ne", "i32 %z, i32 32"),
        ("cttz", "eq", "i32 32, i32 %z"),
    ] {
        let out = combined(
            "i32 %x",
            "i32",
            &format!(
                "  %c = icmp {cmp} i32 %x, 0\n  %z = call i32 @llvm.{name}.i32(i32 %x, i1 true)\n  \
                 %r = select i1 %c, {arms}\n  ret i32 %r\n"
            ),
        );
        assert!(!out.contains("select"), "{name} {cmp}: {out}");
        // The count is unguarded now, so its zero case must be defined: with
        // `i1 true` it returned garbage for 0 (x86-64 `lead0(0)` was 29, not 32).
        assert!(out.contains("i1 false") && !out.contains("i1 true"), "{name} {cmp}: {out}");
    }
}

/// The count under a `zext` or a `trunc` is still the select's arm.
#[test]
fn the_count_may_be_extended_or_truncated() {
    let zext = combined(
        "i32 %x",
        "i64",
        "  %c = icmp eq i32 %x, 0\n  %z = call i32 @llvm.ctlz.i32(i32 %x, i1 true)\n  %e = zext i32 %z to i64\n  \
         %r = select i1 %c, i64 32, i64 %e\n  ret i64 %r\n",
    );
    assert!(!zext.contains("select"), "{zext}");
    let trunc = combined(
        "i32 %x",
        "i16",
        "  %c = icmp eq i32 %x, 0\n  %z = call i32 @llvm.cttz.i32(i32 %x, i1 true)\n  %e = trunc i32 %z to i16\n  \
         %r = select i1 %c, i16 32, i16 %e\n  ret i16 %r\n",
    );
    assert!(!trunc.contains("select"), "{trunc}");
}

/// `x == -1 ? 32 : cttz(~x)` and `x == y ? 32 : cttz(x ^ y)`.
#[test]
fn the_count_may_be_of_the_not_or_the_xor_of_the_compare() {
    let not = combined(
        "i32 %x",
        "i32",
        "  %c = icmp eq i32 %x, -1\n  %n = xor i32 %x, -1\n  %z = call i32 @llvm.cttz.i32(i32 %n, i1 true)\n  \
         %r = select i1 %c, i32 32, i32 %z\n  ret i32 %r\n",
    );
    assert!(!not.contains("select"), "{not}");
    for xor in ["xor i32 %x, %y", "xor i32 %y, %x"] {
        let out = combined(
            "i32 %x, i32 %y",
            "i32",
            &format!(
                "  %c = icmp eq i32 %x, %y\n  %n = {xor}\n  %z = call i32 @llvm.cttz.i32(i32 %n, i1 true)\n  \
                 %r = select i1 %c, i32 32, i32 %z\n  ret i32 %r\n"
            ),
        );
        assert!(!out.contains("select"), "{xor}: {out}");
    }
}

/// The zero case is defined by the intrinsic: its flag is false after the
/// fold, as `ctlz(x, false)` in the pin's output.
#[test]
fn the_intrinsic_keeps_its_flag_and_the_select_goes() {
    let out = combined(
        "i32 %x",
        "i32",
        "  %c = icmp ne i32 %x, 0\n  %z = call i32 @llvm.ctlz.i32(i32 %x, i1 false)\n  \
         %r = select i1 %c, i32 %z, i32 32\n  ret i32 %r\n",
    );
    assert!(out.contains("call i32 @llvm.ctlz.i32(i32 %x, i1 false)"), "{out}");
    assert!(!out.contains("select"), "{out}");
}

/// Another value at zero stays a select, and the one-use count relaxes to
/// "zero is poison" (the pin's `ctlz(x, true)`).
#[test]
fn another_zero_value_relaxes_the_count_to_zero_is_poison() {
    let out = combined(
        "i32 %x",
        "i32",
        "  %c = icmp eq i32 %x, 0\n  %z = call i32 @llvm.ctlz.i32(i32 %x, i1 false)\n  \
         %r = select i1 %c, i32 7, i32 %z\n  ret i32 %r\n",
    );
    assert!(out.contains("@llvm.ctlz.i32(i32 %x, i1 true)"), "{out}");
    assert!(out.contains("select"), "{out}");
}

/// A count used elsewhere is not relaxed; a compare of another value and a
/// wrong zero value are not this fold.
#[test]
fn the_fold_does_not_apply_to_other_shapes() {
    let shared = combined(
        "i32 %x",
        "i32",
        "  %c = icmp eq i32 %x, 0\n  %z = call i32 @llvm.ctlz.i32(i32 %x, i1 false)\n  \
         %r = select i1 %c, i32 7, i32 %z\n  %s = add i32 %r, %z\n  ret i32 %s\n",
    );
    assert!(shared.contains("@llvm.ctlz.i32(i32 %x, i1 false)"), "{shared}");
    let other = combined(
        "i32 %x, i32 %y",
        "i32",
        "  %c = icmp eq i32 %y, 0\n  %z = call i32 @llvm.ctlz.i32(i32 %x, i1 true)\n  \
         %r = select i1 %c, i32 32, i32 %z\n  ret i32 %r\n",
    );
    assert!(other.contains("select"), "{other}");
    let wrong = combined(
        "i32 %x",
        "i32",
        "  %c = icmp eq i32 %x, 0\n  %z = call i32 @llvm.ctlz.i32(i32 %x, i1 true)\n  \
         %r = select i1 %c, i32 31, i32 %z\n  ret i32 %r\n",
    );
    assert!(wrong.contains("select"), "{wrong}");
}

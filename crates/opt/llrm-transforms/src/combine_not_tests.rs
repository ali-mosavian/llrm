use llrm_mir::datalayout::DataLayout;

use crate::combine::{combine_with_declared, visit};
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\ndeclare void @use(i1)\n";

/// `@f(params)` returning `ret` after the whole InstCombine visit.
fn run(
    ret: &str,
    params: &str,
    body: &str,
) -> String {
    let mut module = parsed(&format!("{HEAD}define {ret} @f({params}) {{\nb0:\n{body}}}\n"));
    let globals = module.globals.clone();
    let metadata = module.metadata.clone();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    combine_with_declared(context, &globals, &metadata, &DataLayout::default(), function, visit, &mut declared);
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// ispunct on the 6502 grew 111 -> 137 bytes: `~(a | b)` of two compares
/// stayed an `or` and a `not`, where the pin inverts both compares and makes an
/// `and` (`sinkNotIntoLogicalOp`).
#[test]
fn a_not_of_an_or_of_two_compares_is_an_and_of_the_inverted_compares() {
    let out = run(
        "i1",
        "i16 %x",
        "  %a = add i16 %x, -48\n  %b = icmp ult i16 %a, 10\n  %c = add i16 %x, -97\n  %d = icmp ult i16 %c, 26\n  %o = or i1 %b, %d\n  %n = xor i1 %o, true\n  ret i1 %n\n",
    );
    assert!(out.contains("and i1 %") && !out.contains("or i1") && !out.contains("xor i1"), "{out}");
    assert_eq!(out.matches("icmp").count(), 2, "{out}");
}

/// The `and` form takes the same fold, to an `or`.
#[test]
fn a_not_of_an_and_of_two_compares_is_an_or_of_the_inverted_compares() {
    let out = run(
        "i1",
        "i16 %x, i16 %y",
        "  %b = icmp ult i16 %x, 10\n  %d = icmp ult i16 %y, 26\n  %o = and i1 %b, %d\n  %n = xor i1 %o, true\n  ret i1 %n\n",
    );
    assert!(out.contains("icmp ugt i16 %x, 9") && out.contains("icmp ugt i16 %y, 25"), "{out}");
    assert!(out.contains("or i1 %") && !out.contains("and i1") && !out.contains("xor i1"), "{out}");
}

/// A logical or (`select a, true, b`) becomes a logical and.
#[test]
fn a_not_of_a_logical_or_is_a_logical_and_of_the_inverted_compares() {
    let out = run(
        "i1",
        "i16 %x, i16 %y",
        "  %b = icmp ult i16 %x, 10\n  %d = icmp ult i16 %y, 26\n  %o = select i1 %b, i1 true, i1 %d\n  %n = xor i1 %o, true\n  ret i1 %n\n",
    );
    assert!(out.contains("icmp ugt i16 %x, 9") && out.contains("icmp ugt i16 %y, 25"), "{out}");
    assert!(out.contains("select i1 %b, i1 %d, i1 false") || out.contains("i1 false"), "{out}");
    assert!(!out.contains("xor i1"), "{out}");
}

/// Control: plain `i1` arguments are not freely invertible, the `not` stays.
#[test]
fn a_not_of_an_or_of_plain_arguments_stays() {
    let out = run("i1", "i1 %p, i1 %q", "  %o = or i1 %p, %q\n  %n = xor i1 %o, true\n  ret i1 %n\n");
    assert!(out.contains("or i1 %p, %q") && out.contains("xor i1 %o, true"), "{out}");
}

/// Control: a compare with another user cannot be inverted for all its users.
#[test]
fn a_compare_with_another_user_keeps_the_not() {
    let out = run(
        "i1",
        "i16 %x, i16 %y",
        "  %b = icmp ult i16 %x, 10\n  %d = icmp ult i16 %y, 26\n  call void @use(i1 %b)\n  %o = or i1 %b, %d\n  %n = xor i1 %o, true\n  ret i1 %n\n",
    );
    assert!(out.contains("or i1 %b, %d") && out.contains("xor i1 %o, true"), "{out}");
}

/// A select that reads the `not` as its condition swaps its arms instead.
#[test]
fn a_select_on_the_not_swaps_its_arms() {
    let out = run(
        "i16",
        "i16 %x, i16 %y",
        "  %b = icmp ult i16 %x, 10\n  %d = icmp ult i16 %y, 26\n  %o = or i1 %b, %d\n  %n = xor i1 %o, true\n  %s = select i1 %n, i16 1, i16 2\n  ret i16 %s\n",
    );
    assert!(out.contains("and i1 %") && !out.contains("xor i1") && !out.contains("or i1"), "{out}");
}

//! A constant under a scale, through an extension, or in an add another
//! address reads, becomes a displacement: each case's fixture is the
//! pass's input, its shape asserted, and `ptrtoint` of the address run
//! before and after.

use llrm_mir::module::Module;

use super::GepOffset;
use crate::testing::{managed, parsed, results};

const HEAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32\"\n\n@g = global [64 x i16] zeroinitializer\n@h = global [64 x i16] zeroinitializer\n\n";

/// `text` through the pass: the module, its text, and the text before.
fn separated(text: &str) -> (Module, String, String) {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let before = llrm_mir::print::module(&module);
    let after = managed(&mut module, GepOffset);
    (module, before, after)
}

/// `text` split, answering as before on `inputs`; its text before and after.
fn checked(text: &str, inputs: &[&[i128]]) -> (String, String) {
    let original = parsed(&format!("{HEAD}{text}"));
    let (module, before, after) = separated(text);
    assert_eq!(results(&module, inputs), results(&original, inputs), "{before}\n{after}");
    (before, after)
}

const INPUTS: &[&[i128]] = &[&[0], &[1], &[-1], &[127], &[-128], &[300], &[32767], &[-32768]];

/// `a[(i + 8) * 2]` kept its add, its multiply and a register for the sum:
/// the 32 bytes were never a displacement.
#[test]
fn test_a_constant_added_under_a_scale_is_a_displacement() {
    let (before, after) = checked(
        "define i16 @f(i16 %i) {
b0:
  %s = add i16 %i, 8
  %m = mul i16 %s, 2
  %p = getelementptr i16, ptr @g, i16 %m
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}
",
        INPUTS,
    );
    assert!(before.contains("add i16 %i, 8"), "{before}");
    assert!(after.contains("getelementptr i8, ptr @g, i16 32"), "{after}");
    assert!(!after.contains("add i16 %i, 8"), "{after}");
}

/// `i + 8` read by two addresses: each takes the 16 bytes, and the sum goes.
#[test]
fn test_an_add_with_a_second_reader_is_a_displacement_in_each_address() {
    let (_, after) = checked(
        "define i16 @f(i16 %i) {
b0:
  %t = add i16 %i, 8
  %p = getelementptr i16, ptr @g, i16 %t
  %q = getelementptr i16, ptr @h, i16 %t
  %a = ptrtoint ptr %p to i16
  %b = ptrtoint ptr %q to i16
  %c = add i16 %a, %b
  ret i16 %c
}
",
        INPUTS,
    );
    assert!(after.contains("getelementptr i8, ptr @g, i16 16") && after.contains("getelementptr i8, ptr @h, i16 16"), "{after}");
    assert!(!after.contains("add i16 %i, 8"), "{after}");
}

/// The `i8` inputs on which `x + 3` does not wrap under `flags`: a wrap
/// there is poison, which nothing is held to.
fn defined(flags: &str) -> Vec<Vec<i128>> {
    (0..256).filter(|&x: &i128| match flags {
        "nsw" => !(125..128).contains(&x),
        "nuw" => x <= 252,
        _ => true,
    }).map(|x| vec![x]).collect()
}

/// A byte index the `gep` sign-extends, summed without wrapping, and the same
/// through an explicit `sext`: the constant moves out. Without `nsw` it
/// does not, where 127 + 3 wraps.
#[test]
fn test_a_narrow_index_gives_up_its_constant_only_where_it_cannot_wrap() {
    for (index, flags, split) in [
        ("%s = add {flags} i8 %x, 3\n  %e = sext i8 %s to i16", "nsw", true),
        ("%s = add {flags} i8 %x, 3\n  %e = sext i8 %s to i16", "", false),
        ("%s = add {flags} i8 %x, 3\n  %e = zext i8 %s to i16", "nuw", true),
        ("%s = add {flags} i8 %x, 3\n  %e = zext i8 %s to i16", "nsw", false),
    ] {
        let body = index.replace("{flags}", flags);
        let (_, after) = checked(
            &format!(
                "define i16 @f(i8 %x) {{
b0:
  {body}
  %p = getelementptr i16, ptr @g, i16 %e
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}}
"
            ),
            &defined(flags).iter().map(Vec::as_slice).collect::<Vec<_>>(),
        );
        assert_eq!(after.contains("getelementptr i8, ptr @g, i16 6"), split, "{index} {flags}\n{after}");
    }
}

/// A `gep` indexed by an `i8` the pointer's width sign-extends itself.
#[test]
fn test_an_index_the_gep_extends_is_split_under_nsw() {
    for (flags, split) in [("nsw", true), ("", false)] {
        let (_, after) = checked(
            &format!(
                "define i16 @f(i8 %x) {{
b0:
  %s = add {flags} i8 %x, 3
  %p = getelementptr i16, ptr @g, i8 %s
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}}
"
            ),
            &defined(flags).iter().map(Vec::as_slice).collect::<Vec<_>>(),
        );
        assert_eq!(after.contains("getelementptr i8, ptr @g, i16 6"), split, "{flags}\n{after}");
    }
}

/// `a[i - 1]` points outside the object: the split addresses are not
/// `inbounds`, whatever the original was.
#[test]
fn test_a_negative_constant_drops_inbounds() {
    let (_, after) = checked(
        "define i16 @f(i16 %i) {
b0:
  %s = add i16 %i, -1
  %p = getelementptr inbounds i16, ptr @g, i16 %s
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}
",
        INPUTS,
    );
    assert!(after.contains("getelementptr i8, ptr @g, i16 -2"), "{after}");
    assert!(!after.contains("inbounds"), "{after}");
}

/// A wide index is truncated to the pointer's width: the constant is taken
/// modulo it, 70000 as 4464.
#[test]
fn test_a_wide_index_constant_is_taken_modulo_the_pointer_width() {
    let (_, after) = checked(
        "define i16 @f(i32 %i) {
b0:
  %s = add i32 %i, 70000
  %p = getelementptr i16, ptr @g, i32 %s
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}
",
        &[&[0], &[1], &[-1], &[65535], &[65536], &[1 << 30]],
    );
    assert!(after.contains("getelementptr i8, ptr @g, i16 8928"), "{after}");
}

/// A split that would make an instruction for none it frees stays whole:
/// `(i + 8) * 2` with the sum and the product both read elsewhere.
#[test]
fn test_a_split_that_costs_an_instruction_is_left_alone() {
    let (before, after) = checked(
        "define i16 @f(i16 %i) {
b0:
  %s = add i16 %i, 8
  %m = mul i16 %s, 2
  %p = getelementptr i16, ptr @g, i16 %m
  %a = ptrtoint ptr %p to i16
  %b = add i16 %a, %s
  %c = add i16 %b, %m
  ret i16 %c
}
",
        INPUTS,
    );
    assert_eq!(before, after);
}

/// A product by a value is no affine index: nothing is taken out.
#[test]
fn test_a_product_by_a_value_is_left_alone() {
    let (before, after) = checked(
        "define i16 @f(i16 %i, i16 %k) {
b0:
  %s = add i16 %i, 8
  %m = mul i16 %s, %k
  %p = getelementptr i16, ptr @g, i16 %m
  %a = ptrtoint ptr %p to i16
  ret i16 %a
}
",
        &[&[0, 3], &[5, -7], &[300, 9]],
    );
    assert_eq!(before, after);
}

/// `(x + 3) + y` and `(x + 3) - y` in bytes, kept from wrapping by the flags
/// their extension needs: the inputs where neither add wraps, with the
/// ones that made a narrow rest wrap (`x + y` is -129, 127 as a byte).
fn two_leaves(flag: &str, kind: &str, body: &str) -> (String, String) {
    let signed = flag == "nsw";
    let values: Vec<i128> = if signed { vec![-128, -127, -126, -4, -3, -1, 0, 1, 5, 100, 124, 127] } else { vec![0, 1, 2, 3, 5, 100, 128, 200, 250, 252, 255] };
    let (low, high) = if signed { (-128, 127) } else { (0, 255) };
    let mut inputs = Vec::new();
    for &x in &values {
        for &y in &values {
            let inner = x + 3;
            let whole = if kind == "add" { inner + y } else { inner - y };
            if (low..=high).contains(&inner) && (low..=high).contains(&whole) {
                inputs.push(vec![x, y]);
            }
        }
    }
    assert!(inputs.len() > 20);
    let text = format!(
        "define i16 @f(i8 %x, i8 %y) {{
b0:
  %a = add {flag} i8 %x, 3
  %s = {kind} {flag} i8 %a, %y
  {body}
  %a2 = ptrtoint ptr %p to i16
  ret i16 %a2
}}
"
    );
    let (_, after) = checked(&text, &inputs.iter().map(Vec::as_slice).collect::<Vec<_>>());
    (text, after)
}

/// A rest summed in the narrow width wraps where the extended whole does
/// not: `sext(x + 3 + y)` with x = -1, y = -128 is -126, and `sext(x + y) + 3`
/// was 130. The extension goes to the leaves. (The same sum under the
/// `gep`'s own extension costs a cast a leaf and is left alone.)
#[test]
fn test_a_rest_of_two_leaves_is_summed_after_the_extension() {
    let (_, kept) = two_leaves("nsw", "add", "%p = getelementptr i16, ptr @g, i8 %s");
    assert!(!kept.contains("getelementptr i8, ptr @g"), "{kept}");
    for (flag, kind, body) in [
        ("nsw", "add", "%e = sext i8 %s to i16\n  %p = getelementptr i16, ptr @g, i16 %e"),
        ("nsw", "sub", "%e = sext i8 %s to i16\n  %p = getelementptr i16, ptr @g, i16 %e"),
        ("nuw", "sub", "%e = zext i8 %s to i16\n  %p = getelementptr i16, ptr @g, i16 %e"),
        ("nuw", "add", "%e = zext i8 %s to i16\n  %p = getelementptr i16, ptr @g, i16 %e"),
    ] {
        let (_, after) = two_leaves(flag, kind, body);
        assert!(after.contains("getelementptr i8, ptr @g, i16 6"), "{flag} {kind} {body}\n{after}");
    }
}

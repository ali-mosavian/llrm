use crate::valuetracking::{alignment, sign_bits};
use crate::{GlobalKind, parse};

/// The sign bits of what `@f` returns.
fn returned(body: &str) -> u32 {
    let module = parse::module(&format!("define i64 @f(i32 %a, i32 %b, i64 %w) {{\n{body}\n}}\n")).expect("parses");
    let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else { unreachable!() };
    let ret = function.terminator(function.entry().expect("an entry")).expect("a return");
    sign_bits(&module.context, function, function.instruction(ret).operands[0])
}

/// A 32x32 product in 64 bits, then its fixed-point forms: what isel asks
/// before it multiplies or divides in one 32-bit instruction.
#[test]
fn test_sign_bits_follow_extension_shifts_and_products() {
    assert_eq!(returned("  %x = sext i32 %a to i64\n  ret i64 %x"), 33);
    assert_eq!(returned("  %x = zext i32 %a to i64\n  ret i64 %x"), 32);
    assert_eq!(returned("  %x = sext i32 %a to i64\n  %y = shl i64 %x, 16\n  ret i64 %y"), 17);
    assert_eq!(returned("  %x = sext i32 %a to i64\n  %y = ashr i64 %x, 4\n  ret i64 %y"), 37);
    assert_eq!(
        returned("  %x = sext i32 %a to i64\n  %y = sext i32 %b to i64\n  %z = mul i64 %x, %y\n  ret i64 %z"),
        1
    );
    assert_eq!(returned("  %x = ashr i64 %w, 48\n  %y = ashr i64 %w, 40\n  %z = mul i64 %x, %y\n  ret i64 %z"), 25);
    assert_eq!(returned("  ret i64 -1"), 64);
    assert_eq!(returned("  ret i64 65535"), 48);
    assert_eq!(returned("  ret i64 %w"), 1);
}

/// The alignment of what `@f` returns, in a module with a word-aligned
/// `@w` and an unaligned `@b`.
fn aligned(body: &str) -> u64 {
    let module = parse::module(&format!("@w = global [100 x i8] zeroinitializer, align 2\n@b = global [100 x i8] zeroinitializer\ndefine ptr @f(i16 %i) {{\n{body}\n}}\n")).expect("parses");
    let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else { unreachable!() };
    let ret = function.terminator(function.entry().expect("an entry")).expect("a return");
    let layout = crate::datalayout::DataLayout::default();
    alignment(&module.context, &layout, &module.globals, function, function.instruction(ret).operands[0])
}

/// An element of a word-aligned word array is word aligned wherever its
/// index is: what lets hoist show a word read cannot cross offset FFFFh.
#[test]
fn test_alignment_is_the_object_s_less_what_each_index_may_add() {
    assert_eq!(aligned("  %p = getelementptr inbounds i16, ptr @w, i16 %i\n  ret ptr %p"), 2);
    assert_eq!(
        aligned(
            "  %d = mul i16 %i, 2\n  %p = getelementptr i8, ptr @w, i16 %d\n  %q = getelementptr i8, ptr %p, i16 1280\n  ret ptr %q"
        ),
        2
    );
    assert_eq!(aligned("  %p = getelementptr i8, ptr @w, i16 %i\n  ret ptr %p"), 1);
    assert_eq!(
        aligned(
            "  %p = getelementptr inbounds i16, ptr @w, i16 %i\n  %q = getelementptr i8, ptr %p, i16 1\n  ret ptr %q"
        ),
        1
    );
    assert_eq!(aligned("  %p = getelementptr inbounds i32, ptr @b, i16 %i\n  ret ptr %p"), 1);
}

/// The bits `@f` returns that are proven zero, as a 64-bit mask.
fn zeros(body: &str) -> u128 {
    let module = parse::module(&format!("define i64 @f(i32 %a, i32 %b, i64 %w) {{\n{body}\n}}\n")).expect("parses");
    let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else { unreachable!() };
    let ret = function.terminator(function.entry().expect("an entry")).expect("a return");
    crate::valuetracking::known_zero(&module.context, function, function.instruction(ret).operands[0])
}

#[test]
fn test_known_zero_bits_follow_masks_extensions_and_shifts() {
    let high = |low: u32| u128::from(u64::MAX) & !((1_u128 << low) - 1);
    assert_eq!(zeros("  %m = and i32 %a, 255\n  %x = zext i32 %m to i64\n  ret i64 %x"), high(8));
    assert_eq!(
        zeros("  %m = and i64 %w, 255\n  %t = trunc i64 %m to i8\n  %x = zext i8 %t to i64\n  ret i64 %x"),
        high(8)
    );
    assert_eq!(zeros("  %m = and i64 %w, 255\n  %x = lshr i64 %m, 4\n  ret i64 %x"), high(4));
    assert_eq!(zeros("  %x = shl i64 %w, 3\n  ret i64 %x"), 7);
    assert_eq!(zeros("  %m = and i64 %w, 15\n  %n = and i64 %w, 3\n  %x = or i64 %m, %n\n  ret i64 %x"), high(4));
    assert_eq!(zeros("  ret i64 %w"), 0);
}

/// A scale by 2 or 4 clears the low bits, and a sum of two such multiples keeps
/// the least: tile's `(x + 7) * 2 & 126` kept its mask because `mul` and `add`
/// proved no bit.
#[test]
fn test_known_zero_low_bits_follow_products_and_sums() {
    assert_eq!(zeros("  %x = mul i64 %w, 8\n  ret i64 %x"), 7);
    assert_eq!(zeros("  %m = mul i64 %w, 4\n  %n = mul i64 %w, 2\n  %x = mul i64 %m, %n\n  ret i64 %x"), 7);
    assert_eq!(zeros("  %m = mul i64 %w, 4\n  %n = mul i64 %w, 16\n  %x = add i64 %m, %n\n  ret i64 %x"), 3);
    assert_eq!(zeros("  %m = mul i64 %w, 4\n  %x = sub i64 %m, 1\n  ret i64 %x"), 0);
    assert_eq!(zeros("  %x = mul i64 %w, 3\n  ret i64 %x"), 0);
}

/// The ValueTracking API answers from what the stub proves: a constant exactly,
/// a masked value by its zero bits, a shifted one by the sign bit.
#[test]
fn the_stub_api_answers_from_the_zero_bits_it_proves() {
    use crate::valuetracking::{
        compute_constant_range, compute_known_bits, is_known_non_negative, is_known_to_be_a_power_of_two,
        masked_value_is_zero,
    };
    let module = crate::parse::module(
        "define i16 @f(i16 %x) {\nb0:\n  %m = and i16 %x, 255\n  %s = lshr i16 %x, 4\n  ret i16 %m\n}\n",
    )
    .expect("parses");
    let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else { unreachable!() };
    let named = |name: &str| {
        let result = function
            .walk()
            .map(|(_, inst)| function.instruction(inst))
            .find(|one| one.result.is_some_and(|value| function.value(value).name.as_deref() == Some(name)))
            .and_then(|one| one.result)
            .expect("a named value");
        crate::module::Operand::Value(result)
    };
    let context = &module.context;
    let (m, s) = (named("m"), named("s"));
    assert_eq!(compute_known_bits(context, function, m, 0, None).zero, 0xFF00);
    assert!(masked_value_is_zero(context, function, m, 0xFF00, 0, None));
    assert!(is_known_non_negative(context, function, m, 0, None));
    assert!(is_known_non_negative(context, function, s, 0, None));
    let range = compute_constant_range(context, function, m, false, 0, None);
    assert_eq!((range.lower, range.upper), (0, 256));
    assert!(!is_known_to_be_a_power_of_two(context, function, m, false, 0, None));
}

/// A table a pass declared (`Declared::table`) is a constant naming a global
/// that is not in the module until `place`; the alignment asked then panicked
/// indexing past `globals`. Like every other lookup it is `get`: unknown, so 1.
#[test]
fn test_alignment_of_a_global_the_running_pass_declared_is_one() {
    let module = parse::module("@w = global [100 x i8] zeroinitializer, align 2\ndefine ptr @f(i16 %i) {\n  %p = getelementptr inbounds i16, ptr @w, i16 %i\n  ret ptr %p\n}\n").expect("parses");
    let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else { unreachable!() };
    let ret = function.terminator(function.entry().expect("an entry")).expect("a return");
    let layout = crate::datalayout::DataLayout::default();
    let pointer = function.instruction(ret).operands[0];
    assert_eq!(alignment(&module.context, &layout, &module.globals, function, pointer), 2);
    // `@w` not yet among the globals.
    assert_eq!(alignment(&module.context, &layout, &[], function, pointer), 1);
}

/// A far pointer made of a global's selector is `segment:0`, a pointer into
/// the data segment: `underlying` named the global its selector came from.
#[test]
fn underlying_stops_at_a_cast_from_a_selector_alone() {
    let text = |datalayout: &str| {
        format!(
            "target datalayout = \"{datalayout}\"

@pa = global [2 x i8] zeroinitializer

define ptr addrspace(1) @f(i16 %n) {{
b0:
  %near = getelementptr i8, ptr @pa, i16 0
  %far0 = addrspacecast ptr %near to ptr addrspace(1)
  %sg = addrspacecast ptr addrspace(1) %far0 to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %sg to ptr addrspace(1)
  %p = getelementptr i8, ptr addrspace(1) %far, i16 %n
  ret ptr addrspace(1) %p
}}
"
        )
    };
    let base = "e-p:16:16-p1:32:16:16:16-p2:16:16";
    let root = |datalayout: &str| {
        let module = parse::module(&text(datalayout)).expect("parses");
        let layout = crate::datalayout::DataLayout::parse(datalayout).expect("a layout");
        let GlobalKind::Function(function) = &module.global(module.named("f").expect("@f")).kind else {
            unreachable!()
        };
        let ret = function.terminator(function.entry().expect("an entry")).expect("a return");
        let (root, _) =
            crate::valuetracking::underlying(&module.context, &layout, function, function.instruction(ret).operands[0]);
        matches!(
            root,
            crate::module::Operand::Constant(id) if matches!(module.context.get(id).kind, crate::context::ConstantKind::Global(_))
        )
    };
    assert!(root(base), "premise: through an integral selector space the global is the object");
    assert!(!root(&format!("{base}-ni:2")));
}

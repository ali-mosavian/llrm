use crate::datalayout::DataLayout;
use crate::interpret::{Trap, Val, run};
use crate::parse;
use crate::types::{FloatKind, Type, Types};

const LAYOUT: &str = "e-p:16:16-p1:32:16:16:16-p3:32:16:16:32-i32:16-i64:16";

fn result(text: &str) -> Result<Val, Trap> {
    let module = parse::module(&format!("target datalayout = \"{LAYOUT}\"\n{text}")).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    run(&module, "f", Vec::new(), 10_000)
}

fn int(bits: u128, width: u32) -> Result<Val, Trap> {
    Ok(Val::Int { bits, width })
}

#[test]
fn the_datalayout_places_fields_and_far_pointers_as_stated() {
    let layout = DataLayout::parse(LAYOUT).expect("parses");
    let mut types = Types::default();
    let (i16, i32) = (types.int(16), types.int(32));
    let pair = types.intern(Type::Struct { fields: vec![i16, i32], packed: false });
    assert_eq!(layout.struct_layout(&types, pair), (6, vec![0, 2]), "i32 aligned to 2 bytes");
    let far = layout.pointer(1);
    assert_eq!((far.bits, far.align, far.index_bits), (32, 2, 16));
    let i64 = types.int(64);
    assert_eq!(DataLayout::default().align(&types, i64), 4, "LLVM's default i64:32");
}

#[test]
fn flags_make_poison_where_the_value_does_not_fit() {
    assert_eq!(result("define i16 @f() {\n  %x = add nsw i16 32767, 1\n  ret i16 %x\n}\n"), Ok(Val::Poison));
    assert_eq!(result("define i16 @f() {\n  %x = add i16 32767, 1\n  ret i16 %x\n}\n"), int(0x8000, 16));
    assert_eq!(result("define i16 @f() {\n  %x = shl i16 1, 16\n  ret i16 %x\n}\n"), Ok(Val::Poison));
}

#[test]
fn undefined_behaviour_is_reported_not_run() {
    assert_eq!(result("define i16 @f() {\n  %x = sdiv i16 1, 0\n  ret i16 %x\n}\n"), Err(Trap::Undefined("a division by zero".to_owned())));
    let text = "define i16 @f() {\nentry:\n  %c = icmp eq i16 poison, 0\n  br i1 %c, label %a, label %b\na:\n  ret i16 1\nb:\n  ret i16 2\n}\n";
    assert_eq!(result(text), Err(Trap::Undefined("a branch on poison".to_owned())));
}

#[test]
fn a_far_offset_wraps_at_its_index_width() {
    let text = "@a = global [2 x i16] [i16 1, i16 2]\ndefine i32 @f() {\n  %far = addrspacecast ptr @a to ptr addrspace(1)\n  %p = getelementptr i16, ptr addrspace(1) %far, i16 32768\n  %base = ptrtoint ptr addrspace(1) %far to i32\n  %moved = ptrtoint ptr addrspace(1) %p to i32\n  %d = sub i32 %moved, %base\n  ret i32 %d\n}\n";
    // 32768 * 2 bytes is 0x10000: the 16-bit offset wraps to where it began.
    assert_eq!(result(text), int(0, 32));
}

/// A huge pointer indexes by 32 bits, so a displacement past 64K carries into
/// its selector where a far one's wraps.
#[test]
fn a_huge_offset_carries_where_a_far_one_wraps() {
    let moved = |space: u32| {
        let cast = if space == 1 { "getelementptr i8, ptr addrspace(1) @a, i32 0".to_owned() } else { format!("addrspacecast ptr addrspace(1) @a to ptr addrspace({space})") };
        format!(
            "@a = addrspace(1) global [2 x i16] [i16 1, i16 2]\ndefine i32 @f() {{\n  %p = {cast}\n  %q = getelementptr i8, ptr addrspace({space}) %p, i32 65536\n  %base = ptrtoint ptr addrspace({space}) %p to i32\n  %moved = ptrtoint ptr addrspace({space}) %q to i32\n  %d = sub i32 %moved, %base\n  ret i32 %d\n}}\n"
        )
    };
    assert_eq!(result(&moved(1)), int(0, 32), "far: the offset wraps to where it began");
    assert_eq!(result(&moved(3)), int(65536, 32), "huge: the selector carries");
}

/// Two huge pointers' distance in bytes, across 64K segments.
#[test]
fn a_huge_pointer_difference_is_the_bytes_between() {
    let text = "@a = addrspace(1) global [40000 x i32] zeroinitializer\ndeclare i32 @llrm.ia16.ptrdiff.i32.p3(ptr addrspace(3), ptr addrspace(3))\ndefine i32 @f() {\n  %p = addrspacecast ptr addrspace(1) @a to ptr addrspace(3)\n  %q = getelementptr i32, ptr addrspace(3) %p, i32 39999\n  %d = call i32 @llrm.ia16.ptrdiff.i32.p3(ptr addrspace(3) %q, ptr addrspace(3) %p)\n  ret i32 %d\n}\n";
    assert_eq!(result(text), int(159_996, 32));
}

#[test]
fn every_fixture_that_states_its_answer_gives_it() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut ran = 0;
    for dir in std::fs::read_dir(&root).expect("tests/").flatten() {
        for file in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            let text = std::fs::read_to_string(file.path()).unwrap_or_default();
            let Some(want) = text.lines().find_map(|line| line.strip_prefix("; expect: ")).and_then(|one| one.parse::<u128>().ok()) else { continue };
            let module = parse::module(&text).expect("parses");
            let got = run(&module, "main", Vec::new(), 1_000_000);
            assert!(matches!(got, Ok(Val::Int { bits, .. }) if bits & 0xff == want), "{}: {got:?}", file.path().display());
            ran += 1;
        }
    }
    assert!(ran >= 6, "{ran} fixtures ran");
}

/// lli runs no overflow, min/max or fmuladd intrinsic to check these against;
/// each answer is derived by hand.
#[test]
fn overflow_intrinsics_wrap_and_say_so() {
    let pair = |intrinsic: &str, a: i16, b: i16| {
        result(&format!(
            "declare {{ i16, i1 }} @llvm.{intrinsic}.with.overflow.i16(i16, i16)\n\
             define {{ i16, i1 }} @f() {{\n  %r = call {{ i16, i1 }} @llvm.{intrinsic}.with.overflow.i16(i16 {a}, i16 {b})\n  ret {{ i16, i1 }} %r\n}}\n"
        ))
    };
    let wrapped = |bits: u128, overflowed: u128| Ok(Val::Aggregate(vec![Val::Int { bits, width: 16 }, Val::Int { bits: overflowed, width: 1 }]));
    assert_eq!(pair("sadd", 32767, 1), wrapped(0x8000, 1));
    assert_eq!(pair("uadd", 32767, 1), wrapped(0x8000, 0));
    assert_eq!(pair("uadd", -1, 1), wrapped(0, 1));
    assert_eq!(pair("ssub", -32768, 1), wrapped(0x7fff, 1));
    assert_eq!(pair("usub", 0, 1), wrapped(0xffff, 1));
    assert_eq!(pair("smul", -2, 3), wrapped(0xfffa, 0));
    assert_eq!(pair("smul", 256, 128), wrapped(0x8000, 1));
    assert_eq!(pair("umul", 256, 256), wrapped(0, 1));
}

#[test]
fn min_max_intrinsics_compare_as_their_names_say() {
    let chosen = |intrinsic: &str, a: i16, b: i16| {
        result(&format!("declare i16 @llvm.{intrinsic}.i16(i16, i16)
define i16 @f() {{
  %r = call i16 @llvm.{intrinsic}.i16(i16 {a}, i16 {b})
  ret i16 %r
}}
"))
    };
    assert_eq!(chosen("smax", -5, 3), int(3, 16));
    assert_eq!(chosen("smin", -5, 3), int(0xfffb, 16));
    assert_eq!(chosen("umax", -5, 3), int(0xfffb, 16));
    assert_eq!(chosen("umin", -5, 3), int(3, 16));
}

#[test]
fn fmuladd_multiplies_then_adds() {
    let text = "declare float @llvm.fmuladd.f32(float, float, float)
define float @f() {
  %r = call float @llvm.fmuladd.f32(float 2.000000e+00, float 3.000000e+00, float 4.000000e+00)
  ret float %r
}
";
    assert_eq!(result(text), Ok(Val::Float(FloatKind::Float, u64::from(10.0f32.to_bits()))));
}

/// BASIC's float to integer conversion, the machine's round to nearest,
/// ties to even; derived by hand, since lli cannot run it.
#[test]
fn lrint_rounds_ties_to_even() {
    let rounded = |x: &str| {
        result(&format!("declare i16 @llvm.lrint.i16.f64(double)\ndefine i16 @f() {{\n  %r = call i16 @llvm.lrint.i16.f64(double {x})\n  ret i16 %r\n}}\n"))
    };
    assert_eq!(rounded("2.500000e+00"), int(2, 16));
    assert_eq!(rounded("3.500000e+00"), int(4, 16));
    assert_eq!(rounded("-2.600000e+00"), int(0xfffd, 16));
}

/// The unary float intrinsics, each answer exact and derived by hand.
#[test]
fn unary_float_intrinsics_compute_their_functions() {
    let applied = |name: &str, x: &str| {
        result(&format!("declare double @llvm.{name}.f64(double)\ndefine double @f() {{\n  %r = call double @llvm.{name}.f64(double {x})\n  ret double %r\n}}\n"))
    };
    let double = |x: f64| Ok(Val::Float(FloatKind::Double, x.to_bits()));
    assert_eq!(applied("fabs", "-3.000000e+00"), double(3.0));
    assert_eq!(applied("sqrt", "2.250000e+00"), double(1.5));
    assert_eq!(applied("rint", "2.500000e+00"), double(2.0));
    assert_eq!(applied("exp2", "3.000000e+00"), double(8.0));
    assert_eq!(applied("log2", "8.000000e+00"), double(3.0));
    assert_eq!(applied("sin", "0.000000e+00"), double(0.0));
    assert_eq!(applied("cos", "0.000000e+00"), double(1.0));
    assert_eq!(applied("atan", "0.000000e+00"), double(0.0));
}

/// `llvm.smul.fix` and `llvm.sdiv.fix` compute what Nib's expansion did:
/// at twice the width, shifted, divided toward zero, and wrapped.
#[test]
fn fixed_point_intrinsics_are_their_expansions() {
    for width in [16_u32, 32] {
        let top = 1_i128 << (width - 1);
        let values = [0, 1, -1, 7, -7, 300, -300, top - 1, -top];
        for (a, b) in values.iter().flat_map(|&a| values.iter().map(move |&b| (a, b))) {
            for (name, expansion) in [("smul", "%p = mul i64 %a, %b\n  %s = ashr i64 %p, 8"), ("sdiv", "%u = shl i64 %a, 8\n  %s = sdiv i64 %u, %b")] {
                let wide = |body: &str| body.replace("i64", &format!("i{}", 2 * width));
                let expanded = result(&format!(
                    "define i{width} @f() {{\n  %a = sext i{width} {a} to i{w2}\n  %b = sext i{width} {b} to i{w2}\n  {}\n  %r = trunc i{w2} %s to i{width}\n  ret i{width} %r\n}}\n",
                    wide(expansion),
                    w2 = 2 * width
                ));
                let called = result(&format!(
                    "declare i{width} @llvm.{name}.fix.i{width}(i{width}, i{width}, i32)\n\
                     define i{width} @f() {{\n  %r = call i{width} @llvm.{name}.fix.i{width}(i{width} {a}, i{width} {b}, i32 8)\n  ret i{width} %r\n}}\n"
                ));
                match expanded {
                    Err(Trap::Undefined(_)) => assert!(matches!(called, Err(Trap::Undefined(_))), "{name} {a} {b}: {called:?}"),
                    expanded => assert_eq!(called, expanded, "{name}.i{width} {a} {b}"),
                }
            }
        }
    }
}

#[test]
fn near_memory_stays_below_64k_whatever_is_far() {
    // Globals were laid out in order, allocas after them, so a far array
    // pushed a near global and every frame past 0xFFFF, and a near pointer
    // stored in its 16 bits came back pointing at 0 (tools/loops batches).
    let text = "@big = addrspace(1) global [70000 x i8] zeroinitializer\n@n = global i16 0\n\
        define i16 @f() {\n  %slot = alloca ptr\n  store ptr @n, ptr %slot\n  %p = load ptr, ptr %slot\n  \
        store i16 7, ptr %p\n  %v = load i16, ptr @n\n  ret i16 %v\n}\n";
    assert_eq!(result(text), int(7, 16));
}

/// Runs `@f` on two pointers to one 4-byte cell, each as given, checked.
fn aliased(attrs: &str, body: &str) -> Result<Val, Trap> {
    let text = format!(
        "target datalayout = \"{LAYOUT}\"\n@cell = global [2 x i16] zeroinitializer\n\
         define i16 @g(ptr {attrs} %p, ptr {attrs} %q) {{\n{body}}}\n\
         define i16 @f() {{\n  %r = call i16 @g(ptr @cell, ptr @cell)\n  ret i16 %r\n}}\n"
    );
    let module = parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    crate::interpret::run_checked(&module, "g", vec![Val::Ptr(16), Val::Ptr(16)], 10_000)
}

const WRITES_THEN_READS: &str = "  store i16 1, ptr %p\n  %x = load i16, ptr %q\n  ret i16 %x\n";

/// A `noalias` parameter reached through another pointer that writes is a
/// broken promise; unchecked it ran, and whatever a pass did with the fact
/// was wrong without a trace.
#[test]
fn a_noalias_parameter_reached_another_way_is_reported() {
    let trapped = aliased("noalias", WRITES_THEN_READS).unwrap_err();
    assert!(matches!(&trapped, Trap::Undefined(why) if why.contains("noalias parameter")), "{trapped:?}");
}

/// Without the fact there is no promise to break, and two readers do not
/// conflict.
#[test]
fn aliased_pointers_without_noalias_or_with_only_reads_are_fine() {
    assert_eq!(aliased("", WRITES_THEN_READS), int(1, 16));
    assert_eq!(aliased("noalias", "  %x = load i16, ptr %p\n  %y = load i16, ptr %q\n  %s = add i16 %x, %y\n  ret i16 %s\n"), int(0, 16));
}

/// Runs `@g(ptr %p)` checked, with `attrs` on the parameter, on `argument`.
fn one_pointer(attrs: &str, body: &str, argument: u64) -> Result<Val, Trap> {
    let text = format!("target datalayout = \"{LAYOUT}\"\n@cell = global [2 x i16] zeroinitializer\ndefine i16 @g(ptr {attrs} %p) {{\n{body}}}\n");
    let module = parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    crate::interpret::run_checked(&module, "g", vec![Val::Ptr(argument)], 10_000)
}

/// A parameter stated `readonly` and written through is a broken promise.
#[test]
fn a_readonly_parameter_written_through_is_reported() {
    let writes = "  store i16 1, ptr %p\n  ret i16 0\n";
    let trapped = one_pointer("readonly", writes, 16).unwrap_err();
    assert!(matches!(&trapped, Trap::Undefined(why) if why.contains("readonly parameter")), "{trapped:?}");
    assert_eq!(one_pointer("", writes, 16), int(0, 16));
    assert_eq!(one_pointer("readonly", "  %x = load i16, ptr %p\n  ret i16 %x\n", 16), int(0, 16));
}

/// A parameter stated `nonnull` and passed null is a broken promise.
#[test]
fn a_nonnull_parameter_passed_null_is_reported() {
    let reads = "  %x = load i16, ptr @cell\n  ret i16 %x\n";
    let trapped = one_pointer("nonnull", reads, 0).unwrap_err();
    assert!(matches!(&trapped, Trap::Undefined(why) if why.contains("nonnull parameter 0")), "{trapped:?}");
    assert_eq!(one_pointer("nonnull", reads, 16), int(0, 16));
    assert_eq!(one_pointer("", reads, 0), int(0, 16));
}

/// Runs `@g` of `text` checked on `arguments`.
fn range_run(text: &str, arguments: Vec<Val>) -> Result<Val, Trap> {
    let module = parse::module(&format!("target datalayout = \"{LAYOUT}\"\n@cell = global i16 5\n{text}")).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    crate::interpret::run_checked(&module, "g", arguments, 10_000)
}

fn small(bits: u128) -> Val {
    Val::Int { bits, width: 16 }
}

/// A value outside the `range` stated of a parameter, of the result, or of a
/// load (`!range`) is a broken promise; a value inside, or an unchecked run, is not.
#[test]
fn a_value_outside_its_stated_range_is_reported() {
    let parameter = "define i16 @g(i16 range(i16 0, 2) %p) {\nentry:\n  ret i16 %p\n}\n";
    assert_eq!(range_run(parameter, vec![small(1)]), Ok(small(1)));
    let trapped = range_run(parameter, vec![small(5)]).unwrap_err();
    assert!(matches!(&trapped, Trap::Undefined(why) if why.contains("parameter 0") && why.contains("[0, 2)")), "{trapped:?}");
    let result = "define range(i16 0, 2) i16 @g(i16 %p) {\nentry:\n  ret i16 %p\n}\n";
    assert_eq!(range_run(result, vec![small(0)]), Ok(small(0)));
    assert!(matches!(range_run(result, vec![small(7)]).unwrap_err(), Trap::Undefined(why) if why.contains("the result")));
    let load = "define i16 @g() {\nentry:\n  %v = load i16, ptr @cell, !range !0\n  ret i16 %v\n}\n\n!0 = !{i16 0, i16 2}\n";
    assert!(matches!(range_run(load, vec![]).unwrap_err(), Trap::Undefined(why) if why.contains("!range")));
    let signed = "define i16 @g(i16 range(i16 -1, 2) %p) {\nentry:\n  ret i16 %p\n}\n";
    assert_eq!(range_run(signed, vec![small(0xffff)]), Ok(small(0xffff)), "-1 is inside -1..=1");
    assert!(range_run(signed, vec![small(2)]).is_err());
    // Unchecked, the promise is not looked at.
    let module = parse::module(&format!("target datalayout = \"{LAYOUT}\"\n{parameter}")).unwrap();
    assert_eq!(crate::interpret::run(&module, "g", vec![small(5)], 100), Ok(small(5)));
}

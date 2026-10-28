use crate::datalayout::DataLayout;
use crate::interpret::{Trap, Val, run};
use crate::parse;
use crate::types::{FloatKind, Type, Types};

const LAYOUT: &str = "e-p:16:16-p1:32:16:16:16-p3:32:16:16:32-i32:16-i64:16";

fn result(text: &str) -> Result<Val, Trap> {
    let module =
        parse::module(&format!("target datalayout = \"{LAYOUT}\"\n{text}")).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    run(&module, "f", Vec::new(), 10_000)
}

fn int(
    bits: u128,
    width: u32,
) -> Result<Val, Trap> {
    Ok(Val::Int { bits, width })
}

#[test]
fn the_datalayout_places_fields_and_far_pointers_as_stated() {
    let layout = DataLayout::parse(LAYOUT).expect("parses");
    let types = Types::default();
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
    assert_eq!(
        result("define i16 @f() {\n  %x = sdiv i16 1, 0\n  ret i16 %x\n}\n"),
        Err(Trap::Undefined("a division by zero".to_owned()))
    );
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
        let cast = if space == 1 {
            "getelementptr i8, ptr addrspace(1) @a, i32 0".to_owned()
        } else {
            format!("addrspacecast ptr addrspace(1) @a to ptr addrspace({space})")
        };
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
            let Some(want) =
                text.lines().find_map(|line| line.strip_prefix("; expect: ")).and_then(|one| one.parse::<u128>().ok())
            else {
                continue;
            };
            let module = parse::module(&text).expect("parses");
            let got = run(&module, "main", Vec::new(), 1_000_000);
            assert!(
                matches!(got, Ok(Val::Int { bits, .. }) if bits & 0xff == want),
                "{}: {got:?}",
                file.path().display()
            );
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
    let wrapped = |bits: u128, overflowed: u128| {
        Ok(Val::Aggregate(vec![Val::Int { bits, width: 16 }, Val::Int { bits: overflowed, width: 1 }]))
    };
    assert_eq!(pair("sadd", 32767, 1), wrapped(0x8000, 1));
    assert_eq!(pair("uadd", 32767, 1), wrapped(0x8000, 0));
    assert_eq!(pair("uadd", -1, 1), wrapped(0, 1));
    assert_eq!(pair("ssub", -32768, 1), wrapped(0x7fff, 1));
    assert_eq!(pair("usub", 0, 1), wrapped(0xffff, 1));
    assert_eq!(pair("smul", -2, 3), wrapped(0xfffa, 0));
    assert_eq!(pair("smul", 256, 128), wrapped(0x8000, 1));
    assert_eq!(pair("umul", 256, 256), wrapped(0, 1));
}

/// `llvm.{s,u}{add,sub}.sat` stopped the parser at the unknown name; each
/// answer is derived by hand.
#[test]
fn saturating_intrinsics_clamp_to_the_range() {
    let sat = |intrinsic: &str, a: i16, b: i16| {
        result(&format!(
            "declare i16 @llvm.{intrinsic}.sat.i16(i16, i16)
define i16 @f() {{
  %r = call i16 @llvm.{intrinsic}.sat.i16(i16 {a}, i16 {b})
  ret i16 %r
}}
"
        ))
    };
    assert_eq!(sat("sadd", 32767, 1), int(0x7fff, 16));
    assert_eq!(sat("sadd", -32768, -1), int(0x8000, 16));
    assert_eq!(sat("sadd", 5, -7), int(0xfffe, 16));
    assert_eq!(sat("ssub", -32768, 1), int(0x8000, 16));
    assert_eq!(sat("ssub", 32767, -1), int(0x7fff, 16));
    assert_eq!(sat("uadd", -1, 1), int(0xffff, 16));
    assert_eq!(sat("uadd", 3, 4), int(7, 16));
    assert_eq!(sat("usub", 3, 4), int(0, 16));
    assert_eq!(sat("usub", 4, 3), int(1, 16));
}

#[test]
fn min_max_intrinsics_compare_as_their_names_say() {
    let chosen = |intrinsic: &str, a: i16, b: i16| {
        result(&format!(
            "declare i16 @llvm.{intrinsic}.i16(i16, i16)
define i16 @f() {{
  %r = call i16 @llvm.{intrinsic}.i16(i16 {a}, i16 {b})
  ret i16 %r
}}
"
        ))
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

#[test]
fn copysign_takes_the_second_arguments_sign() {
    let text = "declare double @llvm.copysign.f64(double, double)
define double @f() {
  %r = call double @llvm.copysign.f64(double 2.000000e+00, double -0.000000e+00)
  ret double %r
}
";
    assert_eq!(result(text), Ok(Val::Float(FloatKind::Double, (-2.0f64).to_bits())));
}

/// BASIC's float to integer conversion, the machine's round to nearest,
/// ties to even; derived by hand, since lli cannot run it.
#[test]
fn lrint_rounds_ties_to_even() {
    let rounded = |x: &str| {
        result(&format!(
            "declare i16 @llvm.lrint.i16.f64(double)\ndefine i16 @f() {{\n  %r = call i16 @llvm.lrint.i16.f64(double {x})\n  ret i16 %r\n}}\n"
        ))
    };
    assert_eq!(rounded("2.500000e+00"), int(2, 16));
    assert_eq!(rounded("3.500000e+00"), int(4, 16));
    assert_eq!(rounded("-2.600000e+00"), int(0xfffd, 16));
}

/// The unary float intrinsics, each answer exact and derived by hand.
#[test]
fn unary_float_intrinsics_compute_their_functions() {
    let applied = |name: &str, x: &str| {
        result(&format!(
            "declare double @llvm.{name}.f64(double)\ndefine double @f() {{\n  %r = call double @llvm.{name}.f64(double {x})\n  ret double %r\n}}\n"
        ))
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
            for (name, expansion) in [
                ("smul", "%p = mul i64 %a, %b\n  %s = ashr i64 %p, 8"),
                ("sdiv", "%u = shl i64 %a, 8\n  %s = sdiv i64 %u, %b"),
            ] {
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
                    Err(Trap::Undefined(_)) => {
                        assert!(matches!(called, Err(Trap::Undefined(_))), "{name} {a} {b}: {called:?}")
                    }
                    expanded => assert_eq!(called, expanded, "{name}.i{width} {a} {b}"),
                }
            }
        }
    }
}

/// `llvm.ctlz`, `llvm.cttz` and `llvm.ctpop` count as the machine integers
/// do: a zero has as many zeros as the type has bits, and is poison where the
/// second argument of the first two says so.
#[test]
fn bit_count_intrinsics_count_what_the_machine_integers_do() {
    let call = |name: &str, width: u32, value: u32, flag: bool| {
        let (parameters, arguments) =
            if name == "ctpop" { (String::new(), String::new()) } else { (", i1".to_owned(), format!(", i1 {flag}")) };
        result(&format!(
            "declare i{width} @llvm.{name}.i{width}(i{width}{parameters})\n\
             define i{width} @f() {{\n  %r = call i{width} @llvm.{name}.i{width}(i{width} {value}{arguments})\n  ret i{width} %r\n}}\n"
        ))
    };
    for value in [0_u32, 1, 2, 3, 0x80, 0x1234, 0x7fff, 0x8000, 0xffff] {
        let narrow = value as u16;
        assert_eq!(call("ctpop", 16, value, false), int(u128::from(narrow.count_ones()), 16));
        for wide in [value, value << 8 | 1, u32::MAX] {
            assert_eq!(call("ctpop", 32, wide, false), int(u128::from(wide.count_ones()), 32));
        }
        if value == 0 {
            assert_eq!(call("ctlz", 16, 0, false), int(16, 16));
            assert_eq!(call("cttz", 16, 0, false), int(16, 16));
            assert_eq!(call("ctlz", 16, 0, true), Ok(Val::Poison));
            assert_eq!(call("cttz", 16, 0, true), Ok(Val::Poison));
        } else {
            for flag in [false, true] {
                assert_eq!(call("ctlz", 16, value, flag), int(u128::from(narrow.leading_zeros()), 16));
                assert_eq!(call("cttz", 16, value, flag), int(u128::from(narrow.trailing_zeros()), 16));
            }
        }
    }
}

/// `llvm.bswap` reverses the bytes of an integer of any whole number of 16-bit
/// units.
#[test]
fn bswap_reverses_the_bytes() {
    let call = |bits: u32, value: u128| {
        result(&format!(
            "declare i{bits} @llvm.bswap.i{bits}(i{bits})\n\
             define i{bits} @f() {{\n  %r = call i{bits} @llvm.bswap.i{bits}(i{bits} {value})\n  ret i{bits} %r\n}}\n"
        ))
    };
    assert_eq!(call(16, 0x1234), int(0x3412, 16));
    assert_eq!(call(32, 0x1234_5678), int(0x7856_3412, 32));
    assert_eq!(call(64, 0x0123_4567_89ab_cdef), int(0xefcd_ab89_6745_2301, 64));
    assert_eq!(call(48, 0x0123_4567_89ab), int(0xab89_6745_2301, 48));
    assert_eq!(call(128, 1), int(1 << 120, 128));
}

/// `llvm.fshl` and `llvm.fshr` shift the concatenation of their first two
/// arguments by the third modulo the width (values from `opt
/// -passes=instsimplify`).
#[test]
fn funnel_shifts_take_the_amount_modulo_the_width() {
    let call = |name: &str, a: u8, b: u8, by: u8| {
        result(&format!(
            "declare i8 @llvm.{name}.i8(i8, i8, i8)\n\
             define i8 @f() {{\n  %r = call i8 @llvm.{name}.i8(i8 {a}, i8 {b}, i8 {by})\n  ret i8 %r\n}}\n"
        ))
    };
    assert_eq!(call("fshl", 18, 52, 3), int(0x91, 8));
    assert_eq!(call("fshr", 18, 52, 3), int(0x46, 8));
    // A zero amount keeps the half that is not shifted: the first for fshl, the
    // second for fshr.
    assert_eq!(call("fshl", 18, 52, 0), int(18, 8));
    assert_eq!(call("fshr", 18, 52, 8), int(52, 8));
    assert_eq!(call("fshl", 18, 52, 11), int(0x91, 8));
    // The same value twice is a rotate.
    assert_eq!(call("fshl", 129, 129, 1), int(3, 8));
    assert_eq!(call("fshr", 129, 129, 1), int(0xc0, 8));
}

/// `llvm.abs` of the minimum is the minimum, or poison where the flag says.
#[test]
fn abs_of_the_minimum_is_itself_unless_the_flag_makes_it_poison() {
    let call = |a: u8, flag: bool| {
        result(&format!(
            "declare i8 @llvm.abs.i8(i8, i1)\n\
             define i8 @f() {{\n  %r = call i8 @llvm.abs.i8(i8 {a}, i1 {flag})\n  ret i8 %r\n}}\n"
        ))
    };
    assert_eq!(call(251, false), int(5, 8));
    assert_eq!(call(7, true), int(7, 8));
    assert_eq!(call(128, false), int(128, 8));
}

/// `llvm.scmp` and `llvm.ucmp` give -1, 0 or 1 at the result's width, comparing
/// as signed or unsigned integers.
#[test]
fn compare_intrinsics_give_minus_one_zero_or_one() {
    let call = |name: &str, a: u8, b: u8| {
        result(&format!(
            "declare i8 @llvm.{name}.i8.i8(i8, i8)\n\
             define i8 @f() {{\n  %r = call i8 @llvm.{name}.i8.i8(i8 {a}, i8 {b})\n  ret i8 %r\n}}\n"
        ))
    };
    for a in [0_u8, 1, 5, 127, 128, 200, 255] {
        for b in [0_u8, 1, 5, 127, 128, 200, 255] {
            let signed = (a as i8).cmp(&(b as i8)) as i8 as u8;
            let unsigned = a.cmp(&b) as i8 as u8;
            assert_eq!(call("scmp", a, b), int(u128::from(signed), 8), "scmp {a} {b}");
            assert_eq!(call("ucmp", a, b), int(u128::from(unsigned), 8), "ucmp {a} {b}");
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
fn aliased(
    attrs: &str,
    body: &str,
) -> Result<Val, Trap> {
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
    assert_eq!(
        aliased("noalias", "  %x = load i16, ptr %p\n  %y = load i16, ptr %q\n  %s = add i16 %x, %y\n  ret i16 %s\n"),
        int(0, 16)
    );
}

/// Runs `@g(ptr %p)` checked, with `attrs` on the parameter, on `argument`.
fn one_pointer(
    attrs: &str,
    body: &str,
    argument: u64,
) -> Result<Val, Trap> {
    let text = format!(
        "target datalayout = \"{LAYOUT}\"\n@cell = global [2 x i16] zeroinitializer\ndefine i16 @g(ptr {attrs} %p) {{\n{body}}}\n"
    );
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

/// A memcpy moves bytes: one the source never wrote stays only that byte
/// undefined, so the byte written beside it reads back. Copied as an i16
/// load and store it was the whole word that went poison.
#[test]
fn a_memcpy_keeps_an_unwritten_byte_apart_from_the_written_one() {
    let text = |read: &str| {
        format!(
            "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
define i8 @f() {{
  %from = alloca [4 x i8]
  %to = alloca [4 x i8]
  store i8 7, ptr %from
  call void @llvm.memcpy.p0.p0.i16(ptr %to, ptr %from, i16 4, i1 false)
  {read}
  ret i8 %v
}}
"
        )
    };
    assert_eq!(result(&text("%v = load i8, ptr %to")), int(7, 8));
    assert_eq!(result(&text("%at = getelementptr i8, ptr %to, i16 1\n  %v = load i8, ptr %at")), Ok(Val::Poison));
}

/// A pattern fill stores its cell little-endian, once per count: a dword
/// read back as two words is its halves.
#[test]
fn a_pattern_fill_stores_its_cell_little_endian() {
    let text = |read: &str| {
        format!(
            "declare void @llvm.experimental.memset.pattern.p0.i32.i16(ptr, i32, i16, i1)
define i16 @f() {{
  %to = alloca [3 x i32]
  call void @llvm.experimental.memset.pattern.p0.i32.i16(ptr %to, i32 305419896, i16 3, i1 false)
  {read}
  ret i16 %v
}}
"
        )
    };
    assert_eq!(result(&text("%v = load i16, ptr %to")), int(0x5678, 16));
    assert_eq!(result(&text("%at = getelementptr i16, ptr %to, i16 5\n  %v = load i16, ptr %at")), int(0x1234, 16));
}

/// Runs `@g` of `text` checked on `arguments`.
fn range_run(
    text: &str,
    arguments: Vec<Val>,
) -> Result<Val, Trap> {
    let module = parse::module(&format!("target datalayout = \"{LAYOUT}\"\n@cell = global i16 5\n{text}"))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    crate::interpret::run_checked(&module, "g", arguments, 10_000)
}

fn small(bits: u128) -> Val {
    Val::Int { bits, width: 16 }
}

/// A value outside the `range` stated of a parameter, of the result, or of a
/// load (`!range`) is a broken promise; a value inside, or an unchecked run, is
/// not.
#[test]
fn a_value_outside_its_stated_range_is_reported() {
    let parameter = "define i16 @g(i16 range(i16 0, 2) %p) {\nentry:\n  ret i16 %p\n}\n";
    assert_eq!(range_run(parameter, vec![small(1)]), Ok(small(1)));
    let trapped = range_run(parameter, vec![small(5)]).unwrap_err();
    assert!(
        matches!(&trapped, Trap::Undefined(why) if why.contains("parameter 0") && why.contains("[0, 2)")),
        "{trapped:?}"
    );
    let result = "define range(i16 0, 2) i16 @g(i16 %p) {\nentry:\n  ret i16 %p\n}\n";
    assert_eq!(range_run(result, vec![small(0)]), Ok(small(0)));
    assert!(
        matches!(range_run(result, vec![small(7)]).unwrap_err(), Trap::Undefined(why) if why.contains("the result"))
    );
    let load =
        "define i16 @g() {\nentry:\n  %v = load i16, ptr @cell, !range !0\n  ret i16 %v\n}\n\n!0 = !{i16 0, i16 2}\n";
    assert!(matches!(range_run(load, vec![]).unwrap_err(), Trap::Undefined(why) if why.contains("!range")));
    // Two pairs, 0..2 and 10..12: inside the second is inside.
    let by_pair = |value: u128| {
        let text = format!(
            "define i16 @g() {{\nentry:\n  %v = load i16, ptr @cell3, !range !0\n  ret i16 %v\n}}\n\n@cell3 = global i16 {value}\n!0 = !{{i16 0, i16 2, i16 10, i16 12}}\n"
        );
        range_run(&text, vec![])
    };
    assert_eq!(by_pair(11), Ok(small(11)), "inside the second pair");
    assert_eq!(by_pair(1), Ok(small(1)), "inside the first");
    assert!(matches!(by_pair(5).unwrap_err(), Trap::Undefined(why) if why.contains("!range")), "between the pairs");
    let signed = "define i16 @g(i16 range(i16 -1, 2) %p) {\nentry:\n  ret i16 %p\n}\n";
    assert_eq!(range_run(signed, vec![small(0xffff)]), Ok(small(0xffff)), "-1 is inside -1..=1");
    assert!(range_run(signed, vec![small(2)]).is_err());
    // Unchecked, the promise is not looked at.
    let module = parse::module(&format!("target datalayout = \"{LAYOUT}\"\n{parameter}")).unwrap();
    assert_eq!(crate::interpret::run(&module, "g", vec![small(5)], 100), Ok(small(5)));
}

/// An `undef` operand reads as some value: the interpreter picks zero, so
/// `undef + 5` is 5 and not poison.
#[test]
fn undef_reads_as_a_value_not_poison() {
    assert_eq!(result("define i16 @f() {\nentry:\n  %a = add i16 undef, 5\n  ret i16 %a\n}\n"), int(5, 16));
}

/// `atomicrmw`, `cmpxchg` and the atomic loads and stores had no parser and no
/// interpreter: one thread runs, so each is the plain access with LLVM's result
/// (the old value; `{ old, hit }`).
#[test]
fn an_atomic_access_is_the_plain_one_on_one_thread() {
    let text = "define i32 @f() {
entry:
  %p = alloca i32, align 4
  store atomic i32 7, ptr %p seq_cst, align 4
  %old = atomicrmw add ptr %p, i32 5 acq_rel
  %miss = cmpxchg ptr %p, i32 1, i32 100 seq_cst seq_cst
  %hit = cmpxchg ptr %p, i32 12, i32 40 seq_cst seq_cst
  %flag = extractvalue { i32, i1 } %hit, 1
  %f = zext i1 %flag to i32
  %now = load atomic i32, ptr %p acquire, align 4
  fence seq_cst
  %a = mul i32 %old, 1000
  %b = mul i32 %now, 10
  %c = add i32 %a, %b
  %d = add i32 %c, %f
  ret i32 %d
}
";
    // old 7, then 12 -> 40 on the hit: 7000 + 400 + 1.
    assert_eq!(result(text), int(7401, 32));
}

/// `llvm.umul.fix`, `llvm.{s,u}mul.fix.sat` and `llvm.{s,u}shl.sat` were
/// unknown names to the parser; each answer is derived by hand at 16 bits.
#[test]
fn fixed_multiplies_and_saturating_shifts_clamp_to_the_range() {
    let fixed = |name: &str, a: i16, b: i16, scale: u32| {
        result(&format!(
            "declare i16 @llvm.{name}.i16(i16, i16, i32)
define i16 @f() {{
  %r = call i16 @llvm.{name}.i16(i16 {a}, i16 {b}, i32 {scale})
  ret i16 %r
}}
"
        ))
    };
    let shift = |name: &str, a: i16, by: i16| {
        result(&format!(
            "declare i16 @llvm.{name}.i16(i16, i16)
define i16 @f() {{
  %r = call i16 @llvm.{name}.i16(i16 {a}, i16 {by})
  ret i16 %r
}}
"
        ))
    };
    // 1.5 * 1.5 = 2.25 with 8 fraction bits.
    assert_eq!(fixed("umul.fix", 384, 384, 8), int(576, 16));
    assert_eq!(fixed("umul.fix.sat", 384, 384, 8), int(576, 16));
    // 200 * 200 = 40000 wraps to -25536 as signed, saturates to the maximum.
    assert_eq!(fixed("umul.fix", 200, 200, 0), int(40000, 16));
    assert_eq!(fixed("umul.fix.sat", -1, -1, 0), int(0xffff, 16));
    assert_eq!(fixed("smul.fix.sat", 200, 200, 0), int(0x7fff, 16));
    assert_eq!(fixed("smul.fix.sat", 200, -200, 0), int(0x8000, 16));
    assert_eq!(fixed("smul.fix.sat", -384, 384, 8), int(0xfdc0, 16));
    assert_eq!(shift("sshl.sat", 0x4000, 1), int(0x7fff, 16));
    assert_eq!(shift("sshl.sat", -0x4001, 1), int(0x8000, 16));
    assert_eq!(shift("sshl.sat", 3, 2), int(12, 16));
    assert_eq!(shift("ushl.sat", -1, 1), int(0xffff, 16));
    assert_eq!(shift("ushl.sat", 3, 2), int(12, 16));
}

use crate::{parse, print};

const DATALAYOUT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16\"\n";

/// Every `.ll` under `tests/`: hand-written mappings, clang's output, and
/// with `invalid`, the ones LLVM's verifier refuses instead.
fn fixtures(invalid: bool) -> Vec<(std::path::PathBuf, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut out = Vec::new();
    for dir in std::fs::read_dir(&root).expect("tests/").flatten() {
        if (dir.file_name() == "invalid") != invalid {
            continue;
        }
        for file in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            if file.path().extension().is_some_and(|one| one == "ll") {
                out.push((file.path(), std::fs::read_to_string(file.path()).expect("readable")));
            }
        }
    }
    out
}

/// `text` read and written back.
fn round(text: &str) -> String {
    print::module(&parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}")))
}

fn refusal(text: &str) -> String {
    parse::module(text).expect_err("refused").to_string()
}

#[test]
fn the_crate_depends_on_nothing() {
    // MIR may name no decoder, object file or target; the dependency graph
    // enforces it.
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest.split("[dependencies]").nth(1).expect("a [dependencies] table");
    // llrm-env is a leaf of its own, with no dependencies: the one reader of an
    // `LLRM_*` switch.
    let entries: Vec<&str> = dependencies
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("llrm-env "))
        .collect();
    assert!(entries.is_empty(), "{entries:?}");
}

#[test]
fn every_fixture_prints_to_a_fixed_point() {
    let fixtures = fixtures(false);
    assert!(fixtures.len() >= 17, "{} fixtures", fixtures.len());
    for (path, text) in fixtures {
        let module = parse::module(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(crate::verify::verify(&module), Vec::<String>::new(), "{}", path.display());
        let once = round(&text);
        assert_eq!(round(&once), once, "{}", path.display());
    }
}

#[test]
fn a_value_may_be_used_before_its_definition() {
    let text = format!(
        "{DATALAYOUT}\ndefine i16 @count(i16 %n) {{\nentry:\n  br label %head\n\nhead:\n  %i = phi i16 [ 0, %entry ], [ %next, %head ]\n  %next = add nsw i16 %i, 1\n  %more = icmp slt i16 %next, %n\n  br i1 %more, label %head, label %done\n\ndone:\n  ret i16 %next\n}}\n"
    );
    assert_eq!(round(&text), text);
}

#[test]
fn a_forward_use_must_agree_with_its_definition() {
    let text = "define i16 @f() {\nentry:\n  br label %b\n\nb:\n  %x = phi i16 [ 0, %entry ], [ %y, %b ]\n  %y = zext i16 %x to i32\n  br label %b\n}\n";
    assert_eq!(refusal(text), "line 7: %y is used as i16 but defined as i32");
}

#[test]
fn a_value_never_defined_is_refused() {
    let text = "define i16 @f() {\n  ret i16 %ghost\n}\n";
    assert_eq!(refusal(text), "line 2: %ghost is used but never defined");
}

#[test]
fn unnamed_values_count_the_entry_block() {
    let text = "define i16 @f(i16 %0) {\n  %2 = add i16 %0, 1\n  ret i16 %2\n}\n";
    assert_eq!(round(text), text);
    assert_eq!(refusal(&text.replace("%2", "%3")), "line 2: expected to be numbered %2");
}

#[test]
fn constructs_outside_the_subset_are_refused() {
    assert_eq!(refusal("@x = global fp128 zeroinitializer\n"), "line 1: `fp128` is outside MIR's subset of LLVM");
    assert_eq!(refusal("target triple = \"i386\"\n"), "line 1: `triple` is outside MIR's subset of LLVM");
}

#[test]
fn attribute_groups_may_follow_their_use() {
    let text = "declare void @stop(i16) #0\n\nattributes #0 = { noreturn memory(argmem: read) \"kind\"=\"error\" }\n";
    assert_eq!(round(text), "declare void @stop(i16) noreturn memory(argmem: read) \"kind\"=\"error\"\n");
}

#[test]
fn floating_constants_print_as_llvm_does() {
    // As LLVM 20's llvm-dis writes them.
    let text = "@a = global double 1.5\n@b = global double 0.1\n@c = global double 0x3FD5555555555555\n@d = global float 0x3FB99999A0000000\n";
    assert_eq!(
        round(text),
        "@a = global double 1.500000e+00\n@b = global double 1.000000e-01\n@c = global double 0x3FD5555555555555\n@d = global float 0x3FB99999A0000000\n"
    );
}

/// LLVM 24 (the pin) prints a float as its shortest decimal (`float
/// 1.000000e-01`) and a hex float as `f0x` and the type's own digits; MIR
/// refused both, so 14 vsgcc programs (floats, x_nbody_f, ...) were refused
/// before selection.
#[test]
fn a_float_decimal_is_read_as_a_float_and_f0x_takes_the_types_width() {
    // 0.1 as a float, not as a double that happens to be near one.
    assert_eq!(round("@a = global float 0.1\n"), "@a = global float 0x3FB99999A0000000\n");
    // Just above the midpoint of 1.0 and the next float: through a double it
    // is the midpoint, and ties to even give 1.0; read as a float it rounds up.
    assert_eq!(round("@a = global float 1.00000005960464477539062501\n"), "@a = global float 0x3FF0000020000000\n");
    assert_eq!(round("@a = global double f0x3FF0040000000000\n"), round("@a = global double 0x3FF0040000000000\n"));
    assert_eq!(round("@a = global float f0x3F800000\n"), round("@a = global float 1.0\n"));
    assert_eq!(
        refusal("@a = global float f0x3FF0040000000000\n"),
        "line 1: f0x3FF0040000000000 is not a float's 8 hex digits"
    );
}

/// The pin writes a `long double` as `f0x` and the 20 digits of x87's format
/// (sign and exponent, then the 64-bit significand); MIR refused it ("expected
/// a constant"), so `double_to_uint64.c` never compiled through `llrm-cc`. A
/// constant of the type holds the double nearest its value.
#[test]
fn f0x_takes_the_twenty_digits_of_an_x86_fp80() {
    // 2^64 - 2048 is a double; 1.0 is exponent 0x3FFF with the integer bit set.
    assert_eq!(
        round("@a = global x86_fp80 f0x403EFFFFFFFFFFFFF800\n"),
        round("@a = global x86_fp80 0x43EFFFFFFFFFFFFF\n")
    );
    assert_eq!(round("@a = global x86_fp80 f0x3FFF8000000000000000\n"), round("@a = global x86_fp80 1.0\n"));
    assert_eq!(round("@a = global x86_fp80 f0xC0008000000000000000\n"), round("@a = global x86_fp80 -2.0\n"));
    assert_eq!(
        refusal("@a = global double f0x3FFF8000000000000000\n"),
        "line 1: f0x3FFF8000000000000000 is not a double's 16 hex digits"
    );
}

#[test]
fn integer_constants_hold_their_bits_and_print_signed() {
    assert_eq!(round("@a = global i16 65535\n"), "@a = global i16 -1\n");
    assert_eq!(refusal("@a = global i16 65536\n"), "line 1: 65536 does not fit i16");
}

#[test]
fn globals_keep_their_definition_order_whatever_uses_them_first() {
    let text = "@first = global ptr @second\n@second = global i16 7\n";
    assert_eq!(round(text), text);
    assert_eq!(
        refusal("@first = global ptr addrspace(1) @second\n@second = global i16 7\n"),
        "line 1: a global in address space 0 used as ptr addrspace(1)"
    );
}

#[test]
fn metadata_keeps_its_numbers_whatever_mentions_it_first() {
    // Numbered by first mention, an attachment's node swapped with the one
    // it refers to, and clang's loop metadata never printed the same twice.
    let text = "define void @f() {\n  ret void, !x !1\n}\n\n!0 = !{!\"first\"}\n!1 = !{!0}\n";
    assert_eq!(round(text), text);
}

/// LLVM's `initializes`, which MIR did not read.
#[test]
fn initializes_is_read_and_printed_as_llvm_spells_it() {
    let text = "declare void @f(ptr initializes((0, 4), (8, 12)), ptr initializes((-2, 0)))\n";
    assert_eq!(round(text), text);
}

/// `appending` was refused: "expected `global` or `constant`, found
/// `appending`" (every `__attribute__((constructor))` of the QB runtime).
#[test]
fn appending_linkage_is_read_and_printed_as_llvm_spells_it() {
    let text = "@llvm.global_ctors = appending global [1 x { i32, ptr, ptr }] [{ i32, ptr, ptr } { i32 65535, ptr @f, ptr null }]\n\ndefine void @f() {\n  ret void\n}\n";
    let once = round(text);
    assert!(once.contains("@llvm.global_ctors = appending global [1 x { i32, ptr, ptr }]"), "{once}");
    assert_eq!(round(&once), once);
}

/// `@llvm.used`'s `section "llvm.metadata"` was read and then dropped by the
/// printer, so a printed module lost it (the pipeline's text round trips).
#[test]
fn a_global_section_survives_printing() {
    let text = "@llvm.used = appending global [1 x ptr] [ptr @f], section \"llvm.metadata\"\n\ndefine void @f() {\n  ret void\n}\n";
    assert_eq!(round(text), text);
}

#[test]
fn every_invalid_fixture_is_refused_for_its_reason() {
    let fixtures = fixtures(true);
    assert!(fixtures.len() >= 9, "{} fixtures", fixtures.len());
    for (path, text) in fixtures {
        let reason = text.lines().find_map(|line| line.strip_prefix("; invalid: ")).expect("a `; invalid:` line");
        let found = match parse::module(&text) {
            Err(error) => vec![error.to_string()],
            Ok(module) => crate::verify::verify(&module),
        };
        assert!(found.iter().any(|one| one.contains(reason)), "{}: {found:?}", path.display());
    }
}

/// LLVM replaces an intrinsic declaration's attributes with the intrinsic's
/// own; kept from the text, `cold` and `noundef` would claim what LLVM does
/// not, and a missing `memory(none)` would hide that the call is pure.
#[test]
fn an_intrinsic_declaration_takes_llvms_attributes() {
    let module =
        parse::module("declare i16 @llvm.smax.i16(i16 noundef, i16) cold\n").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        print::module(&module),
        "declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n"
    );
}

/// `Intrinsics.td` gives the copies and fills `nosync`; without it a pass
/// reads each as a possible synchronisation point.
#[test]
fn the_copies_and_fills_are_nosync() {
    for declaration in [
        "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)",
        "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)",
        "declare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)",
        "declare void @llvm.experimental.memset.pattern.p0.i16.i16(ptr, i16, i16, i1)",
    ] {
        let text = print::module(&parse::module(&format!("{declaration}\n")).expect("a module"));
        assert!(text.contains(" nosync "), "{text}");
    }
}

/// `memcpy`'s two pointers are `noalias` in `Intrinsics.td`: the copy does not
/// overlap, which `memmove` does not say.
#[test]
fn memcpy_pointers_are_noalias_and_memmove_pointers_are_not() {
    let text = |name: &str| {
        print::module(&parse::module(&format!("declare void @llvm.{name}.p0.p0.i16(ptr, ptr, i16, i1)\n")).unwrap())
    };
    assert!(text("memcpy").starts_with(
        "declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly,"
    ));
    assert!(!text("memmove").contains("noalias"));
}

/// An intrinsic MIR does not enumerate got no attributes, so every pass read
/// its call as unknown: `ctpop` as able to write memory and not return.
#[test]
fn a_declaration_takes_the_attributes_intrinsics_td_gives_its_intrinsic() {
    let print = |text: &str| print::module(&parse::module(text).unwrap_or_else(|error| panic!("{error}")));
    assert_eq!(
        print("declare i16 @llvm.ctpop.i16(i16)\n"),
        "declare i16 @llvm.ctpop.i16(i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n"
    );
    // Parameter attributes.
    assert_eq!(
        print("declare i16 @llvm.ctlz.i16(i16, i1)\n"),
        "declare i16 @llvm.ctlz.i16(i16, i1 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n"
    );
    // Memory the table does not state, and a target's own, by its whole name.
    assert_eq!(
        print("declare void @llvm.va_start.p0(ptr)\n"),
        "declare void @llvm.va_start.p0(ptr) nocallback nofree nosync nounwind willreturn\n"
    );
    assert_eq!(
        print("declare void @llvm.va_end.p0(ptr)\ndeclare void @llvm.va_copy.p0.p0(ptr, ptr)\n"),
        "declare void @llvm.va_end.p0(ptr) nocallback nofree nosync nounwind willreturn\n\n\
         declare void @llvm.va_copy.p0.p0(ptr, ptr) nocallback nofree nosync nounwind willreturn\n"
    );
    assert_eq!(
        print("declare i64 @llvm.x86.sse42.crc32.64.64(i64, i64)\n"),
        "declare i64 @llvm.x86.sse42.crc32.64.64(i64, i64) nocallback nofree nosync nounwind willreturn memory(none)\n"
    );
    // A return range, at the declared width.
    assert_eq!(
        print("declare i8 @llvm.ucmp.i8.i16(i16, i16)\n"),
        "declare range(i8 -1, 2) i8 @llvm.ucmp.i8.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n"
    );
    // A name that only starts like one is no intrinsic's.
    assert_eq!(print("declare i16 @llvm.ctpopx(i16)\n"), "declare i16 @llvm.ctpopx(i16)\n");
    // A type suffix on a name that is not overloaded is not its intrinsic.
    assert_eq!(
        print("declare i64 @llvm.x86.sse42.crc32.64.64.i64(i64, i64)\n"),
        "declare i64 @llvm.x86.sse42.crc32.64.64.i64(i64, i64)\n"
    );
}

/// A generated attribute MIR cannot parse back would make a printed module
/// unreadable; every one is a word the parser knows, and every memory
/// location a name it reads.
#[test]
fn every_generated_attribute_is_one_mir_reads() {
    use crate::intrinsic_attrs::{Attr, NAMES, SETS};
    let known = |word: &str| crate::opcode::FLAG_ATTRIBUTES.contains(&word);
    let mut memory = std::collections::BTreeSet::new();
    for &(name, set, _) in NAMES.iter() {
        let set = &SETS[set as usize];
        assert!(set.function.iter().all(|one| known(one)), "{name}: {:?}", set.function);
        for (_, attrs) in set.arguments {
            for attr in attrs.iter() {
                match attr {
                    Attr::Flag(word) => assert!(known(word), "{name}: {word}"),
                    Attr::Int(word, _) => assert!(crate::opcode::INT_ATTRIBUTES.contains(word), "{name}: {word}"),
                    Attr::Range(..) => {}
                }
            }
        }
        let parts: Vec<String> = set
            .memory
            .iter()
            .map(|(location, access)| location.map_or(access.to_string(), |one| format!("{one}: {access}")))
            .collect();
        memory.insert(parts.join(", "));
    }
    memory.remove("");
    for effects in &memory {
        let text = format!("declare void @f() memory({effects})\n");
        assert!(parse::module(&text).is_ok(), "{text}");
    }
}

/// Inline assembly's registers are in its callee's name; the declaration
/// must answer them, and a block that reaches no memory keeps its place
/// among the ports as their own state does.
#[test]
fn an_inline_assembly_declaration_is_checked_against_its_name() {
    let name = "llrm.ia16.asm.cd1a.ax.cx_dx.flags.n";
    let block = crate::intrinsics::asm(name).expect("parses");
    assert_eq!(
        (block.code, block.inputs, block.outputs, block.clobbers, block.memory),
        (
            vec![0xcd, 0x1a],
            vec!["ax".to_owned()],
            vec!["cx".to_owned(), "dx".to_owned()],
            vec!["flags".to_owned()],
            false
        )
    );
    assert_eq!(crate::intrinsics::asm_name(&crate::intrinsics::asm(name).unwrap()), name);
    let problems = |text: &str| crate::verify::verify(&parse::module(text).unwrap_or_else(|error| panic!("{error}")));
    let right = format!("declare {{i16, i16}} @{name}(i16)\n");
    assert_eq!(problems(&right), Vec::<String>::new());
    assert_eq!(
        print::module(&parse::module(&right).unwrap()),
        format!("declare {{ i16, i16 }} @{name}(i16) nounwind memory(inaccessiblemem: readwrite)\n")
    );
    assert!(problems(&format!("declare i16 @{name}(i16)\n"))[0].contains("incorrect return type"));
    assert!(problems(&format!("declare {{i16, i16}} @{name}(i16, i16)\n"))[0].contains("incorrect argument type"));
}

/// BCC stores 3.125L as 00 00 00 00 00 00 00 C8 00 40; a long double
/// global llrm lays down must hold the same ten bytes.
#[test]
fn test_a_double_is_laid_down_in_x87_extended_form() {
    let extended = |value: f64| crate::types::x87_extended(value.to_bits());
    assert_eq!(extended(3.125), [0, 0, 0, 0, 0, 0, 0, 0xC8, 0x00, 0x40]);
    assert_eq!(extended(-2.25), [0, 0, 0, 0, 0, 0, 0, 0x90, 0x00, 0xC0]);
    assert_eq!(extended(0.0), [0; 10]);
    assert_eq!(extended(f64::from_bits(1)), [0, 0, 0, 0, 0, 0, 0, 0x80, 0xCD, 0x3B]);
}

/// What a call does to memory the program can name, less what it does to
/// memory it cannot: a routine that ends the program by writing its own
/// state touches nothing the loop around the call can see.
#[test]
fn a_call_to_inaccessible_memory_touches_nothing_nameable() {
    let module = crate::parse::module(
        "declare void @stop() noreturn memory(inaccessiblemem: readwrite)
declare void @any()

define void @f() {
b0:
  call void @stop()
  call void @any()
  ret void
}
",
    )
    .expect("a module");
    let callees = crate::memory::callees(&module);
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let calls = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            crate::opcode::Opcode::Call(_)
        ))
        .collect::<Vec<_>>();
    let stop = crate::memory::accessible(&module.context, &callees, function, calls[0]);
    let any = crate::memory::accessible(&module.context, &callees, function, calls[1]);
    assert_eq!((stop, any), (crate::memory::Effects::NONE, crate::memory::Effects::ANY));
    assert!(
        crate::memory::of(&module.context, &callees, function, calls[0]).writes,
        "it still writes what it can reach"
    );
}

/// Verifier.cpp: `llvm.bswap` takes whole 16-bit units; an `i8` or an `i24` is
/// reported at the call, an `i16` is not.
#[test]
fn a_bswap_of_an_odd_number_of_bytes_is_reported() {
    let problems = |bits: u32| {
        crate::verify::verify(
            &parse::module(&format!(
                "declare i{bits} @llvm.bswap.i{bits}(i{bits})\n\
                 define i{bits} @f(i{bits} %x) {{\nb0:\n  %r = call i{bits} @llvm.bswap.i{bits}(i{bits} %x)\n  ret i{bits} %r\n}}\n"
            ))
            .unwrap_or_else(|error| panic!("{error}")),
        )
    };
    assert_eq!(problems(16), Vec::<String>::new());
    for bits in [8, 24] {
        let found = problems(bits);
        assert!(found.iter().any(|one| one.contains("bswap must be an even number of bytes")), "i{bits}: {found:?}");
    }
}

/// `llvm.invariant.start` and `.end` are intrinsics MIR has (`Intrinsics.td`):
/// a scope's call verifies, and the start's size is -1, 0 or positive
/// (Verifier.cpp#L6072). MIR said "an intrinsic MIR does not have" of both.
#[test]
fn an_invariant_scope_verifies_and_a_size_below_minus_one_is_reported() {
    let problems = |size: i64| {
        crate::verify::verify(
            &parse::module(&format!(
                "declare ptr @llvm.invariant.start.p0(i64 immarg, ptr nocapture)\n\
                 declare void @llvm.invariant.end.p0(ptr, i64 immarg, ptr nocapture)\n\
                 define void @f(ptr %p) {{\nb0:\n  %s = call ptr @llvm.invariant.start.p0(i64 {size}, ptr %p)\n  \
                 call void @llvm.invariant.end.p0(ptr %s, i64 {size}, ptr %p)\n  ret void\n}}\n"
            ))
            .unwrap_or_else(|error| panic!("{error}")),
        )
    };
    for size in [-1, 0, 4] {
        assert_eq!(problems(size), Vec::<String>::new(), "size {size}");
    }
    let found = problems(-2);
    assert!(
        found.iter().any(|one| one.contains("invariant_start parameter must be -1, 0 or a positive number")),
        "{found:?}"
    );
}

/// `Intrinsics.td` gives `invariant.start` the result `ptr`, whatever pointer
/// it covers: an `i32` result is a wrong signature.
#[test]
fn an_invariant_start_with_a_wrong_result_is_a_wrong_signature() {
    let found = crate::verify::verify(
        &parse::module("declare i32 @llvm.invariant.start.p0(i64 immarg, ptr nocapture)\n")
            .unwrap_or_else(|error| panic!("{error}")),
    );
    let error = found.join("\n");
    assert!(error.contains("incorrect return type"), "{error}");
}

/// `llvm.fshl`, `llvm.fshr` and `llvm.abs` are intrinsics MIR has: a call of
/// each verifies. The verifier said "an intrinsic MIR does not have" of all
/// three, and the funnel-shift and abs folds of InstCombine could not build
/// them.
#[test]
fn funnel_shifts_and_abs_are_known_intrinsics() {
    for text in [
        "declare i16 @llvm.fshl.i16(i16, i16, i16)\ndefine i16 @f(i16 %x, i16 %y) {\nb0:\n  %r = call i16 @llvm.fshl.i16(i16 %x, i16 %y, i16 3)\n  ret i16 %r\n}\n",
        "declare i32 @llvm.fshr.i32(i32, i32, i32)\ndefine i32 @f(i32 %x, i32 %y) {\nb0:\n  %r = call i32 @llvm.fshr.i32(i32 %x, i32 %y, i32 3)\n  ret i32 %r\n}\n",
        "declare i16 @llvm.abs.i16(i16, i1)\ndefine i16 @f(i16 %x) {\nb0:\n  %r = call i16 @llvm.abs.i16(i16 %x, i1 false)\n  ret i16 %r\n}\n",
    ] {
        let module = parse::module(text).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(crate::verify::verify(&module), Vec::<String>::new(), "{text}");
    }
}

fn dominance_problems(text: &str) -> Vec<String> {
    crate::verify::verify(&parse::module(text).unwrap_or_else(|error| panic!("{error}")))
        .into_iter()
        .filter(|one| one.contains("dominate"))
        .collect()
}

/// A value used in a branch arm that does not contain its definition, in
/// the join after it, or before it in the same block, is not dominated; the
/// verifier that runs after every pass must say so.
#[test]
fn a_use_not_dominated_by_its_definition_is_reported() {
    let diamond = |join: &str| {
        format!(
            "define i16 @f(i1 %c, i16 %x) {{
b0:
  br i1 %c, label %l, label %r
l:
  %a = add i16 %x, 1
  br label %j
r:
  br label %j
j:
{join}
}}
"
        )
    };
    assert_eq!(dominance_problems(&diamond("  ret i16 %x")), Vec::<String>::new());
    assert_eq!(
        dominance_problems(&diamond("  ret i16 %a")).len(),
        1,
        "a definition in one arm does not reach the join"
    );
    assert_eq!(
        dominance_problems(&diamond("  %p = phi i16 [ %a, %l ], [ %x, %r ]\n  ret i16 %p")),
        Vec::<String>::new()
    );
    assert_eq!(
        dominance_problems(&diamond("  %p = phi i16 [ %x, %l ], [ %a, %r ]\n  ret i16 %p")).len(),
        1,
        "a phi input must dominate its own edge"
    );
}

/// Within a block a use before the definition, and a use of the value
/// itself, are not dominated.
#[test]
fn a_use_before_its_definition_in_one_block_is_reported() {
    let before = "define i16 @f(i16 %x) {
b0:
  %b = add i16 %a, 1
  %a = add i16 %x, 1
  ret i16 %b
}
";
    assert_eq!(dominance_problems(before).len(), 1);
}

/// A module its frontend made with a use its definition does not dominate
/// was reported as the first pass's doing ("after mem2reg: ..."), sending
/// the search to the wrong crate; it is reported before any pass runs.
#[test]
fn a_module_made_wrong_is_reported_before_the_first_pass() {
    let mut module = parse::module(
        "define i16 @f(i16 %x) {
b0:
  %b = add i16 %a, 1
  %a = add i16 %x, 1
  ret i16 %b
}
",
    )
    .expect("parses");
    struct Idle;
    impl crate::passes::FunctionPass for Idle {
        fn name(&self) -> &'static str {
            "idle"
        }

        fn run(
            &mut self,
            _: &mut crate::passes::Unit,
            _: &mut crate::passes::Analyses,
        ) -> crate::passes::PreservedAnalyses {
            crate::passes::PreservedAnalyses::all()
        }
    }
    let mut manager = crate::passes::PassManager { verify_each: true, ..Default::default() };
    manager.add(Idle);
    let error = manager.run_module(&mut module, std::rc::Rc::new(crate::target::Neutral)).unwrap_err();
    assert!(error.starts_with("before the first pass:") && error.contains("dominate"), "{error}");
}

/// The recurrences a loop carries, for the loop corpus's `mir-ivs`: a far
/// pointer walked beside a counter is two, though only the counter has a
/// `Recurrence`. `wordlen` in BASIC counted one before the pass walked its
/// far pointer as an integer offset and two after, for the same loop.
#[test]
fn a_walked_pointer_is_a_recurrence_the_loop_carries() {
    let module = crate::parse::module(
        "define i16 @f(ptr addrspace(1) %far, i16 %n) {
entry:
  br label %l

l:
  %i = phi i16 [ 0, %entry ], [ %i.next, %l ]
  %p = phi ptr addrspace(1) [ %far, %entry ], [ %p.next, %l ]
  %v = load i16, ptr addrspace(1) %p
  %p.next = getelementptr i8, ptr addrspace(1) %p, i16 2
  %i.next = add i16 %i, 1
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %l, label %d

d:
  ret i16 %i.next
}

define i16 @g(ptr addrspace(1) %far, i16 %n) {
entry:
  br label %l

l:
  %i = phi i16 [ 0, %entry ], [ %i.next, %l ]
  %p = phi ptr addrspace(1) [ %far, %entry ], [ %p, %l ]
  %v = load i16, ptr addrspace(1) %p
  %i.next = add i16 %i, 1
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %l, label %d

d:
  ret i16 %i.next
}
",
    )
    .expect("a module");
    let counted = |name: &str| {
        let (_, _, function) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).expect("a function");
        let loops = crate::loopinfo::LoopInfo::analyze(function, || crate::domtree::DomTree::of(function));
        let evolution = crate::scalarevolution::Evolution::new(&module.context, function, &loops);
        let header = loops.get_loops_in_preorder()[0].get_header();
        evolution.counted(&module.context, function, &loops, header)
    };
    assert_eq!(
        (counted("f"), counted("g")),
        (2, 1),
        "a counter and a walked pointer, then a counter and a pointer that stays"
    );
}

/// `releases` read and written back, stated of a parameter: a copy of a routine
/// that frees its argument has to know it.
#[test]
fn releases_is_a_parameter_attribute_that_round_trips() {
    let text = format!("{DATALAYOUT}\ndeclare void @free(ptr releases)\n");
    let once = crate::print::module(&crate::parse::module(&text).unwrap());
    assert!(once.contains("declare void @free(ptr releases)"), "{once}");
}

/// `noretain` read and written back, stated of a parameter and of a call's
/// argument; before it existed the parser refused the attribute.
#[test]
fn noretain_is_a_parameter_attribute_that_round_trips() {
    let text = format!(
        "{DATALAYOUT}\ndeclare void @erase(ptr nocapture noretain)\n\ndefine void @f(ptr %p) {{\nb0:\n  call void @erase(ptr noretain %p)\n  ret void\n}}\n"
    );
    let once = round(&text);
    assert!(
        once.contains("declare void @erase(ptr nocapture noretain)")
            && once.contains("call void @erase(ptr noretain %p)"),
        "{once}"
    );
    assert_eq!(round(&once), once);
}

/// `noipa` is a function attribute (GCC's: no interprocedural pass uses the
/// body for anyone else); it is read, written back and found by `opaque`.
#[test]
fn a_noipa_routine_round_trips_through_text() {
    let text = format!(
        "{DATALAYOUT}\ndefine internal i16 @f(i16 %a) noipa {{\nb0:\n  ret i16 %a\n}}\n\ndefine internal i16 @g(i16 %a) {{\nb0:\n  ret i16 %a\n}}\n"
    );
    let once = round(&text);
    assert!(once.contains("noipa"), "{once}");
    assert_eq!(round(&once), once);
    let module = crate::parse::module(&once).expect("parses");
    let opaque: Vec<_> = module
        .functions()
        .filter(|(_, _, function)| crate::facts::opaque(function))
        .map(|(_, global, _)| global.name.owned().unwrap_or_default())
        .collect();
    assert_eq!(opaque, vec!["f".to_owned()]);
}

/// `nearcode` is a function attribute the Nib frontend and `nearcode::placed`
/// write (callgraph::NEAR_CODE); the parser refused it ("expected `{`, found
/// `nearcode`"), so llrm-mir --run could not read any module the compiler
/// dumped with a near-entered procedure, and tools/loops reported every such
/// case wrong (#1634).
#[test]
fn nearcode_is_a_function_attribute_that_round_trips() {
    let text =
        format!("{DATALAYOUT}\ndefine internal i16 @f(i16 %a) addrspace(1) nearcode {{\nb0:\n  ret i16 %a\n}}\n");
    let once = round(&text);
    assert!(once.contains("nearcode"), "{once}");
    assert_eq!(round(&once), once);
    assert!(crate::opcode::FLAG_ATTRIBUTES.contains(&crate::callgraph::NEAR_CODE));
}

/// `null_pointer_is_valid` is the attribute `null_pointer_is_defined` reads;
/// the parser refused it ("expected `}`, found `null_pointer_is_valid`"), so a
/// function carrying it could not be read and no pointer fact could depend on
/// it.
#[test]
fn null_pointer_is_valid_is_a_function_attribute_that_round_trips() {
    let text = format!("{DATALAYOUT}\ndefine i16 @f(i16 %a) null_pointer_is_valid {{\nb0:\n  ret i16 %a\n}}\n");
    let once = round(&text);
    assert!(once.contains("null_pointer_is_valid"), "{once}");
    assert_eq!(round(&once), once);
}

/// Clang's output for arm64 states `ssp` and `uwtable(sync)` on a function and
/// `captures(none)` on a parameter (LLVM 22's `nocapture`); the reader refused
/// each ("expected `}`, found `ssp`", "expected `,`, found `captures`"), so no
/// module from that front end could be read.
#[test]
fn clangs_target_attributes_and_captures_are_read() {
    let text = format!(
        "{DATALAYOUT}\ndefine void @f(ptr captures(none) %p) ssp uwtable(sync) {{\nb0:\n  store i16 0, ptr %p\n  ret void\n}}\n"
    );
    let once = round(&text);
    assert!(once.contains("nocapture") && once.contains("uwtable"), "{once}");
    assert_eq!(round(&once), once);
    let kept = round(&format!("{DATALAYOUT}\ndeclare void @g(ptr captures(address) %p)\n"));
    assert!(!kept.contains("nocapture"), "{kept}");
    // The attributes of by-value copies.
    round(&format!("{DATALAYOUT}\ndeclare i16 @h(ptr nofreeobj dead_on_return %p) nocreateundeforpoison\n"));
    // `allocsize(0, 1)` on a declaration (clang's `malloc`, `calloc`).
    round(&format!("{DATALAYOUT}\ndeclare ptr @calloc(i16, i16) allocsize(0, 1) allockind(\"alloc,zeroed\")\n"));
}

/// Each use asked `instruction_dominates` of its definition, which scanned the
/// block for both: the verifier was 12% of compiling 800 BASIC statements
/// (#560). The positions are made once.
#[test]
fn test_verifying_a_block_does_not_scan_it_for_each_use() {
    let chain: String = (1..300).map(|at| format!("  %v{at} = add i16 %v{}, 1\n", at - 1)).collect();
    let text = format!("{DATALAYOUT}define i16 @f(i16 %v0) {{\nentry:\n{chain}  ret i16 %v299\n}}\n");
    let module = parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let before = crate::domtree::scans();
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
    assert_eq!(crate::domtree::scans() - before, 0, "a use scanned its block");
}

/// A use before its definition in one block is still refused, by position.
#[test]
fn test_a_use_before_its_definition_in_one_block_is_refused() {
    let text = format!(
        "{DATALAYOUT}define i16 @f(i16 %a) {{\nentry:\n  %y = add i16 %x, 1\n  %x = add i16 %a, 1\n  ret i16 %y\n}}\n"
    );
    let problems = crate::verify::verify(&parse::module(&text).unwrap_or_else(|error| panic!("{error}")));
    assert!(problems.iter().any(|one| one.contains("does not dominate")), "{problems:?}");
}

/// `Declared::over` the module's declarations answers as `of` the module does:
/// a name the module has is its id, a new one the next, and `place` says how
/// many it added (the caller's held declarations are then stale).
#[test]
fn test_declared_over_the_declarations_answers_as_declared_of_the_module() {
    use crate::passes::{Declarations, Declared, ModuleAnalyses};
    let mut module =
        parse::module(&format!("{DATALAYOUT}declare void @known()\n")).unwrap_or_else(|error| panic!("{error}"));
    let ty = module.global(module.named("known").unwrap()).function().unwrap().ty;
    let held = ModuleAnalyses::of(&module, std::rc::Rc::new(crate::target::Neutral)).get::<Declarations>(&module);
    let (mut over, mut of) = (Declared::over(held, module.metadata.len()), Declared::of(&module));
    let known = module.named("known").unwrap();
    assert_eq!((over.declare("known", ty), of.declare("known", ty)), (known, known));
    let fresh = over.declare("fresh", ty);
    assert_eq!(fresh, of.declare("fresh", ty));
    assert_eq!(over.place(&mut module).unwrap(), 1);
    assert_eq!(module.named("fresh"), Some(fresh));
    assert_eq!(Declared::over(Default::default(), 0).place(&mut module).unwrap(), 0);
}

/// Hash and tree maps keyed by a dense id cost 12% of a compile in hashing and
/// 7% in tree nodes where gcc uses bitmaps and vectors: new keyed maps and sets
/// use `dense::IdMap` / `IdSet`. A file may not gain one; the ceilings in
/// `dense-keys.txt` fall as files convert (clippy's `disallowed-types` cannot
/// tell a `HashMap<ValueId, _>` from any other).
///
/// A type alias hides its key from the text (`pub type Intervals =
/// IndexMap<ValueId, Interval>`): the alias is resolved and each use of its
/// name counts as one keyed map, the alias's own line as none.
fn dense_key_counts(sources: &[(String, String)]) -> std::collections::BTreeMap<String, usize> {
    let kinds = ["HashMap<", "HashSet<", "BTreeMap<", "BTreeSet<", "IndexMap<", "IndexSet<"];
    let ids = ["ValueId", "InstId", "BlockId", "module::ValueId", "module::InstId", "module::BlockId"]
        .map(String::from)
        .into_iter()
        .flat_map(|id| {
            ["".to_owned(), "crate::".to_owned(), "llrm_mir::".to_owned()].into_iter().map(move |prefix| prefix + &id)
        })
        .collect::<Vec<_>>();
    let keyed = |text: &str| -> usize {
        kinds
            .iter()
            .map(|kind| {
                text.match_indices(kind)
                    .filter(|(at, _)| {
                        ids.iter().any(|id| {
                            text[at + kind.len()..].trim_start().starts_with(id)
                                && !text[at + kind.len()..].trim_start()[id.len()..]
                                    .starts_with(|c: char| c.is_alphanumeric() || c == '_')
                        })
                    })
                    .count()
            })
            .sum()
    };
    // An alias is a `type` at the start of a line (an associated type in an
    // impl is indented), its right side a keyed map.
    let is_alias = |line: &str| {
        (line.starts_with("type ") || line.starts_with("pub type ") || line.starts_with("pub(crate) type "))
            && line.contains(" = ")
            && keyed(line) > 0
    };
    // (the file defining it, the name)
    let aliases: Vec<(&str, String)> = sources
        .iter()
        .flat_map(|(file, text)| text.lines().filter(|line| is_alias(line)).map(move |line| (file.as_str(), line)))
        .filter_map(|(file, line)| {
            line.split_once("type ")
                .and_then(|(_, rest)| rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next())
                .map(|name| (file, name.to_owned()))
        })
        .collect();
    // A generic alias (`type Sparse<K, V> = IndexMap<K, V>`) names no id on its
    // own line: it is a keyed map wherever it is applied to an id.
    // SparseIdMap and SparseIdSet are the sanctioned ones (a few of many ids:
    // llrm_support::hash), by name and no other.
    const SANCTIONED: [&str; 2] = ["SparseIdMap", "SparseIdSet"];
    let generic_aliases: Vec<(&str, String)> = sources
        .iter()
        .flat_map(|(file, text)| text.lines().map(move |line| (file.as_str(), line)))
        .filter(|(_, line)| {
            line.starts_with("type ") || line.starts_with("pub type ") || line.starts_with("pub(crate) type ")
        })
        .filter_map(|(file, line)| {
            let (head, right) = line.split_once(" = ")?;
            let after = head.split_once("type ")?.1;
            let (name, params) = after.split_once('<')?;
            let first = params.split([',', '>']).next()?.trim();
            let key_of_map = kinds
                .iter()
                .any(
                    |kind| right.find(kind).is_some_and(|at| right[at + kind.len()..].trim_start().starts_with(first)),
                );
            (!first.is_empty()
                && key_of_map
                && !SANCTIONED.contains(&name)
                && !kinds.contains(&format!("{name}<").as_str()))
            .then(|| (file, name.to_owned()))
        })
        .collect();
    let whole_word = |text: &str, at: usize, word: &str| {
        !text[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_')
            && !text[at + word.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
    };
    let mut counts = std::collections::BTreeMap::new();
    for (name, text) in sources {
        let body: String = text.lines().filter(|line| !is_alias(line)).collect::<Vec<_>>().join("\n");
        let mut found = keyed(&body);
        for (home, alias) in &aliases {
            // A name some other file also uses for something else (`Facts`,
            // `Calls`) is the alias only where the file defines it,
            // imports it, or writes it as a path.
            let module = std::path::Path::new(home).file_stem().and_then(|stem| stem.to_str()).unwrap_or("");
            let imported = body
                .lines()
                .any(
                    |line| line.trim_start().starts_with("use ")
                        && line.contains(module)
                        && line.match_indices(alias.as_str()).any(|(at, _)| whole_word(line, at, alias)),
                );
            let qualified = body.contains(&format!("{module}::{alias}"));
            if name != home && !imported && !qualified {
                continue;
            }
            let uses: String =
                body.lines().filter(|line| !line.trim_start().starts_with("use ")).collect::<Vec<_>>().join("\n");
            found += uses.match_indices(alias.as_str()).filter(|(at, _)| whole_word(&uses, *at, alias)).count();
        }
        // Where a generic alias is applied to an id.
        for (home, alias) in &generic_aliases {
            let module = std::path::Path::new(home).file_stem().and_then(|stem| stem.to_str()).unwrap_or("");
            let imported = body
                .lines()
                .any(
                    |line| line.trim_start().starts_with("use ")
                        && line.contains(module)
                        && line.match_indices(alias.as_str()).any(|(at, _)| whole_word(line, at, alias)),
                );
            if name != home && !imported && !body.contains(&format!("{module}::{alias}")) {
                continue;
            }
            let uses: String = body
                .lines()
                .filter(|line| {
                    !line.trim_start().starts_with("use ")
                        && !line.starts_with("type ")
                        && !line.starts_with("pub type ")
                })
                .collect::<Vec<_>>()
                .join("\n");
            found += uses
                .match_indices(&format!("{alias}<"))
                .filter(|(at, _)| whole_word(&uses, *at, &format!("{alias}<")[..alias.len()]))
                .filter(|(at, _)| {
                    ids.iter().any(|id| {
                        uses[at + alias.len() + 1..].trim_start().starts_with(id.as_str())
                            && !uses[at + alias.len() + 1..].trim_start()[id.len()..]
                                .starts_with(|c: char| c.is_alphanumeric() || c == '_')
                    })
                })
                .count();
        }
        counts.insert(name.clone(), found);
    }
    counts
}

/// SparseIdMap is the sanctioned container for a few of many ids; any other
/// alias of a keyed map, generic or not, counts where it is applied to an id: a
/// new alias is no loophole.
#[test]
fn the_sparse_containers_are_sanctioned_by_name_and_a_generic_alias_is_not() {
    let source = |name: &str, text: &str| (name.to_owned(), text.to_owned());
    let counts = dense_key_counts(&[
        source("hash.rs", "pub type SparseIdMap<K, V> = IndexMap<K, V>;\npub type Sparse<K, V> = IndexMap<K, V>;\n"),
        source(
            "a.rs",
            "use llrm_support::hash::SparseIdMap;\nfn f(x: SparseIdMap<ValueId, u8>, y: SparseIdMap<InstId, u8>) {}\n",
        ),
        source("b.rs", "use llrm_support::hash::Sparse;\nfn f(x: Sparse<ValueId, u8>, y: Sparse<String, u8>) {}\n"),
    ]);
    assert_eq!(counts["a.rs"], 0, "SparseIdMap is sanctioned");
    assert_eq!(counts["b.rs"], 1, "a generic alias applied to an id is one keyed map; applied to a string it is none");
    assert_eq!(counts["hash.rs"], 0, "the alias lines themselves count none");
}

/// A ceiling for a file that moved or went away in the re-org stayed in
/// `dense-keys.txt`: 8 of them were left, naming nothing, and a ceiling that
/// names nothing guards nothing.
#[test]
fn every_dense_key_ceiling_names_a_file_that_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let stale: Vec<&str> = include_str!("../dense-keys.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .map(|(file, _)| file)
        .filter(|file| !root.join(file).exists())
        .collect();
    assert!(stale.is_empty(), "dense-keys.txt names files that are gone: {stale:?}");
}

#[test]
fn no_file_gains_a_hash_or_tree_map_keyed_by_a_dense_id() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let ceilings: std::collections::BTreeMap<&str, usize> = include_str!("../dense-keys.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .map(|(file, count)| (file, count.parse().expect("a count")))
        .collect();
    let mut sources = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("a directory reads").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.strip_prefix(&root).expect("under the root").to_string_lossy().into_owned();
            if !name.ends_with(".rs")
                || name.ends_with("_tests.rs")
                || name.ends_with("/tests.rs")
                || name.contains("/tests/")
            {
                continue;
            }
            sources.push((name, std::fs::read_to_string(&path).expect("a source reads")));
        }
    }
    let over: Vec<String> = dense_key_counts(&sources)
        .into_iter()
        .filter(|(name, found)| *found > ceilings.get(name.as_str()).copied().unwrap_or(0))
        .map(|(name, found)| format!("{name}: {found} > {}", ceilings.get(name.as_str()).copied().unwrap_or(0)))
        .collect();
    assert!(over.is_empty(), "use llrm_mir::dense::{{IdMap, IdSet}} for ids: {over:?}");
}

/// Copies of a value's use list: `users(..)` followed by `.snapshot()`. LLVM
/// walks the list in place (`make_early_inc_range`); each copy is a place a
/// pass could walk it instead, and costs the whole list.
fn slice_user_counts(sources: &[(String, String)]) -> std::collections::BTreeMap<String, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for (name, text) in sources {
        let mut found = 0;
        // `.users(` and `.block_users(`, but not `all_users(` or `fn users(`.
        for (at, _) in
            text.match_indices("users(").filter(|(at, _)| matches!(text[..*at].chars().last(), Some('.' | '_')))
        {
            let mut depth = 0;
            let mut end = None;
            for (i, c) in text[at + "users".len()..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(at + "users".len() + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let rest = end.map_or("", |end| text[end..].trim_start());
            found += [".snapshot()"].iter().filter(|one| rest.starts_with(**one)).count();
        }
        if found > 0 {
            counts.insert(name.clone(), found);
        }
    }
    counts
}

#[test]
fn no_file_gains_a_slice_read_of_a_use_list() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let ceilings: std::collections::BTreeMap<&str, usize> = include_str!("../slice-users.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .map(|(file, count)| (file, count.parse().expect("a count")))
        .collect();
    let mut sources = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("a directory reads").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.strip_prefix(&root).expect("under the root").to_string_lossy().into_owned();
            if name.ends_with(".rs")
                && !name.ends_with("_tests.rs")
                && !name.ends_with("/tests.rs")
                && !name.contains("/tests/")
            {
                sources.push((name, std::fs::read_to_string(&path).expect("a source reads")));
            }
        }
    }
    let over: Vec<String> = slice_user_counts(&sources)
        .into_iter()
        .filter(|(name, found)| *found > ceilings.get(name.as_str()).copied().unwrap_or(0))
        .map(|(name, found)| format!("{name}: {found} > {}", ceilings.get(name.as_str()).copied().unwrap_or(0)))
        .collect();
    assert!(
        over.is_empty(),
        "copy of a use list (`users(v).snapshot()`); walk it (`iter()`, `only()`, `has_one_use()`) instead: {over:?}"
    );
}

/// A scan that finds nothing in a tree full of such reads is the broken
/// instrument: the three shapes count, an iteration and a nested call do not.
#[test]
fn a_copy_of_a_use_list_is_counted_and_a_walk_is_not() {
    let source = |text: &str| vec![("a.rs".to_owned(), text.to_owned())];
    let text = "f.users(v).snapshot(); f.block_users(g(v)).snapshot(); f.users(v)\n    .snapshot(); f.users(v).iter(); f.users(v).only();";
    assert_eq!(slice_user_counts(&source(text)).get("a.rs"), Some(&3));
}

/// `pub type Intervals = IndexMap<ValueId, Interval>` once let two files hold
/// keyed maps the text scan could not see; each use of the alias is one.
#[test]
fn an_alias_of_a_keyed_map_and_three_uses_of_it_count_as_three() {
    let source = |name: &str, text: &str| (name.to_owned(), text.to_owned());
    let counts = dense_key_counts(&[
        source("defines.rs", "pub type Intervals = IndexMap<ValueId, Interval>;\nfn a(x: Intervals) {}\n"),
        source("uses.rs", "use crate::defines::Intervals;\nfn b(x: &Intervals) -> Intervals { todo!() }\n"),
        source("other.rs", "fn c(x: &IntervalsIndex, y: Vec<Intervals>) {}\n    type Result = BTreeSet<InstId>;\n"),
    ]);
    assert_eq!(counts["defines.rs"], 1);
    assert_eq!(counts["uses.rs"], 2);
    assert_eq!(counts["defines.rs"] + counts["uses.rs"], 3);
    assert_eq!(
        counts["other.rs"], 1,
        "a file that neither imports nor defines the alias is no user of it (a name like Facts is another file's own); an indented associated type is no alias, but a keyed set itself"
    );
}

/// `depth` loops one inside the next: loop k's header `h{k}` goes into the next
/// loop, or the body of the innermost, and out to the latch of the loop round
/// it.
fn nested(depth: usize) -> String {
    let mut text = String::from("define void @f(i32 %n) {\nentry:\n  br label %h0\n\n");
    for k in 0..depth {
        let before = if k == 0 { "entry".to_owned() } else { format!("h{}", k - 1) };
        let inside = if k + 1 < depth { format!("h{}", k + 1) } else { "body".to_owned() };
        let outside = if k == 0 { "x".to_owned() } else { format!("l{}", k - 1) };
        text += &format!(
            "h{k}:\n  %i{k} = phi i32 [ 0, %{before} ], [ %n{k}, %l{k} ]\n  %c{k} = icmp slt i32 %i{k}, %n\n  br i1 %c{k}, label %{inside}, label %{outside}\n\n"
        );
    }
    text += &format!("body:\n  br label %l{}\n\n", depth - 1);
    for k in 0..depth {
        text += &format!("l{k}:\n  %n{k} = add i32 %i{k}, 1\n  br label %h{k}\n\n");
    }
    text + "x:\n  ret void\n}\n"
}

/// Every loop's blocks were a flood from its latches, and its parent the
/// nearest larger loop holding all of them: a nest of d loops took d times its
/// blocks and d squared times the loops' sets (quicksort -O3, five deep: a 20 x
/// slope in `analysis loops`). `LoopInfo` names the same loops, parents and
/// innermost loops.
#[test]
fn a_nest_of_loops_is_found_in_the_blocks_and_not_the_blocks_times_the_depth() {
    use crate::domtree::DomTree;
    use crate::loopinfo::LoopInfo;
    let depth = 12;
    let module = parse::module(&nested(depth)).expect("a module");
    let function = module.functions().next().expect("@f").2;
    let info = LoopInfo::analyze(function, || DomTree::of(function));
    let loops = info.get_loops_in_preorder();
    assert_eq!(loops.len(), depth);
    for (at, one) in loops.iter().enumerate() {
        // Outermost first: loop `at` is the one whose header is `h{at}`, and
        // holds the `depth - at` loops from there in, each `h`, `l`
        // pair, and the innermost's body.
        let named = |name: &str| {
            function.layout().iter().copied().find(|&block| function.block(block).name.as_deref() == Some(name))
        };
        let header = named(&format!("h{at}")).expect("a header");
        assert_eq!(one.get_header(), header);
        assert_eq!(one.get_num_blocks(), 2 * (depth - at) + 1, "loop {at}");
        assert_eq!(one.get_parent_loop().as_ref(), at.checked_sub(1).map(|up| &loops[up]), "loop {at}");
        let latch = named(&format!("l{at}")).expect("a latch");
        assert_eq!(one.get_loop_latch(function), Some(latch));
        assert!(one.contains(latch) && one.contains(header));
        assert_eq!(info.get_loop_depth(latch), (at + 1) as u32);
    }
    assert!(loops[depth - 1].get_blocks().iter().any(|&block| function.block(block).name.as_deref() == Some("body")));
}

/// Twice the nest, twice the walk: the blocks a function's loops are found in
/// grow with the blocks, not with their depth.
#[test]
fn the_loops_of_a_function_are_found_in_time_that_grows_with_its_blocks() {
    use crate::domtree::DomTree;
    use crate::loopinfo::{LoopInfo, visits};
    let visited = |depth: usize| {
        let module = parse::module(&nested(depth)).expect("a module");
        let function = module.functions().next().expect("@f").2;
        let before = visits();
        LoopInfo::analyze(function, || DomTree::of(function));
        visits() - before
    };
    let (small, large) = (visited(40), visited(80));
    assert!(large < 3 * small, "{small} blocks walked for 40 loops, {large} for 80");
}

/// Allocations the calling thread made, for the test below.
mod counted {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static MADE: Cell<usize> = const { Cell::new(0) };
    }

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(
            &self,
            layout: Layout,
        ) -> *mut u8 {
            MADE.with(|made| made.set(made.get() + 1));
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(
            &self,
            pointer: *mut u8,
            layout: Layout,
        ) {
            unsafe { System.dealloc(pointer, layout) }
        }

        unsafe fn realloc(
            &self,
            pointer: *mut u8,
            layout: Layout,
            size: usize,
        ) -> *mut u8 {
            MADE.with(|made| made.set(made.get() + 1));
            unsafe { System.realloc(pointer, layout, size) }
        }
    }

    pub fn made() -> usize {
        MADE.with(Cell::get)
    }
}

#[global_allocator]
static COUNTING: counted::Counting = counted::Counting;

/// The fixed point told a state it had been in by printing the body and keeping
/// the text: 1.4% of a d_faces compile in formatting and comparing. A digest
/// of the body stands for its text, and makes no allocation a line of it.
#[test]
fn a_body_is_told_apart_by_a_digest_that_agrees_with_its_text_and_prints_nothing() {
    let mut seen = 0;
    for (path, text) in fixtures(false) {
        let Ok(module) = parse::module(&text) else { continue };
        // A constant is numbered by its context: bodies of one module.
        let mut text_of = crate::hash::HashMap::<u128, String>::default();
        let mut digest_of = crate::hash::HashMap::<String, u128>::default();
        for global in &module.globals {
            let crate::module::GlobalKind::Function(function) = &global.kind else { continue };
            let (body, digest) = (print::body(&module.context, function), print::fingerprint(function));
            assert_eq!(
                *digest_of.entry(body.clone()).or_insert(digest),
                digest,
                "{}: one text, two digests",
                path.display()
            );
            assert_eq!(
                *text_of.entry(digest).or_insert(body.clone()),
                body,
                "{}: one digest, two texts",
                path.display()
            );
            seen += 1;
        }
    }
    assert!(seen > 50, "premise: the fixtures hold functions ({seen})");

    let mut body = String::from("define i16 @f(i16 %x) {\nb0:\n  %a0 = add i16 %x, 1\n");
    for k in 1..300 {
        body += &format!("  %a{k} = add i16 %a{}, {k}\n", k - 1);
    }
    body += "  ret i16 %a299\n}\n";
    let module = parse::module(&body).expect("parses");
    let function = module
        .globals
        .iter()
        .find_map(
            |one| match &one.kind {
                crate::module::GlobalKind::Function(function) => Some(function),
                _ => None,
            },
        );
    let function = function.expect("a function");
    let before = counted::made();
    let _ = print::fingerprint(function);
    let made = counted::made() - before;
    assert!(made <= 40, "a digest of 300 instructions made {made} allocations");
    let before = counted::made();
    let _ = print::body(&module.context, function);
    assert!(counted::made() - before > 300, "premise: printing allocates a string a line");
}

/// `undef` was a word MIR did not read: a function returning it was refused.
/// It prints as `undef`, apart from `poison`.
#[test]
fn undef_is_a_constant_that_round_trips_apart_from_poison() {
    let text = format!("{DATALAYOUT}\ndefine i16 @f() {{\nentry:\n  ret i16 undef\n}}\n");
    let once = round(&text);
    assert!(once.contains("ret i16 undef"), "{once}");
    assert_eq!(round(&once), once);
}

/// A global's cached intrinsic followed neither a rename nor a copy would
/// leave `intrinsic()` naming the old callee: a `memset` read as a plain call.
#[test]
fn a_globals_intrinsic_follows_its_name_through_renames_and_copies() {
    use crate::{intrinsics::Intrinsic, module::GlobalName};
    let mut module = parse::module("declare void @f(ptr, i8, i16, i1)\n").unwrap();
    let id = module.named("f").expect("declared");
    let intrinsic = |module: &crate::module::Module| module.global(id).intrinsic();
    assert_eq!(intrinsic(&module), None);
    module.globals[id.0 as usize].name = GlobalName::from("llvm.memset.p0.i16".to_owned());
    assert_eq!(intrinsic(&module), Some(Intrinsic::MemSet));
    assert_eq!(module.global(id).declaration().intrinsic(), Some(Intrinsic::MemSet));
    assert_eq!(module.clone().global(id).intrinsic(), Some(Intrinsic::MemSet));
    module.globals[id.0 as usize].name = GlobalName::from("f".to_owned());
    assert_eq!(intrinsic(&module), None);
    module.globals[id.0 as usize].name = GlobalName::default();
    assert_eq!(intrinsic(&module), None);
}

/// LLVM's `+inf`, `-qnan`, `+nan(0x..)` and `snan(0x..)` spellings were
/// refused (`unexpected character +`): 84 of the 619 floating-point test files
/// of Transforms/InstSimplify and Transforms/InstCombine stopped at the first
/// (#3180). Each reads as the double it names.
#[test]
fn the_special_floating_point_spellings_read_as_their_doubles() {
    let text = "define double @f(double %x) {\nentry:\n  %a = fadd double %x, +inf\n  %b = fadd double %a, -inf\n  %c = fadd double %b, +qnan\n  %d = fadd double %c, -qnan\n  %e = fadd double %d, +nan(0x1)\n  %f = fadd double %e, +snan(0x1)\n  ret double %f\n}\n";
    let printed = round(text);
    for bits in [
        "0x7FF0000000000000",
        "0xFFF0000000000000",
        "0x7FF8000000000000",
        "0xFFF8000000000000",
        "0x7FF8000000000001",
        "0x7FF0000000000001",
    ] {
        assert!(printed.contains(bits), "{bits} is not in\n{printed}");
    }
}

/// A routine whose result is the flags is read by comparisons alone: a fold
/// to `ashr v, 15` of its sign left instruction selection no way to read it.
#[test]
fn a_flags_result_read_other_than_by_a_comparison_is_refused() {
    let text = |user: &str| {
        format!(
            "declare i16 @cmp(ptr, ptr) flagsresult\n\
             define i16 @f(ptr %a, ptr %b) {{\nb0:\n  %v = call i16 @cmp(ptr %a, ptr %b)\n{user}  ret i16 %r\n}}\n"
        )
    };
    let problems =
        |user: &str| crate::verify::verify(&parse::module(&text(user)).unwrap_or_else(|error| panic!("{error}")));
    assert_eq!(problems("  %c = icmp slt i16 %v, 0\n  %r = sext i1 %c to i16\n"), Vec::<String>::new());
    let refused = problems("  %r = ashr i16 %v, 15\n");
    assert!(
        refused.len() == 1 && refused[0].contains("returns flags that something other than a comparison reads"),
        "{refused:?}"
    );
}

/// A constant of a packed identified struct printed as `{ .. }`, the unpacked
/// spelling: the pin's llc refused the IR a `--dump` wrote
/// ("packed'ness of initializer and type don't match") for 8 of QCport's 65
/// modules, so their D-G1 count was missing.
#[test]
fn a_packed_named_struct_initializer_prints_packed() {
    let text = "%s = type <{ i8, i16 }>\n@g = global %s <{ i8 1, i16 2 }>\n";
    let printed = round(text);
    assert!(printed.contains("global %s <{ i8 1, i16 2 }>"), "{printed}");
}

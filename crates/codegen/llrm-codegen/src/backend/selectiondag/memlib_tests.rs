//! A variable-size `memcpy`/`memset` calls the runtime's routine only where
//! the target's `libcalls` name it; elsewhere it is `rep movsb`/`stosb`, and
//! a `memmove` is refused, as the old selector does. A call of a routine the
//! runtime lacks does not link: before, the DAG path emitted one regardless.

use llrm_mir::datalayout::DataLayout;

use super::builder::{BuilderTarget, build_block};
use super::function_lowering::FunctionLoweringInfo;

const BODY: &str = "target datalayout = \"e-p:32:32-n8:16:32\"\n\
    declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
    declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)\n\
    declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)\n\
    define void @copy(ptr %d, ptr %s, i32 %n) {\n  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)\n  ret void\n}\n\
    define void @move(ptr %d, ptr %s, i32 %n) {\n  call void @llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)\n  ret void\n}\n\
    define void @fill(ptr %d, i8 %v, i32 %n) {\n  call void @llvm.memset.p0.i32(ptr %d, i8 %v, i32 %n, i1 false)\n  ret void\n}\n";

fn built(
    function_name: &str,
    target: &BuilderTarget,
) -> Result<String, &'static str> {
    built_from(BODY, function_name, target)
}

fn built_from(
    text: &str,
    function_name: &str,
    target: &BuilderTarget,
) -> Result<String, &'static str> {
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some(function_name))
        .unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    build_block(
        &module,
        function,
        &layout,
        target,
        &llrm_x86_m32::M32,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .map(|built| built.dag.dump())
    .map_err(|why| why.0)
}

#[test]
fn a_variable_copy_calls_memcpy_only_if_the_runtime_has_it() {
    let off = built("copy", &BuilderTarget::for_pointer_bits(32).with_libcalls(&[])).expect("builds");
    assert!(off.contains("REP_MOVS") && !off.contains("'memcpy'"), "{off}");
    let on = built("copy", &BuilderTarget::for_pointer_bits(32).with_libcalls(&["memcpy"])).expect("builds");
    assert!(on.contains("TargetExternalSymbol:i32'memcpy'") && !on.contains("REP_MOVS"), "{on}");
}

#[test]
fn a_variable_fill_calls_memset_only_if_the_runtime_has_it() {
    let off = built("fill", &BuilderTarget::for_pointer_bits(32).with_libcalls(&[])).expect("builds");
    assert!(off.contains("REP_STOS") && !off.contains("'memset'"), "{off}");
    let on = built("fill", &BuilderTarget::for_pointer_bits(32).with_libcalls(&["memset"])).expect("builds");
    assert!(on.contains("TargetExternalSymbol:i32'memset'") && !on.contains("REP_STOS"), "{on}");
}

#[test]
fn a_variable_move_is_refused_without_a_memmove() {
    assert!(built("move", &BuilderTarget::for_pointer_bits(32).with_libcalls(&[])).is_err());
    let on = built("move", &BuilderTarget::for_pointer_bits(32).with_libcalls(&["memmove"])).expect("builds");
    assert!(on.contains("TargetExternalSymbol:i32'memmove'"), "{on}");
}

/// A memmove proved to run forward is lowered as any memmove, loads before
/// stores: as a memcpy its pieces were free to reorder, and `for (i < 10) b[i]
/// = b[i + 3]` copied the high words first and read back what it had written
/// (memmove_overlap.c -O2, wrong output at m16 and m32).
#[test]
fn a_forward_move_is_lowered_as_a_move() {
    let plain = BODY.replace(
        "@llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)",
        "@llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 10, i1 false)",
    );
    let forward = plain.replace("i32 10, i1 false)", "i32 10, i1 false), !llrm.forward !0") + "!0 = !{}\n";
    assert_ne!(plain, forward);
    let dump = |text: &str| {
        let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let (id, _, function) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some("move")).unwrap();
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        build_block(
            &module,
            function,
            &layout,
            &BuilderTarget::for_pointer_bits(32).with_libcalls(&[]),
            &llrm_x86_m32::M32,
            &convention,
            &mut info,
            function.entry().unwrap(),
        )
        .unwrap_or_else(|why| panic!("{}", why.0))
        .dag
        .dump()
    };
    assert_eq!(dump(&forward), dump(&plain));
}

/// The libcalls the m32 runtime's `c.toml` names, read from the file.
fn real_m32_fact() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../runtime/c/x86-m32/c.toml");
    let table: toml::Table = std::fs::read_to_string(path).expect("c.toml").parse().expect("parses");
    table
        .get("libcalls")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|one| one.as_str().map(str::to_owned))
        .collect()
}

/// The fact the m32 runtime states: `memcpy`, `memmove` and `memset`, all three
/// in ext.asm and (as `memfuncs.asm`) in the QB runtime; while the QB link
/// lacked them the fact was none (qbrt 183/218 with it on). The builder has
/// neither their symbols nor their convention yet, so with the real fact it
/// refuses the variable fill, move and copy rather than call one in the wrong
/// convention; the names given by hand (above) are the pin's shape.
#[test]
fn the_real_m32_fact_names_memcpy_memmove_and_memset() {
    assert_eq!(real_m32_fact(), ["memcpy", "memmove", "memset"]);
    for function in ["fill", "move", "copy"] {
        let why = built(function, &BuilderTarget::for_pointer_bits(32)).expect_err(function);
        assert!(why.contains("symbol and convention are the target fact's"), "{function}: {why}");
    }
}

/// Every routine a runtime's `c.toml` lists in `libcalls` is defined in the
/// runtime (its `ext.asm` and the files it includes), under the name its
/// register ABI gives (`memmove_`) or the stack one (`_memmove`, `_memmove@3`):
/// a call of one that is not there does not link.
#[test]
fn every_libcall_a_c_toml_names_is_in_its_runtime() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../runtime/c");
    for entry in std::fs::read_dir(&root).expect("runtime/c") {
        let directory = entry.expect("entry").path();
        let Ok(text) = std::fs::read_to_string(directory.join("c.toml")) else { continue };
        let table: toml::Table = text.parse().expect("parses");
        let names: Vec<&str> = table
            .get("libcalls")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
            .collect();
        let mut source = std::fs::read_to_string(directory.join("ext.asm")).expect("ext.asm");
        for line in source.clone().lines().filter_map(|line| line.trim().strip_prefix("include ")) {
            source += &std::fs::read_to_string(directory.join(line.trim())).expect("an included file");
        }
        for name in names {
            let defined = |symbol: String| source.lines().any(|line| line.trim() == format!("public {symbol}"));
            assert!(
                defined(format!("{name}_")) || defined(format!("_{name}")) || defined(format!("_{name}@3")),
                "{}: libcalls names {name}, which no `public` of ext.asm defines",
                directory.display()
            );
        }
    }
}

/// Inline memset width at x86-64: the pin's `getOptimalMemOpType` gives `v4i32`
/// for sixteen bytes with SSE2, so a 16-byte clear is one `xorps` and one
/// 16-byte store, not two `mov qword [..], 0` (9 B each; nbody O2 +31 B).
#[test]
fn a_16_byte_clear_at_m64_is_one_16_byte_store() {
    let ir = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"\n\
        @g = global [16 x i8] zeroinitializer\n\
        declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
        define void @f() {\n  call void @llvm.memset.p0.i64(ptr @g, i8 0, i64 16, i1 false)\n  ret void\n}\n";
    let module = llrm_mir::parse::module(ir).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some("f"))
        .unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m64::M64).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let dump = build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::for_pointer_bits(64),
        &llrm_x86_m64::M64,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .map(|built| built.dag.dump())
    .unwrap_or_else(|why| panic!("{}", why.0));
    assert_eq!(dump.matches("store (s64)").count(), 0, "{dump}");
    assert_eq!(dump.matches("LLRM_ZERO128").count(), 1, "{dump}");
}

/// m64 memory_expansion -O1: `mv64` was eight `mov r64,[src+N]` loads and eight
/// stores (64 B) where the pin's `getMemmoveLoadsAndStores` takes
/// `getOptimalMemOpType` = `v4i32` and does four `movups` loads, four stores
/// (32 B); lc64 +55, mv33 +17, 14 functions in all. A 64-byte `memmove` at m64
/// is one `LLRM_MOVE128` node of four pieces.
#[test]
fn a_64_byte_memmove_at_m64_moves_four_16_byte_pieces() {
    let ir = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"\n\
        declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)\n\
        define void @f(ptr %d, ptr %s) {\n  call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 64, i1 false)\n  ret void\n}\n";
    let module = llrm_mir::parse::module(ir).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module
        .functions()
        .find(|(_, global, function)| !function.is_declaration() && global.name.as_deref() == Some("f"))
        .unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m64::M64).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let dump = build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::for_pointer_bits(64),
        &llrm_x86_m64::M64,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .map(|built| built.dag.dump())
    .unwrap_or_else(|why| panic!("{}", why.0));
    assert_eq!(dump.matches("load (s64)").count(), 0, "{dump}");
    assert_eq!(dump.matches("LLRM_MOVE128").count(), 1, "{dump}");
}

/// The memory_expansion Os cell grew 31 B once `memmove` of 64 bytes became the pin's four `movups` pairs (31 B) where
/// gcc -Os and the library call are `mov edx, 64; jmp memmove` (10 B): tuned for size on x86-64 a constant copy whose
/// pieces take more bytes than the call is the call, and a 16-byte one (one pair, 8 B) stays inline.
#[test]
fn a_constant_copy_for_size_at_m64_is_a_call_where_the_pieces_take_more_bytes() {
    let tuning = llrm_mir::target::Tuning {
        size_copy_price: Some(llrm_mir::target::CopyPrice { call: 14, wide_piece: 8, narrow_piece: 6 }),
        ..llrm_mir::target::Tuning::PIN
    };
    let tti = llrm_x86::tti::X86::of_cpu("x86-64").expect("a cpu").with_tuning(tuning);
    let target = BuilderTarget::for_pointer_bits(64).with_libcalls(&["memcpy", "memmove"]).with_tti(Some(std::rc::Rc::new(tti)));
    let text = |bytes: u64, call: &str| {
        format!(
            "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"\n\
            declare void @llvm.{call}.p0.p0.i64(ptr, ptr, i64, i1)\n\
            define void @f(ptr %d, ptr %s) optsize {{\n  call void @llvm.{call}.p0.p0.i64(ptr %d, ptr %s, i64 {bytes}, i1 false)\n  ret void\n}}\n"
        )
    };
    for call in ["memmove", "memcpy"] {
        let long = built_from(&text(64, call), "f", &target).expect("builds");
        assert!(long.contains(&format!("'{call}'")) && !long.contains("LLRM_MOVE128"), "{call} 64\n{long}");
        let short = built_from(&text(16, call), "f", &target).expect("builds");
        assert!(!short.contains(&format!("'{call}'")), "{call} 16\n{short}");
    }
}

/// A constant clear for size, with no `memset` in the runtime, was `rep stosb`:
/// a clock per byte, 512 clocks for matmul.nib's three arrays (+669 clocks at
/// -Os). It is `rep stos` of whole pointer-width units at that alignment.
#[test]
fn a_constant_clear_for_size_without_memset_steps_by_whole_words() {
    let module = "target datalayout = \"e-p:32:32-n8:16:32\"\n\
        declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)\n\
        define void @clear(ptr align 4 %d) optsize {\n  call void @llvm.memset.p0.i32(ptr align 4 %d, i8 0, i32 256, i1 false)\n  ret void\n}\n";
    let dump = built_from(module, "clear", &BuilderTarget::for_pointer_bits(32).with_libcalls(&[])).expect("builds");
    assert!(dump.contains("$ecx, Constant:i32<64>") && dump.contains("ValueType:ch:i32"), "{dump}");
}

/// x_mergesort -O3 at m32: the three copy loops of `msort` became `call
/// _memcpy` (three pushes, the call, the callee's setup: ~35 clk each, 72146
/// clk against 54487 with the loops left). With the target's tuning bit a
/// variable length is `rep movsd` inline at speed, and the call at size.
#[test]
fn a_variable_copy_is_rep_movs_at_speed_where_the_tuning_says_so() {
    let target = BuilderTarget::for_pointer_bits(32).with_libcalls(&["memcpy"]);
    let before = built("copy", &target).expect("builds");
    assert!(before.contains("'memcpy'") && !before.contains("REP_MOVS"), "premise: the pin's call\n{before}");
    let mut tuning = llrm_mir::target::Tuning::PIN;
    tuning.inline_variable_memcpy = true;
    let tti = llrm_x86::tti::X86::of_cpu("i486").expect("a cpu").with_tuning(tuning);
    let target = target.with_tti(Some(std::rc::Rc::new(tti)));
    let after = built("copy", &target).expect("builds");
    assert!(after.contains("REP_MOVS") && !after.contains("'memcpy'"), "{after}");
}

/// x_jpeg_single -O1 at m32: `next()` inlined twice into `getbit` with the refill's variable
/// copy as an inline `rep movs` (about 30 B each against a call's 13), the cell 1762 B over
/// the best external. MEM-1 trades bytes for clocks at -O2/-O3 only: -O1 keeps the call.
#[test]
fn a_variable_copy_stays_a_call_where_bytes_are_strict() {
    let mut tuning = llrm_mir::target::Tuning::PIN;
    tuning.inline_variable_memcpy = true;
    let tti = llrm_x86::tti::X86::of_cpu("i486").expect("a cpu").with_tuning(tuning);
    let target = BuilderTarget::for_pointer_bits(32)
        .with_libcalls(&["memcpy"])
        .with_tti(Some(std::rc::Rc::new(tti)))
        .with_bytes_strict(true);
    let dump = built("copy", &target).expect("builds");
    assert!(dump.contains("'memcpy'") && !dump.contains("REP_MOVS"), "-O1 keeps the call:\n{dump}");
}

/// x_mergesort -O3: a copy of `n << 2` bytes ended in a `rep movsb` of `xor
/// ecx, ecx` (a dead tail, ~5 clk and three instructions per copy, 1818
/// copies). A length with its low two bits known zero has one `rep movs`.
#[test]
fn a_dword_multiple_variable_copy_has_no_byte_tail() {
    let text = "target datalayout = \"e-p:32:32-n8:16:32\"\n\
        declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
        define void @cells(ptr %d, ptr %s, i32 %n) {\n  %b = shl i32 %n, 2\n  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %b, i1 false)\n  ret void\n}\n";
    let mut tuning = llrm_mir::target::Tuning::PIN;
    tuning.inline_variable_memcpy = true;
    let tti = llrm_x86::tti::X86::of_cpu("i486").expect("a cpu").with_tuning(tuning);
    let target = BuilderTarget::for_pointer_bits(32).with_libcalls(&["memcpy"]).with_tti(Some(std::rc::Rc::new(tti)));
    let dump = built_from(text, "cells", &target).expect("builds");
    assert_eq!(dump.matches("REP_MOVS").count(), 1, "{dump}");
}

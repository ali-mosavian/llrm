use std::collections::BTreeMap;

use llrm_mir::datalayout::DataLayout;

use super::super::builder::{BuilderTarget, build_block};
use super::super::dump::normalized;
use super::super::function_lowering::FunctionLoweringInfo;

/// The pin's "Initial selection DAG" dumps by block name (`f:b1`).
fn pin_blocks(text: &str) -> BTreeMap<String, String> {
    let mut blocks = BTreeMap::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Initial selection DAG: ") {
            if let Some((name, body)) = current.take() {
                blocks.insert(name, body);
            }
            let name = rest.split('\'').nth(1).expect("quoted name").to_string();
            current = Some((name, String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    blocks.extend(current);
    blocks.into_iter().map(|(name, body)| (name, normalized(&body))).collect()
}

/// Our dump of every block the builder takes, by the pin's block name.
fn our_blocks(text: &str) -> (BTreeMap<String, String>, Vec<String>) {
    let text: String =
        text.lines().filter(|line| !line.starts_with("target triple")).map(|line| format!("{line}\n")).collect();
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let layout = module
        .datalayout
        .as_deref()
        .map_or_else(DataLayout::default, |text| DataLayout::parse(text).expect("a datalayout"));
    let target = BuilderTarget::m32().with_libcalls(&["memcpy", "memmove", "memset"]);
    let mut built = BTreeMap::new();
    let mut refused = Vec::new();
    for (id, global, function) in module.functions() {
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        for &block in function.layout() {
            let name =
                format!("{}:{}", global.name.as_deref().unwrap(), function.block(block).name.as_deref().unwrap_or(""));
            match build_block(&module, function, &layout, &target, &llrm_x86_m32::M32, &convention, &mut info, block) {
                Ok(done) => {
                    built.insert(name, normalized(&done.dag.dump()));
                }
                Err(why) => refused.push(format!("{name}: {}", why.0)),
            }
        }
    }
    (built, refused)
}

/// Every block of the scalar function the builder takes has the DAG the
/// pin builds for the same IR (ids, register numbers, memory operand text
/// and block addresses aside). Without the builder, nothing is built.
#[test]
fn scalar_blocks_match_the_pins_initial_dag() {
    let ll = include_str!("../golden/scalar.ll");
    let pin = pin_blocks(include_str!("../golden/scalar.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 4, "all four blocks are built");
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

/// Entry blocks lower the arguments and `ret` copies to the return
/// register: the same DAG as the pin's for stack arguments (`load` from
/// `FrameIndex<-1>`, ...), an argument read in another block (`CopyToReg`
/// of the load), and `RET_GLUE`. Without `lower_arguments` the first
/// block is refused for its arguments.
#[test]
fn arguments_and_returns_match_the_pins_initial_dag() {
    let ll = include_str!("../golden/args.ll");
    let pin = pin_blocks(include_str!("../golden/args.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 4, "the entry of f, e and n of h, the entry of v");
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

/// A direct call: `callseq_start`, the stack stores, `X86ISD::CALL`,
/// `callseq_end` and the result copied from `$eax`, as in the pin. A call
/// is refused before this piece.
/// Floating point: loads and stores of `f32`/`f64`, `fadd`/`fmul`/`fsub`,
/// `fneg`, `fcmp`, conversions, an FP constant, and a `ret` of the value
/// (RET_GLUE's operand, st(0) naming no register). Floats were refused.
#[test]
fn floating_point_matches_the_pins_initial_dag() {
    let ll = include_str!("../golden/fp.ll");
    let pin = pin_blocks(include_str!("../golden/fp.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 2);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

/// `llvm.memcpy`/`llvm.memset`: the libcall (variable or large size) and
/// `REP_MOVS`/`REP_STOS` (constant size up to 128 bytes at alignment 4)
/// are the pin's. An intrinsic call was refused.
#[test]
fn memory_intrinsics_match_the_pins_initial_dag() {
    let ll = include_str!("../golden/mem.ll");
    let pin = pin_blocks(include_str!("../golden/mem.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 6);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

/// Small constant sizes are loads and stores (overlapping at the end),
/// and a REP whose size is not a multiple of the block adds the tail as
/// stores: the pin's. These were refused.
#[test]
fn small_memory_intrinsics_match_the_pins_initial_dag() {
    let ll = include_str!("../golden/memsmall.ll");
    let pin = pin_blocks(include_str!("../golden/memsmall.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 9);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

/// Constant expressions (`getelementptr` of a global, `ptrtoint` of one)
/// build the pin's `add GlobalAddress, offset`. The shadow bucket
/// "constant expression" (lru, scroll, x_bigadd) was refused.
#[test]
fn constant_expressions_match_the_pins_initial_dag() {
    let ll = include_str!("../golden/constexpr.ll");
    let pin = pin_blocks(include_str!("../golden/constexpr.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 4);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

/// `fshl`/`fshr` (a rotate when both inputs are one value),
/// `lifetime.start`/`end` of a static alloca, and `memmove`, inline or a
/// libcall: the pin's. Each stopped the builder as an "intrinsic call".
#[test]
fn funnel_lifetime_and_memmove_match_the_pins_initial_dag() {
    let ll = include_str!("../golden/intrinsics.ll");
    let pin = pin_blocks(include_str!("../golden/intrinsics.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 5);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

#[test]
fn indirect_calls_match_the_pins_initial_dag() {
    let ll = include_str!("../golden/indirect.ll");
    let pin = pin_blocks(include_str!("../golden/indirect.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new(), "a call of a value was refused");
    assert_eq!(ours.len(), 2);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

#[test]
fn calls_match_the_pins_initial_dag() {
    let ll = include_str!("../golden/calls.ll");
    let pin = pin_blocks(include_str!("../golden/calls.pin.txt"));
    let (ours, refused) = our_blocks(ll);
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(ours.len(), 2);
    for (name, dag) in &ours {
        assert_eq!(dag, pin.get(name).expect("the pin has the block"), "block {name}");
    }
}

/// A `watcallcc` call passes its first four word arguments in AX, DX, BX,
/// CX: the call's register copies are `CopyToReg` of those, in that order,
/// glued, each read back as an operand of `X86ISD::CALL`. A call that
/// left them on the stack would show `store` nodes instead.
#[test]
fn a_watcom_call_copies_ax_dx_bx_cx_in_order() {
    let text = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\
        declare watcallcc i16 @w(i16, i16, i16, i16)\n\
        define i16 @f(i16 %a, i16 %b, i16 %c, i16 %d) {\n  %r = call watcallcc i16 @w(i16 %a, i16 %b, i16 %c, i16 %d)\n  ret i16 %r\n}\n";
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m16::M16).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let built = build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::m16(),
        &llrm_x86_m16::M16,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .unwrap_or_else(|why| panic!("{}", why.0));
    let dump = built.dag.dump();
    let copies: Vec<&str> = dump.lines().filter(|line| line.contains("CopyToReg") && line.contains(" $")).collect();
    let registers: Vec<&str> =
        copies.iter().filter_map(|line| line.split("Register:i16 $").nth(1)?.split(',').next()).collect();
    assert_eq!(registers, ["ax", "dx", "bx", "cx", "ax"], "four arguments, then the result:\n{dump}");
    assert!(!dump.contains("store"), "{dump}");
}

/// At real mode `i32` is a register type (the pin's `In16BitMode` has `GR32`
/// legal) and `i64` is two of them. With `i8`/`i16` only, an `i32` was split
/// into `i16` halves (`extract_element`, `build_pair`) and an `i64` into
/// four: 47 corpus blocks refused at "wider than two registers", and the
/// legalizer saw a pair for every `long`.
#[test]
fn an_i32_is_one_register_and_an_i64_two_at_m16() {
    let text = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\
        define i16 @narrow(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  %t = trunc i32 %r to i16\n  ret i16 %t\n}\n\
        define i16 @wide(i64 %a, i64 %b) {\n  %r = add i64 %a, %b\n  %t = trunc i64 %r to i16\n  ret i16 %t\n}\n";
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let dump_of = |name: &str| {
        let (id, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).unwrap();
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m16::M16).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        build_block(
            &module,
            function,
            &layout,
            &BuilderTarget::m16(),
            &llrm_x86_m16::M16,
            &convention,
            &mut info,
            function.entry().unwrap(),
        )
        .unwrap_or_else(|why| panic!("{name}: {}", why.0))
        .dag
        .dump()
    };
    let narrow = dump_of("narrow");
    assert!(narrow.contains("i32 = add") && !narrow.contains("extract_element"), "{narrow}");
    let wide = dump_of("wide");
    assert!(
        wide.contains("i64 = build_pair") && wide.contains("i32,ch = load") && !wide.contains("i16,ch = load"),
        "{wide}"
    );
}

/// At real mode the default convention returns an `i32` in `dx:ax` though
/// `i32` is a register type: the return value is two `i16` halves, low in
/// `ax`, glued, and a call reads them back and pairs them. Before, both
/// stopped at "result in registers the value does not fill" (14 QB blocks).
#[test]
fn an_i32_returned_in_dx_ax_is_two_halves_at_m16() {
    let text = "target datalayout = \"e-p:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\
        declare i32 @g(i16)\n\
        define i32 @f(i16 %a) {\n  %r = call i32 @g(i16 %a)\n  %s = add i32 %r, 1\n  ret i32 %s\n}\n";
    let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m16::M16).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let dump = build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::m16(),
        &llrm_x86_m16::M16,
        &convention,
        &mut info,
        function.entry().unwrap(),
    )
    .unwrap_or_else(|why| panic!("{}", why.0))
    .dag
    .dump();
    assert!(dump.contains("i32 = build_pair") && dump.contains("Register:i16 $dx"), "{dump}");
    let returned: Vec<&str> = dump
        .lines()
        .filter(|line| line.contains("CopyToReg"))
        .filter_map(|line| line.split("Register:i16 $").nth(1)?.split(',').next())
        .collect();
    assert_eq!(returned, ["ax", "dx"], "the result's low half in ax, the high in dx:\n{dump}");
}

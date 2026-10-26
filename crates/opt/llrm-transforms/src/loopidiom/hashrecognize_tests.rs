//! `HashRecognize`: the CRC loops it takes and the ones it leaves.

use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::expander::Env;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::interpret::{Val, run};
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId, Module};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::AssumptionCache;

use super::hashrecognize::{PolynomialInfo, gen_sarwate_table, recognize_crc};
use super::process::{AaProvider, Run};
use super::{Library, LoopIdiomRecognize, Options};

/// The loop `%h` of `text`'s function through `recognize_crc` at 8 trips.
fn recognize(text: &str) -> Result<PolynomialInfo, &'static str> {
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("a function");
    let block = |name: &str| -> BlockId {
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some(name)).expect("a block")
    };
    let (h, exit) = (block("h"), block("exit"));
    let phi = function.iter_block(h).next().expect("a phi");
    let cmp: InstId = function
        .iter_block(h)
        .filter(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))
        .last()
        .expect("the latch compare");
    recognize_crc(&module.context, function, h, exit, phi, cmp, 8)
}

const LITTLE: &str = "define i32 @f(i32 %c0) {
entry:
  br label %h

h:
  %iv = phi i32 [ 0, %entry ], [ %iv2, %h ]
  %c = phi i32 [ %c0, %entry ], [ %sel, %h ]
  %b = and i32 %c, 1
  %t = icmp eq i32 %b, 0
  %sh = lshr i32 %c, 1
  %x = xor i32 %sh, -306674912
  %sel = select i1 %t, i32 %sh, i32 %x
  %iv2 = add i32 %iv, 1
  %e = icmp eq i32 %iv2, 8
  br i1 %e, label %exit, label %h

exit:
  %r = phi i32 [ %sel, %h ]
  ret i32 %r
}
";

/// The bitwise CRC-32 loop `c = (c & 1) ? 0xEDB88320 ^ (c >> 1) : c >> 1` is
/// little-endian with that polynomial.
#[test]
fn the_bitwise_crc32_loop_is_recognized() {
    let info = recognize(LITTLE).expect("a CRC");
    assert_eq!((info.rhs, info.width, info.is_big_endian, info.lhs_aux), (0xEDB88320, 32, false, None));
}

/// A value of the loop used outside the CRC's chains (`%extra`) is a stray
/// instruction: not a CRC.
#[test]
fn a_stray_instruction_in_the_loop_is_not_a_crc() {
    let text = LITTLE
        .replace("  %iv2 = add", "  %extra = add i32 %c, 5\n  %iv2 = add")
        .replace("%r = phi i32 [ %sel, %h ]", "%r = phi i32 [ %sel, %h ]");
    assert_eq!(recognize(&text), Err("Found stray unvisited instructions"));
}

/// The Sarwate table of CRC-32: entry 1 is `0x77073096`, entry 255
/// `0x2D02EF8D`.
#[test]
fn the_sarwate_table_of_crc32_matches_the_known_entries() {
    let table = gen_sarwate_table(0xEDB88320, 32, false);
    assert_eq!((table[0], table[1], table[255]), (0, 0x77073096, 0x2D02EF8D));
}

/// `@f` after `LoopIdiomRecognize::run_on_loop` on its loop `%h`: the module
/// with the pass's function and the tables it declared.
fn idiom(
    text: &str,
    for_size: bool,
) -> (Option<bool>, Module) {
    struct NoAa;
    impl AaProvider for NoAa {
        fn with(
            &self,
            _: &llrm_mir::context::Context,
            _: &llrm_mir::module::Function,
            _: &mut dyn FnMut(&mut dyn llrm_analysis::aa::AAResults) -> bool,
        ) -> bool {
            unreachable!("a CRC loop has no store")
        }
    }
    let mut module = llrm_mir::parse::module(text).expect("a module");
    let mut function = module.functions().find(|(_, _, f)| !f.is_declaration()).map(|(_, _, f)| f.clone()).expect("f");
    let mut context = module.context.clone();
    let layout = DataLayout::default();
    let tree = DomTree::of(&function);
    let loops = LoopInfo::analyze(&function, || &tree);
    let assumptions = AssumptionCache::new(&module, &function);
    let env = Env {
        globals: &module.globals,
        metadata: &module.metadata,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let header: BlockId =
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let cur_loop = loops.get_loop_for(header).expect("a loop");
    let mut se = ScalarEvolution::new();
    let mut declared = llrm_mir::passes::Declared::of(&module);
    let library = Library { memset: true, memset_pattern16: false, memcpy: true };
    let changed = {
        let mut run = Run {
            name: "f",
            env: &env,
            context: &mut context,
            function: &mut function,
            se: &mut se,
            aa: &NoAa,
            declared: &mut declared,
            cur_loop,
        };
        let mut idiom = LoopIdiomRecognize::new(Options::default(), library);
        idiom.for_size = for_size;
        idiom.run_on_loop(&mut run)
    };
    module.context = context;
    declared.place(&mut module).expect("the table is placed");
    *module.function_mut("f").expect("f").1 = function;
    (changed, module)
}

fn call(
    module: &Module,
    input: u32,
) -> u128 {
    match run(module, "f", vec![Val::Int { bits: u128::from(input), width: 32 }], 100_000) {
        Ok(Val::Int { bits, .. }) => bits,
        other => panic!("a value, not {other:?}"),
    }
}

/// The eight-trip bitwise CRC-32 loop used to compute the result for any
/// input; the table loop gives the same on every one.
#[test]
fn the_table_lookup_computes_what_the_bitwise_loop_did() {
    let before = llrm_mir::parse::module(LITTLE).expect("a module");
    let (changed, after) = idiom(LITTLE, false);
    assert_eq!(changed, Some(true), "the CRC loop is rewritten");
    for input in [0, 1, 0xFF, 0x1234_5678, 0x8000_0000, u32::MAX] {
        assert_eq!(call(&after, input), call(&before, input), "input {input:#x}");
    }
    let text = llrm_mir::print::module(&after);
    assert!(text.contains("crctable"), "{text}");
    assert!(!text.contains("select"), "{text}");
}

/// At -Os the pin keeps the bitwise loop (`ApplyCodeSizeHeuristics`); the
/// table is 1 KB of data, and m16 -Os x_crc32 grew by it.
#[test]
fn a_size_compile_keeps_the_bitwise_crc_loop() {
    let (changed, after) = idiom(LITTLE, true);
    assert_eq!(changed, Some(false));
    assert!(!llrm_mir::print::module(&after).contains("crctable"));
}

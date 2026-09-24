//! `tests/test_hir.py` QB cases, part 1; helpers in `test_hir`.
//!
//! skipped: test_qb_driver_never_replays_a_stale_in_tree_release_binary --
//! unsetting QBOPT_QBFRONT is process-global and races the parallel tests
//! that need it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use super::abi::physicalize;
use super::main::parse_args;
use super::qbstages;
use super::stage_text;
use super::test_hir::*;
use crate::backend::lower as lower_mir;
use crate::backend::masm;
use crate::hir::{self, model};
use crate::model::ir::{self, Loc, Operation};
use crate::model::lir;
use crate::model::mir::{self, Arg};
use crate::model::passes::O2;
use crate::objectfile::omf;

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|one| (*one).to_owned()).collect()
}

fn qb45(source: &std::path::Path) -> model::Program {
    parsed_as(source, "qb45", "qb45")
}

fn lowered(program: &model::Program) -> Vec<hir::Lowered> {
    hir::lower(program).expect("lowers")
}

/// `namespace["dumped"](source, output, dialect=..., runtime=..., includes=())`.
fn dumped(source: &std::path::Path, output: &std::path::Path, dialect: &str, runtime: &str) {
    let frontend = qbstages::Frontend {
        dialect: dialect.into(),
        runtime: runtime.into(),
        array_order: "column-major".into(),
        ..Default::default()
    };
    qbstages::dumped(source, output, &frontend, &O2()).expect("dumps");
}

fn machine(
    name: &str,
    body: &mir::MirBody,
    calls: &IndexMap<i64, String>,
    contracts: &IndexMap<i64, crate::abi::runtime::Contract>,
    pointer_model: Option<crate::backend::pointers::Model>,
) -> lir::LirBody {
    let occurrences = IndexMap::default();
    lower_mir::lowered(
        name,
        body,
        Some(calls),
        BTreeSet::new(),
        Some(contracts),
        "386",
        lower_mir::Lowered { occurrences: Some(&occurrences), pointer_model, ..Default::default() },
    )
    .expect("lowers")
}

fn hir_calls(function: &model::Function) -> Vec<String> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.op == model::Op::Call)
        .map(|instruction| instruction.callee.clone().unwrap_or_default())
        .collect()
}

fn externals_by_fixup(records: &[std::rc::Rc<omf::Record>]) -> BTreeSet<String> {
    let externals = omf::externals(records);
    omf::fixups(records)
        .iter()
        .filter(|fixup| fixup.target == "external")
        .map(|fixup| externals[fixup.index as usize].clone())
        .collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|one| (*one).to_owned()).collect()
}

/// qb-qrender is built with BC /R; the object CLI formerly hid that semantic option.
#[test]
fn test_qb_cli_exposes_bc_row_major_array_order() {
    let args = parse_args(&argv(&["probe.bas", "--array-order", "row-major"])).expect("parses");
    assert_eq!(args.frontend.array_order, "row-major");
}

/// PDHUGE wrapped at 64 KiB when the object CLI silently dropped BC /Ah.
#[test]
fn test_qb_cli_exposes_pds_huge_array_option() {
    let args = parse_args(&argv(&["probe.bas", "--huge-arrays"])).expect("parses");
    assert!(args.frontend.huge_arrays);
}

/// PDFPA linked BCL71ANR but its module header still claimed BC /FPi.
#[test]
fn test_qb_cli_exposes_pds_alternate_math_option() {
    let args = parse_args(&argv(&["probe.bas", "--alternate-math"])).expect("parses");
    assert!(args.frontend.alternate_math);
}

/// The SYS stage showcase crashed when FSTP carried its target through effects only.
#[test]
fn test_lir_stage_formats_operandless_x87_store_as_intel() {
    let what = ir::Semantics { name: Some("fstp".into()), ..ir::Semantics::new(Operation::FloatStore) };
    assert_eq!(stage_text::instruction_text(&what), ["fstp"]);
}

/// Q45N01 stopped at READ, then a native-only spill frame made READ report syntax error.
#[test]
#[ignore = "the Python original fails too: q45n01 now has no spill, so no B$ENRA/B$EXSA"]
fn test_qb45_numeric_read_data_reaches_typed_hir_and_fresh_omf() {
    let source = root().join("frontends/qb/compat/qb45/q45n01.bas");
    let program = qb45(&source);
    let main = &program.modules[0].functions[0];
    let calls = hir_calls(main);

    assert_eq!(calls[..7], ["B$RDI2", "B$RDI2", "B$RDI4", "B$RDI4", "B$RDI4", "B$RDR4", "B$RDR4"]);
    assert!(lowered(&program).iter().all(|one| mir::verify(&one.body).is_empty()));

    let records = records(&program, "q45n01.bas");
    let data = image(&records, "BC_DS");
    assert_eq!(&data[2..], b" 17, 3, 100000, 3, 7, 1, 2\x00\xff\xff\x01");
    let externals = omf::externals(&records);
    assert!(externals_by_fixup(&records).is_superset(&set(&["B$RDI2", "B$RDI4", "B$RDR4"])));

    let listing = listing(&program);
    assert!(set(&["B$ENRA", "B$EXSA"]).is_subset(&externals.into_iter().collect()));
    assert!(listing.contains("call far ptr B$RDI2"));
    assert!(listing.contains("call far ptr B$RDI4"));
    assert!(listing.contains("call far ptr B$RDR4"));
}

/// Gorillas' synthetic DATA keys made B$RSTB fault before its first READ.
#[test]
fn test_restore_keys_select_the_labeled_serialized_data_row() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(&tmp, "restore.bas", b"restore later\r\nfirst: data 1\r\nlater: data 2\r\n");
    let program = qb45(&source);
    let assembled = assembled(&program).expect("assembles");
    let records = records(&program, "restore.bas");
    let segments = omf::segments(&records);
    let code_size = segments[1].as_ref().unwrap().1;
    let (ds_index, _) = segment(&records, "BC_DS");
    let code = omf::segment_image(&records, 1, code_size);
    let read_data = image(&records, "BC_DS");
    let first = usize::from(u16::from_le_bytes([read_data[0], read_data[1]]));
    let second_at = read_data[2..].iter().position(|byte| *byte == 0).unwrap() + 2 + 1;
    let second = usize::from(u16::from_le_bytes([read_data[second_at], read_data[second_at + 1]]));

    // BC emits one 90h marker per DATA row and stores those final code offsets
    // literally in BC_DS. They are not stream offsets and carry no FIXUPP.
    assert_eq!(second, first + 1);
    assert!(code[first] == code[second] && code[second] == 0x90);
    assert!(omf::fixups(&records).iter().all(|fixup| fixup.seg != Some(ds_index)));
    let listing = masm::text(&assembled).unwrap();
    assert_eq!(listing.matches("xchg ax, ax").count(), 2);
    assert!(listing.contains("push offset"));
    assert!(listing.contains("call far ptr B$RSTB"));
}

/// Gorillas' inline ATN added PUSH BP, so READ reported Out of stack space at R 0.
#[test]
fn test_inline_module_math_does_not_create_a_native_bp_frame() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(&tmp, "ATNREAD.BAS", b"pi# = atn(1#)\r\ndata 7\r\nread value&\r\n");
    let program = qb45(&source);
    let records = records(&program, "ATNREAD.BAS");
    let code_size = omf::segments(&records)[1].as_ref().unwrap().1;
    let code = omf::segment_image(&records, 1, code_size);

    assert_ne!(&code[48..51], b"\x55\x8b\xec");
    assert!(code.windows(2).any(|pair| pair == b"\xd9\xf3"));
}

fn qb45_listing(name: &str, source: &[u8]) -> String {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(&tmp, name, source);
    listing(&qb45(&source))
}

/// Gorillas stopped in GETNUM because BEEP parsed but had no semantic ABI.
#[test]
fn test_gorillas_beep_reaches_the_audited_zero_argument_runtime_call() {
    let listing = qb45_listing("BEEP.BAS", b"beep\r\n");
    assert!(listing.contains("call far ptr B$BEEP"));
    assert!(!listing.contains("add sp"));
}

/// Gorillas' player-name prompt was misparsed as a two-operand graphics LINE.
#[test]
fn test_gorillas_console_line_input_keeps_prompt_and_destination() {
    let listing = qb45_listing("LNINPUT.BAS", b"dim player as string\r\nline input \"Name: \"; player\r\n");
    assert!(listing.contains("call far ptr B$LNIN"));
    assert!(!listing.contains("call far ptr B$LINE"));
}

/// Gorillas stopped at DO WHILE Char$ = "" by treating undeclared Char$ as numeric.
#[test]
fn test_gorillas_implicit_string_suffix_drives_string_comparison() {
    let listing = qb45_listing("STRLOOP.BAS", b"do while char$ = \"\"\r\nchar$ = inkey$\r\nloop\r\n");
    assert!(listing.contains("call far ptr B$SCMP"));
}

/// Gorillas' score line stopped because PRINT TAB(50) was resolved as an array.
#[test]
fn test_gorillas_print_tab_is_a_control_call_not_an_array() {
    let listing = qb45_listing("PRTAB.BAS", b"print \"score\"; tab(50); 7\r\n");
    assert!(listing.contains("pushw 50"));
    assert!(listing.contains("call far ptr B$FTAB"));
}

/// Gorillas stopped at POINT(x#, y#) because it was resolved as an array.
#[test]
fn test_gorillas_point_is_resolved_from_the_intrinsic_table() {
    let listing =
        qb45_listing("POINT.BAS", b"dim x as double, y as double, pixel as integer\r\npixel = point(x, y)\r\n");
    assert!(listing.contains("call far ptr B$PNR4"));
}

/// Gorillas reached SLEEP 1 with four typed bytes but no audited cleanup.
#[test]
fn test_gorillas_sleep_uses_the_long_runtime_abi() {
    let listing = qb45_listing("SLEEP.BAS", b"sleep 1\r\n");
    assert!(listing.contains("call far ptr B$SLEP"));
}

/// Gorillas' InitVars became a fake procedure requiring QB45's nonexistent B$OEGP.
#[test]
fn test_single_module_gosub_keeps_its_module_error_handler() {
    let listing = qb45_listing(
        "GOSUBERR.BAS",
        b"gosub initvars\r\nend\r\ninitvars:\r\non error goto failed\r\nreturn\r\nfailed:\r\nresume next\r\n",
    );
    let main = between(&listing, "$QB$MAIN proc far", "$QB$MAIN endp");
    assert!(!listing.contains("INITVARS proc far"));
    assert!(main.contains("call far ptr B$OEGA"));
    assert!(!listing.contains("call far ptr B$OEGP"));
}

/// Inlining Gorillas' GOSUB exposed module INTEGER i before a local SINGLE DIM i.
#[test]
fn test_procedure_dim_shadows_implicit_module_variable() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source =
        written(&tmp, "SHADOW.BAS", b"i = 1\r\ncall probe\r\nsub probe\r\ndim i as single\r\ni = 1.5\r\nend sub\r\n");
    let program = qb45(&source);
    assert!(program.modules[0].functions.iter().any(|function| function.name == "PROBE"));
}

const STATIC_LOCAL: &[u8] = b"DECLARE FUNCTION F& ()\r\nDIM total AS LONG\r\ntotal = F&\r\nPRINT total\r\n\
FUNCTION F& STATIC\r\nDIM total AS LONG\r\ntotal = 3\r\nF& = total\r\nEND FUNCTION\r\n";

/// A STATIC FUNCTION's DIM total was refused as a duplicate of the module's total.
#[test]
fn test_static_procedure_dim_shadows_module_variable() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(&tmp, "STATLOC.BAS", STATIC_LOCAL);
    assert!(!object_bytes(&parsed(&source), "STATLOC.BAS").expect("emits").is_empty());
}

/// Nibbles stored x87 status 16384 as arena(3,1).sister, then COLOR failed on 8224.
#[test]
fn test_integer_floor_division_stays_integer_until_its_qb_single_result() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(
        &tmp,
        "floor.bas",
        b"dim row as integer, realRow as integer\r\nrow = 3\r\nrealRow = int((row + 1) / 2)\r\n",
    );
    let program = qb45(&source);
    let projection = hir::mir_text(&lowered(&program)[0]);

    assert!(projection.contains(" divmod 2:2"), "{projection}");
    assert!(!projection.contains("fcompare"), "{projection}");
    assert!(!projection.contains(" add 1:2"), "{projection}");
}

/// Nibbles indexed ARENA(row,col) as row*80+col and passed garbage colors to B$COLR.
#[test]
fn test_hir_lowering_honors_qb_multidimensional_array_order() {
    let tmp = tempfile::TempDir::new().unwrap();
    let basic = written(
        &tmp,
        "ORDER.BAS",
        b"dim shared grid(1 to 2, 1 to 3) as integer\n\
          dim row as integer, col as integer, answer as integer\n\
          answer = grid(row, col)\n",
    );

    let mut factors: BTreeMap<&str, Vec<BigInt>> = BTreeMap::new();
    for order in ["column-major", "row-major"] {
        let program = super::driver::parsed(&basic, "vbdos", "vbdos", None, &[], order, false, false, false, false, false)
            .expect("parses");
        let body = lowered(&program).remove(0).body;
        factors.insert(
            order,
            body.blocks
                .iter()
                .flat_map(|block| &block.ops)
                .filter(|operation| operation.kind == mir::Kind::Mul)
                .flat_map(|operation| &operation.args)
                .filter_map(|argument| match argument {
                    Arg::Const(one) => Some(one.n.clone()),
                    _ => None,
                })
                .collect(),
        );
    }

    assert_eq!(factors["column-major"], [BigInt::from(2), BigInt::from(2)]);
    assert_eq!(factors["row-major"], [BigInt::from(3), BigInt::from(2)]);
}

/// Stage output used to expose Semantics(...), hiding the actual Intel operand order.
#[test]
fn test_machine_stage_dump_is_masm_intel_not_python_repr() {
    let held = |value, width| Loc::Held(ir::Held { value, width });
    let add = ir::Semantics {
        name: Some("add".into()),
        dests: vec![held(3, 4)],
        sources: vec![held(1, 4), held(2, 4)],
        ..ir::Semantics::new(Operation::Binary)
    };
    let insn = lir::Insn::new(11, None, Some(add), vec![3], vec![1, 2]);
    let body = lir::LirBody::new(
        "sum",
        10,
        vec![lir::LirBlock::new(10, vec![Arc::new(insn.clone())])],
        IndexMap::default(),
        IndexMap::default(),
    );

    let dumped = qbstages::_lir(&body, None);

    assert!(dumped.contains("sum proc"), "{dumped}");
    assert!(dumped.contains("add v3, v2"), "{dumped}");
    assert!(!dumped.contains("Semantics("), "{dumped}");

    let call = lir::Insn {
        what: Some(ir::Semantics { name: Some("fsin".into()), ..ir::Semantics::new(Operation::Call) }),
        ..insn
    };
    let mut call_body = body.clone();
    call_body.blocks[0].insns = vec![Arc::new(call)];
    let callees = IndexMap::from_iter([(
        11,
        masm::Callee { code: vec![masm::InlinePart::Bytes(vec![0xd9, 0xfe])], ..masm::Callee::new("$inline_fsin", false) },
    )]);
    let inline = qbstages::_lir(&call_body, Some(&callees));
    assert!(!inline.contains("call $inline_fsin"), "{inline}");
    assert!(inline.contains("db 0d9h,0feh"), "{inline}");
}

/// Nibbles reached HIR, but the showcase crashed while copying byte DB from its source.
#[test]
fn test_qb_stage_dump_reads_the_same_cp437_source_as_the_frontend() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(&tmp, "CP437.BAS", b"print \"\xdb\"\r\n\x1aignored");
    assert_eq!(qbstages::_source_text(&source).unwrap(), "print \"\u{2588}\"\r\n");
}

/// The showcase omitted ENRA and printed encoded RETF 4 as a bare RETF.
#[test]
fn test_qb_stage_dump_ends_with_the_emitted_runtime_abi_assembly() {
    let tmp = tempfile::TempDir::new().unwrap();
    dumped(&fixture("runtime-frame-basic.bas"), tmp.path(), "vbdos", "vbdos");

    let emitted = std::fs::read_to_string(tmp.path().join("99-emitted-asm.asm")).unwrap();
    let report = between(&emitted, "REPORT proc far\n", "REPORT endp");
    // B$ENRA owns BP/SI/DI. The OMF emitter strips the shared backend's native
    // push-bp shell, so the allegedly exact final stage must strip it too.
    assert!(report.starts_with("L1_1:\n    mov     cx, 6\n"), "{report}");
    assert!(!report.contains("push    bp"));
    assert!(emitted.contains("mov     cx, 6"));
    assert!(emitted.contains("call    far ptr B$ENRA"));
    assert!(emitted.contains("call    far ptr B$EXSA"));
    assert!(emitted.contains("retf    4"));
}

/// GorillaIntro's final stage displayed Intro's body because INTRO is its suffix.
#[test]
fn test_qb_stage_dump_replaces_exact_procedure_names() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(
        &tmp,
        "NAMES.BAS",
        b"sub gorillaIntro\r\nprint \"GORILLA\"\r\nend sub\r\nsub intro\r\nprint \"INTRO\"\r\nend sub\r\n",
    );
    let output = tmp.path().join("stages");
    dumped(&source, &output, "qb45", "qb45");

    let text = std::fs::read_to_string(output.join("99-emitted-asm.asm")).unwrap();
    let lines: Vec<&str> = text.lines().collect();

    let procedure = |name: &str| -> String {
        let start = lines.iter().position(|line| *line == format!("{name} proc far")).unwrap();
        let stop = start + 1 + lines[start + 1..].iter().position(|line| *line == format!("{name} endp")).unwrap();
        lines[start..=stop].join("\n")
    };

    assert!(procedure("GORILLAINTRO").contains("NAMES$D3"));
    assert!(procedure("INTRO").contains("NAMES$D4"));
}

/// Nibbles panicked because every INPUT table was mistaken for a VBDOS far literal.
#[test]
fn test_qb45_input_type_table_uses_dgroup_far_pointer() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(&tmp, "INPUT.BAS", b"dim answer as string\ninput \"Number\"; answer\n");
    let program = parsed_as(&source, "vbdos", "qb45");

    let table = program.modules[0].data.iter().find(|one| one.name.starts_with("$input")).unwrap();
    assert_eq!(table.address, model::AddressKind::Near);
    object_bytes(&program, "INPUT.BAS").expect("emits");
}

/// UCA1 printed nothing and never returned because fallthrough called B$CEND.
#[test]
fn test_implicit_module_end_uses_cenp_not_explicit_end_entry() {
    let listing = qb45_listing("IMPLICIT.BAS", b"print \"DONE\"\r\n");
    let main = between(&listing, "$QB$MAIN proc far", "$QB$MAIN endp");

    assert!(main.contains("call far ptr B$CENP"));
    assert!(!main.contains("call far ptr B$CEND"));
}

/// B$ASSN's two stack pointers must not become two simultaneous ES inputs.
///
/// The post-rebase frontend failed this 4096-byte local before frame emission
/// with ``value#12 cannot be placed in fixed 71``: both semantic far-pointer
/// effects had been mistaken for encoded ES operands of the call.
#[test]
fn test_runtime_entry_reserves_the_complete_live_local_extent() {
    let source = parsed(&fixture("runtime-frame-stack.bas"));
    let listing = listing(&source);
    let procedure = between(&listing, "REPORT proc far", "REPORT endp");

    assert!(procedure.contains("mov cx, 4096"), "{procedure}");
    assert!(procedure.contains("call far ptr B$ENRA"));
    assert!(procedure.contains("lea ax, [bp-4116]"), "{procedure}");
    assert!(procedure.contains("call far ptr B$EXSA"));
}

/// CALL probe printed OK for -42 because rewriting RETURN erased every block successor.
#[test]
fn test_module_exit_rewrite_preserves_conditional_false_edges() {
    let source = parsed(&fixture("runtime-call-basic.bas"));
    let listing = listing(&source);
    let main = between(&listing, "$QB$MAIN proc far", "$QB$MAIN endp");

    // The shared layout may encode the false edge as the immediately
    // following block or as an explicit jump.  Either spelling must retain
    // the edge and both arms must converge on the runtime exit.
    assert!(
        main.contains("cmp eax, 42\n    je L0_2\nL0_3:") || main.contains("cmp eax, 42\n    je L0_2\n    jmp L0_3"),
        "{main}"
    );
    assert_eq!(main.matches("call far ptr B$PESD").count(), 2);
    assert!(main.contains("L0_4:\n    call far ptr B$CENP"), "{main}");
    assert!(main.contains("L0_2:") && main.contains("jmp L0_4"), "{main}");
}

/// SUBTRACTPAIR read 8-50: calls pushed left-to-right but formals used ascending offsets.
#[test]
fn test_pascal_formals_are_read_in_reverse_physical_stack_order() {
    let source = parsed(&fixture("runtime-call-basic.bas"));
    let listing = listing(&source);
    let function = between(&listing, "SUBTRACTPAIR proc far", "SUBTRACTPAIR endp");

    assert!(function.contains("mov eax, dword ptr [bp+10]"), "{function}");
    assert!(function.contains("sub eax, dword ptr [bp+6]"), "{function}");
}

fn vbdos_calls(name: &str, types: Vec<model::Type>, function: model::Function) -> model::Program {
    model::Program::new(
        model::Dialect::Vbdos,
        model::RuntimeProfile::Vbdos,
        vec![model::Module::new(1, name, types, vec![function])],
    )
}

fn call_function(name: &str, places: Vec<model::Place>, instruction: model::Instruction, order: Vec<i64>) -> model::Function {
    let block =
        model::Block::new(1, vec![instruction], model::Terminator::new(model::TerminatorKind::Return, vec![], vec![]));
    let call = model::CallAbi {
        instruction: 1,
        order,
        cleanup: model::StackCleanup::Callee,
        distance: model::CallDistance::Far,
        callee: None,
        float_return: model::FloatReturn::Pointer,
    };
    model::Function { calls: vec![call], ..model::Function::new(1, name, 0, vec![], places, vec![block], 1) }
}

#[test]
fn test_qb_call_abi_is_materialized_only_after_semantic_mir() {
    let void = model::Type::new(0, "void", model::TypeKind::Void, 0);
    let integer = model::Type { signed: Some(true), ..model::Type::new(1, "integer", model::TypeKind::Integer, 2) };
    let instruction = model::Instruction {
        callee: Some("draw".into()),
        ..model::Instruction::new(
            1,
            model::Op::Call,
            vec![],
            vec![model::Operand::constant(1, 10), model::Operand::constant(1, 20)],
        )
    };
    let function = call_function("caller", vec![], instruction, vec![1, 0]);
    let source = vbdos_calls("calls", vec![void, integer], function.clone());
    let semantic = lowered(&source).remove(0);
    assert_eq!(semantic.body.blocks[0].ops[0].args, [Arg::Const(mir::Const::new(10, 2)), Arg::Const(mir::Const::new(20, 2))]);
    let physical = physicalize(&source, &function, &semantic).expect("physicalizes");
    let operations = &physical.lowered.body.blocks[0].ops;
    assert_eq!(
        operations.iter().map(|one| one.kind).collect::<Vec<_>>(),
        [mir::Kind::Arg, mir::Kind::Arg, mir::Kind::Call, mir::Kind::Return]
    );
    assert_eq!(operations[0].args, [Arg::Const(mir::Const::new(20, 2))]);
    assert_eq!(operations[1].args, [Arg::Const(mir::Const::new(10, 2))]);
    assert!(operations[2].args.is_empty());
    assert_eq!(physical.contracts[&operations[2].at].cleanup, Some(4));
    assert!(physical.far_calls.contains(&operations[2].at));
    let machine =
        machine(&physical.lowered.name, &physical.lowered.body, &physical.calls, &physical.contracts, None);
    assert_eq!(
        machine.insns().iter().filter_map(|one| one.what.as_ref().map(|what| what.op)).take(3).collect::<Vec<_>>(),
        [Operation::Push, Operation::Push, Operation::Call]
    );
}

/// STR$(single) failed before LIR because a Cell keeps width on its MemRef.
#[test]
fn test_qb_memory_argument_uses_its_reference_width() {
    let void = model::Type::new(0, "void", model::TypeKind::Void, 0);
    let single = model::Type {
        evaluation: model::FloatEvaluation::Extended80,
        ..model::Type::new(1, "single", model::TypeKind::Float, 4)
    };
    let argument = model::Place { extent: Some(4), ..model::Place::new(1, "$str4", 1, model::Storage::Local, -4) };
    let instruction = model::Instruction {
        callee: Some("B$STR4".into()),
        ..model::Instruction::new(1, model::Op::Call, vec![], vec![model::Operand::place_ref(1)])
    };
    let function = call_function("str_single", vec![argument], instruction, vec![0]);
    let source = vbdos_calls("strings", vec![void, single], function.clone());
    let physical = physicalize(&source, &function, &lowered(&source)[0]).expect("physicalizes");
    let operations = &physical.lowered.body.blocks[0].ops;
    let Arg::Cell(cell) = &operations[0].args[0] else { panic!("{:?} is not a Cell", operations[0].args[0]) };
    assert_eq!(cell.r#ref.width, 4);
    assert_eq!(physical.contracts[&operations[1].at].cleanup, Some(4));
}

/// common.bas's whole-pointer string element failed HIR comparison verification.
#[test]
fn test_dynamic_string_array_near_offset_reaches_physical_mir() {
    let source = parsed(&fixture("dynamic_strings.bas"));
    let function = &source.modules[0].functions[0];
    let semantic = lowered(&source).remove(0);
    let text = hir::mir_text(&semantic);
    assert!(hir::encode(&source, None).unwrap().contains("\"op\":\"ptr_offset\""));
    assert!(text.contains("call B$SCMP("), "{text}");
    let physical = physicalize(&source, function, &semantic).expect("physicalizes");
    let machine = machine(
        &physical.lowered.name,
        &physical.lowered.body,
        &physical.calls,
        &physical.contracts,
        Some(physical.pointer_model.clone()),
    );
    assert!(!machine.insns().is_empty());
}

/// LS_SELFTEST used values from the true arm after EXIT FUNCTION returned.
#[test]
fn test_single_line_if_exit_does_not_connect_its_unreachable_continuation() {
    let source = parsed(&fixture("early-exit.bas"));
    let classify = source
        .modules
        .iter()
        .flat_map(|module| &module.functions)
        .find(|function| function.name.to_uppercase() == "CLASSIFY")
        .unwrap();
    assert!(classify.blocks.iter().all(|block| block.terminator.kind != model::TerminatorKind::Unreachable));
    assert!(lowered(&source).iter().all(|one| mir::verify(&one.body).is_empty()));
}

/// The source frontend formerly stopped at allocated LIR and could not link anything.
#[test]
fn test_qb_numeric_procedure_emits_a_fresh_far_pascal_object() {
    let source = parsed(&fixture("emission.bas"));
    let records = records(&source, "emission.bas");
    let public = omf::public_definitions(&records).unwrap();
    assert!(public.contains_key("ADDONE"));
    let segments = omf::segments(&records);
    let code = omf::segment_image(&records, 1, segments[1].as_ref().unwrap().1);
    assert_eq!(&code[..10], b"blEMISSION");
    let (bc_sa, _) = segment(&records, "BC_SA");
    assert!(omf::fixups(&records).iter().any(|fixup| fixup.seg == Some(bc_sa)
        && fixup.offset == 0
        && fixup.loc == 3
        && fixup.target == "segment"
        && fixup.index == 1));
    let (segment, offset) = public["ADDONE"];
    let image = omf::segment_image(&records, segment, segments[segment as usize].as_ref().unwrap().1);
    let procedure = &image[offset as usize..];
    let statement_table = procedure[3..].windows(3).position(|bytes| bytes == [0x55, 0x8b, 0xec]).map(|at| at + 3);
    assert!(statement_table.is_some_and(|at| at > 0));
    assert!(procedure[..statement_table.unwrap()].ends_with(&[0xca, 0x04, 0x00]));
}

/// Cross-module calls must use BC's uppercase, unsuffixed Pascal symbols.
#[test]
fn test_source_procedure_names_match_all_three_microsoft_omf_dialects() {
    let expected = set(&["REPORT", "TWICE"]);
    for fixture in ["procs-q-O-zi.obj", "procs-p-ot.obj", "procs-v-g3-zi.obj"] {
        let read = omf::read(root().join("fixtures/omf").join(fixture.to_lowercase())).unwrap();
        assert_eq!(omf::public_definitions(&read).unwrap().keys().cloned().collect::<BTreeSet<_>>(), expected);
        assert!(expected.is_subset(&omf::externals(&read).into_iter().collect()));
    }

    let source = parsed(&fixture("interop.bas"));
    let emitted = records(&source, "interop.bas");
    assert_eq!(omf::public_definitions(&emitted).unwrap().keys().cloned().collect::<BTreeSet<_>>(), expected);

    let caller = parsed(&fixture("interop-external.bas"));
    let emitted = records(&caller, "interop-external.bas");
    assert!(expected.is_subset(&omf::externals(&emitted).into_iter().collect()));
}

/// Gorillas exported FNRAN even though BC keeps its DEF FN label private.
#[test]
fn test_def_fn_keeps_bcs_private_symbol_scope() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(
        &tmp,
        "SYMBOLS.BAS",
        b"def fnPrivate(value) = value + 1\r\nfunction Public(value)\r\npublic = fnPrivate(value)\r\nend function\r\n",
    );
    let program = qb45(&source);
    let functions: IndexMap<&str, &model::Function> =
        program.modules[0].functions.iter().map(|function| (function.name.as_str(), function)).collect();
    assert_eq!(functions["FNPRIVATE"].linkage, model::FunctionLinkage::Internal);
    assert_eq!(functions["PUBLIC"].linkage, model::FunctionLinkage::External);

    let records = records(&program, "SYMBOLS.BAS");
    assert_eq!(omf::public_definitions(&records).unwrap().keys().cloned().collect::<BTreeSet<_>>(), set(&["PUBLIC"]));
}

const SYMNAM: &[u8] = b"dim shared implicit\r\n\
dim shared explicitInteger as integer\r\n\
dim shared explicitLong as long\r\n\
dim shared explicitSingle as single\r\n\
dim shared explicitDouble as double\r\n\
dim shared explicitString as string * 8\r\n\
dim shared implicitArray(1 to 2)\r\n\
implicit = 1\r\n";

/// QB45 /Zi calls implicit `implicit` IMPLICIT!, not an anonymous data offset.
#[test]
fn test_module_globals_keep_bcs_effective_type_suffixes() {
    let listing = qb45_listing("SYMNAM.BAS", SYMNAM);
    for name in [
        "IMPLICIT!",
        "EXPLICITINTEGER%",
        "EXPLICITLONG&",
        "EXPLICITSINGLE!",
        "EXPLICITDOUBLE#",
        "EXPLICITSTRING$",
        "IMPLICITARRAY!",
    ] {
        assert!(listing.contains(&format!("{name} label byte")), "{name}");
    }
    assert!(listing.contains("mov dword ptr IMPLICIT!, 1065353216"));
}

/// The readable stage must retain QB's named zero bytes, not anonymous DB runs.
#[test]
fn test_qb_stage_assembly_compacts_typed_zero_globals() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(
        &tmp,
        "SYMNAM.BAS",
        b"dim shared implicit\r\n\
          dim shared explicitInteger as integer\r\n\
          dim shared explicitLong as long\r\n\
          dim shared explicitDouble as double\r\n\
          dim shared implicitArray(1 to 2)\r\n\
          implicit = 1\r\n",
    );
    let output: PathBuf = tmp.path().join("stages");
    dumped(&source, &output, "qb45", "qb45");

    let readable = std::fs::read_to_string(output.join("99-emitted-asm.asm")).unwrap();
    let raw = std::fs::read_to_string(output.join("99-emitted-asm.raw.asm")).unwrap();

    assert!(readable.contains("; QB source globals: BC-compatible effective names"));
    assert!(readable.contains("IMPLICIT!            dd 0"));
    assert!(readable.contains("EXPLICITINTEGER%     dw 0"));
    assert!(readable.contains("EXPLICITLONG&        dd 0"));
    assert!(readable.contains("EXPLICITDOUBLE#      dq 0"));
    assert!(readable.contains("IMPLICITARRAY!       dq 0"));
    assert!(!readable.contains("\n    db 000h,000h,000h,000h")); // zero globals use typed declarations
    assert!(!readable.contains('\t'));
    assert!(raw.contains("IMPLICIT! label byte\ndb 000h,000h,000h,000h"));
    assert!(raw.contains("EXPLICITDOUBLE# label byte\ndb 000h,000h,000h,000h,000h,000h,000h,000h"));
}

/// A display-only fall-through label obscured code; a jump target must stay visible.
#[test]
fn test_qb_stage_assembly_aligns_code_and_hides_only_unreferenced_labels() {
    let displayed = qbstages::_display_assembly(
        "ONE proc far\n\
         L1_1:\n    mov ax, 1\n\
         L1_2:\n    add ax, 2\n    jne L1_4\n\
         L1_3:\n    retf\n\
         L1_4:\n    retf\n\
         ONE endp\n",
    );

    assert!(displayed.contains("; Procedure: ONE"));
    assert!(displayed.contains("L1_1:")); // procedure entry is an externally useful anchor
    assert!(!displayed.contains("L1_2:"));
    assert!(!displayed.contains("L1_3:"));
    assert!(displayed.contains("L1_4:")); // `jne` has a machine-code reference
    assert!(displayed.contains("    mov     ax, 1"));
    assert!(displayed.contains("    jne     L1_4"));
    assert!(!displayed.contains('\t'));
}

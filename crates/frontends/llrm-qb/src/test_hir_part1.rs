//! `tests/test_hir.py` QB cases, part 1; helpers in `test_hir`.
//!
//! (test_qb_driver_never_replays_a_stale_in_tree_release_binary has no
//! subject now: qbfront is linked in.)

use std::collections::BTreeSet;

use llrm_core::support::hash::IndexMap;

use super::cli::parse_args;
use super::qbstages;
use super::test_hir::*;
use llrm_core::hir::{self, model};
use llrm_core::objectfile::omf;

fn argv(items: &[&str]) -> Vec<String> {
    items.iter().map(|one| (*one).to_owned()).collect()
}

fn qb45(source: &std::path::Path) -> model::Program {
    parsed_as(source, "qb45", "qb45")
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

#[test]
fn test_qb_cli_reads_the_machine_it_is_given() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("far.toml");
    std::fs::write(&path, llrm_core::abi::machine::DOS.replace("far_bss = false", "far_bss = true")).expect("writes");
    let args = parse_args(&argv(&["probe.bas", "--machine", path.to_str().expect("utf-8")])).expect("parses");
    assert!(args.codegen.machine.far_bss);
    assert_eq!(parse_args(&argv(&["probe.bas"])).expect("parses").codegen.machine, *llrm_core::abi::machine::BASIC);
}

/// BASIC's runtime runs compiled code on the program's stack, in the data
/// group, unless -mno-stack-is-data says otherwise.
#[test]
fn test_qb_cli_keeps_the_stack_in_the_data_group_unless_told_not_to() {
    let stack_is_data = |arguments: &[&str]| parse_args(&argv(arguments)).expect("parses").codegen.machine.segments.unwrap().stack_is_data;
    assert!(stack_is_data(&["probe.bas"]));
    assert!(!stack_is_data(&["probe.bas", "-mno-stack-is-data"]));
}

/// Q45N01 stopped at READ, then a native-only spill frame made READ report syntax error.
#[test]
#[ignore = "the Python original fails too: q45n01 now has no spill, so no B$ENRA/B$EXSA"]
fn test_qb45_numeric_read_data_reaches_typed_hir_and_fresh_omf() {
    let source = root().join("tests/differential/conformance/qb45/q45n01.bas");
    let program = qb45(&source);
    let main = &program.modules[0].functions[0];
    let calls = hir_calls(main);

    assert_eq!(calls[..7], ["B$RDI2", "B$RDI2", "B$RDI4", "B$RDI4", "B$RDI4", "B$RDR4", "B$RDR4"]);
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
    let listing = listing(&qb45(&source));

    // A row is keyed by its position; B$RSTB is passed the labeled row's key.
    let rows = between(&listing, "$QB$DS label byte\n", "BC_DS ends");
    assert_eq!(rows, "db 000h,000h\ndb 020h,031h,000h\ndb 001h,000h\ndb 020h,032h,000h\ndb 0ffh,0ffh,001h\n");
    assert!(listing.contains("pushw 1\n    call far ptr B$RSTB"), "{listing}");
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
    // The division can raise, so the handler must be registered.
    let listing = qb45_listing(
        "GOSUBERR.BAS",
        b"gosub initvars\r\nprint 1 / x%\r\nend\r\ninitvars:\r\non error goto failed\r\nreturn\r\nfailed:\r\nprint \"f\"\r\nresume next\r\n",
    );
    let main = between(&listing, "$QB$MAIN proc far", "$QB$MAIN endp");
    assert!(!listing.contains("INITVARS proc far"));
    assert!(main.contains("call far ptr B$OEGA"), "{main}");
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
    let mir = emitted_mir(&program);

    assert!(mir.contains("sdiv i16"), "{mir}");
    assert!(!mir.contains("fcmp"), "{mir}");
    assert!(!mir.contains("fdiv"), "{mir}");
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

    let multiplier = |order: &str| {
        let frontend = super::driver::Frontend { array_order: order.into(), ..super::driver::Frontend::new("vbdos", "vbdos") };
        let mir = emitted_mir(&super::driver::parsed(&basic, &frontend, None).expect("parses"));
        let found: Vec<&str> = mir.lines().filter(|line| line.contains(" = mul i16 ")).map(|line| line.rsplit(", ").next().unwrap()).collect();
        found.join(" ")
    };

    assert_eq!(multiplier("column-major"), "2");
    assert_eq!(multiplier("row-major"), "3");
}

/// Nibbles reached HIR, but the showcase crashed while copying byte DB from its source.
#[test]
fn test_qb_stage_dump_reads_the_same_cp437_source_as_the_frontend() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(&tmp, "CP437.BAS", b"print \"\xdb\"\r\n\x1aignored");
    assert_eq!(qbstages::_source_text(&source).unwrap(), "print \"\u{2588}\"\r\n");
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
    let source = parsed_runtime_frames(&fixture("runtime-frame-stack.bas"));
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
    // A condition the optimizer cannot decide: both arms must survive and
    // converge on the runtime exit.
    let tmp = tempfile::TempDir::new().unwrap();
    let source = written(
        &tmp,
        "EDGE.BAS",
        b"dim result as long\r\nresult = val(command$)\r\nif result = 42 then\r\n    print \"CALL OK\"\r\nelse\r\n    print \"CALL BAD\"\r\nend if\r\n",
    );
    let listing = listing(&parsed(&source));
    let main = between(&listing, "$QB$MAIN proc far", "$QB$MAIN endp");

    assert!(main.contains("cmp eax, 42\n    je L"), "{main}");
    assert_eq!(main.matches("call far ptr B$PESD").count(), 2, "{main}");
    assert_eq!(main.matches("call far ptr B$CENP").count(), 1, "{main}");
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

/// common.bas's whole-pointer string element failed HIR comparison verification.
#[test]
fn test_dynamic_string_array_near_offset_reaches_its_comparison() {
    let source = parsed(&fixture("dynamic_strings.bas"));
    assert!(hir::encode(&source, None).unwrap().contains("\"op\":\"ptr_offset\""));
    let listing = listing(&source);
    assert!(listing.contains("call far ptr B$SCMP"), "{listing}");
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
    assert!(assembled(&source).is_ok());
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
        let read = omf::read(root().join("tests/inputs/omf").join(fixture.to_lowercase())).unwrap();
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

/// Each global written from input and read by a SUB, so each stays in memory.
const SYMNAM: &[u8] = b"declare sub show ()\r\n\
dim shared implicit\r\n\
dim shared explicitInteger as integer\r\n\
dim shared explicitLong as long\r\n\
dim shared explicitSingle as single\r\n\
dim shared explicitDouble as double\r\n\
dim shared explicitString as string * 8\r\n\
dim shared implicitArray(1 to 2)\r\n\
implicit = 1\r\n\
explicitInteger = val(command$)\r\nexplicitLong = val(command$)\r\nexplicitSingle = val(command$)\r\n\
explicitDouble = val(command$)\r\nexplicitString = command$\r\nimplicitArray(1) = val(command$)\r\nshow\r\n\
sub show\r\nprint implicit; explicitInteger; explicitLong; explicitSingle; explicitDouble; explicitString; implicitArray(1)\r\nend sub\r\n";

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

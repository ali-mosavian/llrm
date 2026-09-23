//! `tests/test_hir.py` QB cases, part 3; helpers in `test_hir`.
//!
//! Also `tests/test_qbstages.py::test_stage_observer_uses_one_compilation_and_preserves_object_bytes`
//! and `tests/test_qb_frontend_command.py::test_common_hir_profiles_do_not_become_qb_frontend_options`.
//!
// skipped: test_an_oversized_exact_loop_is_never_cloned_as_a_peel_candidate: monkeypatches loopclone.peeled
// skipped: test_pytest_frontend_setup_builds_once_and_configures_producer: monkeypatches conftest and build_release
// skipped: test_pytest_frontend_setup_preserves_an_explicit_producer: monkeypatches conftest and build_release
// skipped: test_build_release_uses_cargos_qbfront_artifact: monkeypatches subprocess.run
// skipped: test_build_release_rejects_invalid_cargo_report: monkeypatches subprocess.run
// skipped: test_build_release_rejects_cargo_report_without_qbfront: monkeypatches subprocess.run
// skipped: test_build_release_reports_cargo_start_failure: monkeypatches subprocess.run

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use crate::support::hash::IndexMap;

use super::abi::{_contract, physicalize};
use super::compile::{self as qb_compile, Stage, StageObserver, StageValue};
use super::driver as qb_driver;
use super::inline_x87::finalized;
use super::test_hir::*;
use crate::abi::runtime::Contract;
use crate::analysis::loops;
use crate::backend::pointers;
use crate::backend::{floatalloc, frame, lower as lower_mir, masm};
use crate::hir::model::{self as hir, Operand};
use crate::hir::{Lowered, lower, mir_text};
use crate::model::ir::{self, Loc, Operation};
use crate::model::lir;
use crate::model::mir::{self, Arg, Kind};
use crate::model::passes::O2;
use crate::objectfile::module::Space;
use crate::objectfile::omf;

// ---- small helpers -------------------------------------------------------

fn compat(path: &str) -> std::path::PathBuf {
    root().join("frontends/qb/compat").join(path)
}

/// `qb_driver.parsed(source, dialect=..., runtime=..., array_order=..., huge_arrays=..., unchecked_bounds=...)`.
fn parsed_with(source: &Path, dialect: &str, runtime: &str, array_order: &str, huge: bool, unchecked: bool) -> hir::Program {
    qb_driver::parsed(source, dialect, runtime, None, &[], array_order, huge, false, unchecked, false, false)
        .unwrap_or_else(|error| panic!("{}: {error}", source.display()))
}

/// Each listing line without its comment, whitespace collapsed.
fn stripped_lines(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(|line| line.split(';').next().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

fn externals(program: &hir::Program, name: &str) -> Vec<String> {
    omf::externals(&records(program, name))
}

fn ops(body: &mir::MirBody) -> Vec<&mir::Op> {
    body.blocks.iter().flat_map(|block| &block.ops).collect()
}

fn function_named<'p>(program: &'p hir::Program, name: &str) -> (usize, &'p hir::Function) {
    program.modules[0].functions.iter().enumerate().find(|(_, one)| one.name == name).expect("the function")
}

fn semantic(program: &hir::Program, index: usize) -> Lowered {
    lower(program).expect("lowers").remove(index)
}

fn optimized(program: &hir::Program, function: &hir::Function, body: &Lowered) -> Lowered {
    qb_compile::optimized(program, function, body, &O2()).expect("optimizes")
}

fn physical(program: &hir::Program, function: &hir::Function, body: &Lowered) -> super::abi::Physicalized {
    physicalize(program, function, body).expect("physicalizes")
}

/// `lower_mir.lowered(name, body, calls, set(), contracts, occurrences={}, pointer_model=...)`.
fn machine(
    name: &str,
    body: &mir::MirBody,
    calls: &IndexMap<i64, String>,
    contracts: &IndexMap<i64, Contract>,
    pointer_model: Option<pointers::Model>,
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

/// `floatalloc.allocated(machine, frame.of(machine, calls))`.
fn float_allocated(body: &lir::LirBody, calls: &IndexMap<i64, String>) -> lir::LirBody {
    let mut owned = frame::of(body, Some(calls), "", None).expect("frames");
    floatalloc::allocated(body, Some(&mut owned), true, "386").expect("allocates")
}

fn loc_width(one: &Loc) -> u32 {
    match one {
        Loc::Reg(one) => one.width,
        Loc::Mem(one) => one.width,
        Loc::Imm(one) => one.width,
        Loc::Held(one) => one.width,
        other => panic!("no width: {other:?}"),
    }
}

fn int_type(id: i64, name: &str, width: i64) -> hir::Type {
    hir::Type { signed: Some(true), ..hir::Type::new(id, name, hir::TypeKind::Integer, width) }
}

fn float_type(id: i64, name: &str, width: i64) -> hir::Type {
    hir::Type { evaluation: hir::FloatEvaluation::Extended80, ..hir::Type::new(id, name, hir::TypeKind::Float, width) }
}

fn returning(id: i64, instructions: Vec<hir::Instruction>) -> hir::Block {
    hir::Block::new(id, instructions, hir::Terminator::new(hir::TerminatorKind::Return, vec![], vec![]))
}

fn vbdos(module: hir::Module) -> hir::Program {
    hir::Program::new(hir::Dialect::Vbdos, hir::RuntimeProfile::Vbdos, vec![module])
}

fn stack_call(operands: Vec<Operand>, callee: &str, order: Vec<i64>) -> (hir::Instruction, hir::CallAbi) {
    let instruction =
        hir::Instruction { callee: Some(callee.to_owned()), ..hir::Instruction::new(1, hir::Op::Call, vec![], operands) };
    let call = hir::CallAbi {
        instruction: 1,
        order,
        cleanup: hir::StackCleanup::Callee,
        distance: hir::CallDistance::Far,
        callee: None,
    };
    (instruction, call)
}

fn is_word(one: char) -> bool {
    one.is_alphanumeric() || one == '_'
}

fn all_word(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_word)
}

/// `re.search(r"v\d+, v\d+ <- call B\$HARY\(v\d+:2\)", text)`.
fn has_hary_pair(text: &str) -> bool {
    let needle = " <- call B$HARY(v";
    text.match_indices(needle).any(|(at, _)| {
        let after = &text[at + needle.len()..];
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        let after_ok = digits > 0 && after[digits..].starts_with(":2)");
        let before = text[..at].trim_end_matches(|one: char| one.is_ascii_digit());
        let before_ok = before.len() < at
            && before.strip_suffix('v').and_then(|rest| rest.strip_suffix(", ")).is_some_and(|rest| {
                let trimmed = rest.trim_end_matches(|one: char| one.is_ascii_digit());
                trimmed.len() < rest.len() && trimmed.ends_with('v')
            });
        after_ok && before_ok
    })
}

/// `re.findall(r"mov word ptr SUM_THREE\$D\d+,", text)` count.
fn static_stores(text: &str) -> usize {
    let needle = "mov word ptr SUM_THREE$D";
    text.match_indices(needle)
        .filter(|(at, _)| {
            let rest = &text[at + needle.len()..];
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            digits > 0 && rest[digits..].starts_with(',')
        })
        .count()
}

/// `re.search(r"word ptr \[\w+\+\w+\+<disp>\]", text)`.
fn has_indexed_field(text: &str, disp: &str) -> bool {
    let needle = "word ptr [";
    text.match_indices(needle).any(|(at, _)| {
        let Some((inside, _)) = text[at + needle.len()..].split_once(']') else { return false };
        let parts: Vec<&str> = inside.split('+').collect();
        parts.len() == 3 && all_word(parts[0]) && all_word(parts[1]) && parts[2] == disp
    })
}

/// `re.finditer(r"\bj\w+ (\w+)\n", text)` when `newline`, else
/// `re.findall(r"\bj(?!mp)\w+ (\w+)$", text, re.MULTILINE)`: `(start, end, label)`.
fn jumps(text: &str, newline: bool) -> Vec<(usize, usize, String)> {
    let mut found = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let ended = line.ends_with('\n');
        let body = line.strip_suffix('\n').unwrap_or(line);
        if !newline || ended {
            let characters: Vec<(usize, char)> = body.char_indices().collect();
            for (index, (at, one)) in characters.iter().enumerate() {
                if *one != 'j' || (index > 0 && is_word(characters[index - 1].1)) {
                    continue;
                }
                let rest = &body[at + 1..];
                if !newline && rest.starts_with("mp") {
                    continue;
                }
                if let Some((mnemonic, label)) = rest.split_once(' ') {
                    if all_word(mnemonic) && all_word(label) {
                        found.push((offset + at, offset + body.len() + usize::from(newline), label.to_owned()));
                        break;
                    }
                }
            }
        }
        offset += line.len();
    }
    found
}

/// `re.findall(r"^(\w+):$", text, re.MULTILINE)`.
/// Labels of `re.findall(r"^(\w+):\n((?:    .*\n)*)", text, re.MULTILINE)` blocks calling B$...BND.
fn bound_call_labels(text: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut found = BTreeSet::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(name) = line.strip_suffix(":\n").filter(|name| all_word(name)) else { continue };
        let block: String =
            lines[at + 1..].iter().take_while(|one| one.starts_with("    ") && one.ends_with('\n')).copied().collect();
        if block.contains("B$") && block.contains("BND") {
            found.insert(name.to_owned());
        }
    }
    found
}

fn sum_three(unchecked: bool) -> String {
    let source = root().join("bench/parity/sum_three.bas");
    let program = parsed_with(&source, "vbdos", "vbdos", "column-major", false, unchecked);
    let text = listing(&program);
    let start = text.find("SUMTHREE proc").expect("SUMTHREE proc");
    let end = text.find("SUMTHREE endp").expect("SUMTHREE endp");
    text[start..end].to_owned()
}

/// `_backward_loop`: from the first label a later jump returns to, through that jump.
fn backward_loop(procedure: &str) -> String {
    for (start, end, label) in jumps(procedure, true) {
        if let Some(at) = procedure.find(&format!("{label}:\n")) {
            if at < start {
                return procedure[at..end].to_owned();
            }
        }
    }
    panic!("no loop")
}

/// `_function`.
fn function_in(directory: &tempfile::TempDir, name: &str, text: &str, wanted: &str, dialect: &str) -> (hir::Program, hir::Function) {
    let source = written(directory, name, text.as_bytes());
    let program = parsed_as(&source, dialect, dialect);
    let function = program.modules[0].functions.iter().find(|one| one.name == wanted).expect("the function").clone();
    (program, function)
}

/// `_value_exit`.
fn value_exit(function: &hir::Function) -> &hir::Block {
    function
        .blocks
        .iter()
        .find(|block| block.terminator.kind == hir::TerminatorKind::Return && !block.terminator.operands.is_empty())
        .expect("a value exit")
}

fn callees(block: &hir::Block) -> Vec<Option<&str>> {
    block.instructions.iter().map(|one| one.callee.as_deref()).collect()
}

fn position(calls: &[Option<&str>], name: &str) -> usize {
    calls.iter().position(|one| *one == Some(name)).unwrap_or_else(|| panic!("{name} not called"))
}

// ---- tests ---------------------------------------------------------------

/// QGL HOST_RENDER's three `delta ^ 2` terms left duplicate x87 values live.
#[test]
fn test_inline_square_leaves_no_float_live_out() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SQUARE.BAS", b"dim x as single, answer as single\r\nanswer = x ^ 2\r\n");
    let source = parsed_as(&basic, "vbdos", "vbdos");
    assert!(!object_bytes(&source, "SQUARE.BAS").expect("emits").is_empty());
}

/// SYS_MEM_MARK stopped before HIR because FRE("") was sent through numeric lowering.
#[test]
fn test_string_fre_emits_the_measured_vbdos_runtime_call() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "FRESTR.BAS", b"dim available as long\r\navailable = fre(\"\")\r\n");
    let source = parsed_as(&basic, "vbdos", "vbdos");
    assert!(externals(&source, "FRESTR.BAS").iter().any(|one| one == "B$FRSD"));
}

/// D_SURF SC_FTAKE lost far-array address definitions during secondary folding.
#[test]
fn test_dynamic_array_walk_keeps_far_pointer_halves_defined() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "FARWALK.BAS",
        concat!(
            "option explicit\r\n",
            "dim shared head() as integer\r\n",
            "dim shared link() as integer\r\n",
            "dim shared group() as integer\r\n",
            "function take (byval order as integer, byval wanted as integer) as integer\r\n",
            "dim block as integer, previous as integer\r\n",
            "block = head(order)\r\n",
            "previous = -1\r\n",
            "while block >= 0\r\n",
            "if group(block) = wanted then\r\n",
            "if previous >= 0 then link(previous) = link(block) else head(order) = link(block)\r\n",
            "take = block\r\n",
            "exit function\r\n",
            "end if\r\n",
            "previous = block\r\n",
            "block = link(block)\r\n",
            "wend\r\n",
            "take = -1\r\n",
            "end function\r\n",
        )
        .as_bytes(),
    );
    let source = parsed_with(&basic, "vbdos", "vbdos", "row-major", false, false);
    assert!(!object_bytes(&source, "FARWALK.BAS").expect("emits").is_empty());
}

/// SCREEN stopped at ABI lowering although VBDOS B$DSG0 is a zero-argument RETF.
#[test]
fn test_bare_def_seg_reaches_object_emission() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "DEFSEG.BAS", b"def seg = 40960\r\npoke 12, 34\r\ndef seg\r\n");
    let source = parsed_as(&basic, "vbdos", "vbdos");
    let names = externals(&source, "DEFSEG.BAS");
    assert!(names.iter().any(|one| one == "B$DSG0"));
    assert!(!names.iter().any(|one| one == "B$POKE"));
}

/// SCN9 called B$CSCN without B$EGAUSED, so LINK omitted EGA and SCREEN 9 raised error 5.
#[test]
fn test_constant_screen_mode_pulls_its_graphics_driver() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SCN9.BAS", b"screen 9\r\nscreen 0\r\n");
    let source = parsed(&basic);
    assert!(externals(&source, "SCN9.BAS").iter().any(|one| one == "B$EGAUSED"));
}

/// Gorillas SCREEN Mode linked no graphics modules and failed before drawing its first frame.
#[test]
fn test_variable_screen_mode_pulls_all_graphics_drivers() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(&directory, "SCNVAR.BAS", b"dim mode as integer\r\nmode = 9\r\nscreen mode\r\n");
    let source = parsed(&basic);
    assert!(externals(&source, "SCNVAR.BAS").iter().any(|one| one == "B$GRPUSED"));
}

/// Gorillas emitted IDIV AX twice for 30 \\ (80 \\ MaxCol), faulting on its first shot.
/// Once the frontend folded 80 to a LONG constant, lowering dropped it: `idiv eax`.
#[test]
fn test_nested_integer_division_keeps_each_dividend() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "NESTDIV.BAS",
        concat!(
            "defint a-z\r\n",
            "declare function scale (maxCol)\r\n",
            "print scale(80)\r\n",
            "end\r\n",
            "function scale (maxCol)\r\n",
            "scale = 30 \\ (80 \\ maxCol)\r\n",
            "end function\r\n",
        )
        .as_bytes(),
    );
    let source = parsed_as(&basic, "qb45", "qb45");
    let assembly = listing(&source);
    let scale = between(&assembly, "SCALE proc far\n", "SCALE endp\n");

    assert!(scale.contains("mov eax, 80\n"));
    assert!(scale.contains("mov eax, 30\n"));
    assert_eq!(scale.matches("idiv e").count(), 2);
    assert!(!scale.contains("idiv ax"));
}

/// ENT_MOVE_TRIGS passed a four-byte far field address to a two-byte scalar formal.
#[test]
fn test_byref_dynamic_array_field_copies_through_a_near_formal() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "FARFIELD.BAS",
        concat!(
            "option explicit\r\n",
            "type Item\r\npad as integer\r\nvalue as integer\r\nend type\r\n",
            "declare sub consume (number as integer)\r\n",
            "dim shared items() as Item\r\n",
            "sub invoke (byval index as integer)\r\n",
            "consume items(index).value\r\n",
            "end sub\r\n",
        )
        .as_bytes(),
    );
    let source = parsed_with(&basic, "vbdos", "vbdos", "row-major", false, false);

    let module = &source.modules[0];
    let invoke = module.functions.iter().find(|function| function.name == "INVOKE").expect("INVOKE");
    let types: IndexMap<i64, &hir::Type> = module.types.iter().map(|type_| (type_.id, type_)).collect();
    let values: IndexMap<i64, &hir::Type> = invoke.values.iter().map(|value| (value.id, types[&value.r#type])).collect();
    let instructions: Vec<&hir::Instruction> = invoke.blocks.iter().flat_map(|block| &block.instructions).collect();
    let call_at = instructions
        .iter()
        .position(|one| one.op == hir::Op::Call && one.callee.as_deref() == Some("CONSUME"))
        .expect("the CONSUME call");
    let Operand::ValueRef(argument) = &instructions[call_at].operands[0] else { panic!("a ValueRef argument") };
    assert_eq!(values[&argument.value].name, "near*integer");
    assert!(instructions.iter().any(|one| one.op == hir::Op::Load && matches!(one.operands[0], Operand::IndirectPlace(_))));
    assert!(
        instructions[call_at + 1..]
            .iter()
            .any(|one| one.op == hir::Op::Store && matches!(one.operands[0], Operand::IndirectPlace(_)))
    );

    assert!(!object_bytes(&source, "FARFIELD.BAS").expect("emits").is_empty());
}

/// Q45N01's native SUB SP shifted B$ENRA's documented frame fields by four bytes.
#[test]
fn test_runtime_frame_owns_spill_reservation_without_a_native_prefix() {
    let source = parsed_as(&compat("qb45/q45n01.bas"), "qb45", "qb45");
    let text = listing(&source);
    let procedure = between(&text, "$QB$MAIN proc far", "$QB$MAIN endp");
    let before_runtime_frame = procedure.split_once("call far ptr B$ENRA").map_or(procedure, |(before, _)| before);
    assert!(!before_runtime_frame.contains("sub sp"));
}

/// LOCERR retains an ABI entry before its independently resumable ERROR.
///
/// Threading the empty language entry into that statement made late
/// B$ENRA/B$OEGP insertion miss its block, so the emitted procedure began at
/// ERROR 53 with no runtime frame or local handler registration.
#[test]
fn test_local_error_and_resume_label_use_their_measured_procedure_abi() {
    let source = parsed_with(&compat("vbdos/locerr.bas"), "vbdos", "vbdos", "row-major", false, false);
    let (_, recover) = function_named(&source, "RECOVER_LOCALLY");
    assert!(recover.error_handler.is_some());
    assert!(recover.error_handler_local);

    let text = listing(&source);
    let procedure = between(&text, "RECOVER_LOCALLY proc far", "RECOVER_LOCALLY endp");
    let entered = procedure.find("call far ptr B$ENRA").expect("B$ENRA");
    let registered = procedure.find("call far ptr B$OEGP").expect("B$OEGP");
    let resumed = procedure.find("call far ptr B$RESA").expect("B$RESA");
    assert!(entered < registered && registered < resumed);
    assert!(procedure[..resumed].contains("mov word ptr [bp-22]"));
    assert!(!procedure.contains("call far ptr B$OEGA"));
    assert!(procedure[registered..resumed].contains("mov ax, offset"));
}

/// PDLOCAL reported ERL 0 and resumed at L1_9 instead of recovered L1_4.
#[test]
fn test_pds_resume_target_and_numbered_erl_survive_distinct_identity_spaces() {
    let source = parsed_as(&compat("pds71/pdlocal.bas"), "pds71", "pds71");
    let text = listing(&source);
    let procedure = between(&text, "MISSINGFILE proc far", "MISSINGFILE endp");
    assert!(procedure.contains("mov ax, offset L1_4\n    call far ptr B$RESA"));

    let statement_table = between(&text, "$QB$STAT proc near", "$QB$STAT endp");
    let rows: Vec<&str> =
        statement_table.lines().map(str::trim).filter(|line| line.starts_with("db ")).collect();
    assert!(rows.len() > 4);
    assert_eq!(rows[..rows.len() - 1].iter().copied().collect::<BTreeSet<_>>(), BTreeSet::from(["db 064h,000h"]));
    assert_eq!(rows[rows.len() - 1], "db 000h,000h");
}

/// PDHUGE wrapped/aliased beyond 64 KiB when /Ah was dropped and B$HARY was guessed inline.
#[test]
fn test_pds_huge_array_uses_measured_ddim_and_hary_abi() {
    let source = parsed_with(&compat("pds71/pdhuge.bas"), "pds71", "pds71", "row-major", true, false);
    let function = &source.modules[0].functions[0];
    let body = semantic(&source, 0);
    let optimized = optimized(&source, function, &body);
    let physical = physical(&source, function, &optimized);
    let text = mir_text(&physical.lowered);

    assert!(text.contains("v2 <- copy 65534:2"));
    assert!(text.contains("arg v2:2\n  arg 198:2\n  arg 0:2\n  arg 200:2"));
    assert!(text.contains("arg 2:2\n  arg 514:2"));
    assert_eq!(text.matches("call B$HARY(").count(), 10);
    assert!(has_hary_pair(&text));
    assert!(text.contains("+v4@v5):2 <- 123:2"));

    let assembly = listing(&source);
    assert_eq!(assembly.matches("call far ptr B$HARY").count(), 10);
    assert!(assembly.contains("call far ptr B$HARY\n    mov word ptr es:[bx], 123"));
}

/// Q45P04 passed uninitialized slots after optimization deleted 100000 and 23.
#[test]
fn test_byref_call_keeps_the_temporary_values_it_publishes() {
    let source = parsed_as(&compat("qb45/q45p04.bas"), "qb45", "qb45");
    let function = &source.modules[0].functions[0];
    let body = semantic(&source, 0);
    let text = mir_text(&optimized(&source, function, &body));
    assert!(text.contains("100000:4"));
    assert!(text.contains("23:4"));
}

/// FSTKBR's ``PICK = -1/0`` formerly left a volatile FILD live over its arm jump.
///
/// Float allocation then refused the join with ``floating stack live-out
/// requires cross-block allocation``.  `$arg` is only published when an
/// ADDRESS reaches a BYREF call; an ordinary conversion scratch slot must
/// retain neither that publication nor an x87 value after its exact store
/// folds to bits.
#[test]
fn test_unpublished_float_conversion_temporary_does_not_hold_the_x87_stack_across_a_branch() {
    let source = parsed(&fixture("fstkbr.bas"));
    let (index, function) = function_named(&source, "PICK");
    let body = semantic(&source, index);
    let physical = physical(&source, function, &body);

    let fild = |one: &&mir::Op| one.kind == Kind::Fload && one.name == "fild";
    let scratch_loads: Vec<&mir::Op> = ops(&physical.lowered.body).into_iter().filter(fild).collect();
    assert!(!scratch_loads.is_empty() && scratch_loads.iter().all(|operation| !operation.volatile));

    let optimized = qb_compile::optimized_physical(&source, function, &physical.lowered, &O2()).expect("optimizes");
    assert!(!ops(&optimized.body).iter().any(fild));
    assert!(!object_bytes(&source, "FSTKBR.BAS").expect("emits").is_empty());
}

fn contracts_by_name(physical: &super::abi::Physicalized) -> IndexMap<String, Contract> {
    physical.calls.iter().map(|(at, name)| (name.clone(), physical.contracts[at].clone())).collect()
}

/// The first SEEK stage reached ABI refinement but referenced no base contract.
#[test]
fn test_positioned_file_calls_have_audited_pascal_cleanup() {
    let source = parsed(&fixture("positioned_io.bas"));
    let function = &source.modules[0].functions[0];
    let physical = physical(&source, function, &semantic(&source, 0));
    let contracts = contracts_by_name(&physical);
    assert_eq!(contracts["B$SSEK"].cleanup, Some(6));
    assert_eq!(contracts["B$GET4"].cleanup, Some(12));
    assert_eq!(contracts["B$PUT4"].cleanup, Some(12));
    assert!(contracts.values().all(|contract| contract.established));
}

/// screen and mod_tex need STRING$ and LEFT$ to survive physicalization.
#[test]
fn test_string_builders_have_descriptor_stack_contracts() {
    let source = parsed(&fixture("string_builders.bas"));
    let function = &source.modules[0].functions[0];
    let physical = physical(&source, function, &semantic(&source, 0));
    let contracts = contracts_by_name(&physical);
    assert_eq!(contracts["B$LEFT"].cleanup, Some(4));
    assert_eq!(contracts["B$STRI"].cleanup, Some(4));
    assert_eq!(contracts["B$STRS"].cleanup, Some(4));
}

/// Q45LE71 reached B$LEFT but emission refused the previously VBDOS-only cleanup.
#[test]
fn test_classic_string_stack_abis_are_measured_for_every_qb_runtime_family() {
    for family in [hir::RuntimeProfile::Qb45, hir::RuntimeProfile::Pds71, hir::RuntimeProfile::Vbdos] {
        for (name, pushed) in [
            ("B$LEFT", 4),
            ("B$RGHT", 4),
            ("B$UCAS", 2),
            ("B$FHEX", 4),
            ("B$FMKS", 4),
            ("B$FMKD", 8),
            ("B$FMSF", 4),
            ("B$FMDF", 8),
            ("B$FCVI", 2),
            ("B$FCVL", 2),
            ("B$FCVS", 2),
            ("B$FCVD", 2),
            ("B$MCVS", 2),
            ("B$MCVD", 2),
            ("B$INS3", 6),
        ] {
            let contract = _contract(name, hir::StackCleanup::Callee, pushed, family).expect("a contract");
            assert!(contract.established, "{name} {family:?}");
            assert_eq!(contract.cleanup, Some(pushed), "{name} {family:?}");
            assert_eq!(contract.inputs, Some(BTreeSet::new()), "{name} {family:?}");
        }
    }
}

/// Nibbles reached physical HIR and stopped at unaudited screen-call cleanup.
#[test]
fn test_vbdos_nibbles_screen_calls_have_fixed_stack_contracts() {
    for (name, pushed) in [("B$SCLS", 2), ("B$VWPT", 4), ("B$SPLY", 2), ("B$INKY", 0), ("B$USNG", 2)] {
        let contract =
            _contract(name, hir::StackCleanup::Callee, pushed, hir::RuntimeProfile::Vbdos).expect("a contract");
        assert!(contract.established, "{name}");
        assert_eq!(contract.cleanup, Some(pushed), "{name}");
        assert_eq!(contract.inputs, Some(BTreeSet::new()), "{name}");
    }
}


/// Q45FP61 reached OBJ emission with one unencodable eight-byte PUSH.
#[test]
fn test_double_runtime_argument_is_split_high_to_low_at_the_qb_abi_boundary() {
    let program = parsed_as(&compat("qb45/q45fp61.bas"), "qb45", "qb45");
    let function = &program.modules[0].functions[0];
    let body = semantic(&program, 0);
    let optimized = optimized(&program, function, &body);
    let physical = physical(&program, function, &optimized);
    let double_call = *physical.calls.iter().find(|(_, name)| *name == "B$FMKD").expect("B$FMKD").0;
    let block = physical
        .lowered
        .body
        .blocks
        .iter()
        .find(|block| block.ops.iter().any(|op| op.at == double_call))
        .expect("the block");
    let call_index = block.ops.iter().position(|op| op.at == double_call).expect("the call");
    let parts = &block.ops[call_index - 2..call_index];
    assert!(parts.iter().all(|op| op.kind == Kind::Arg));
    let cell = |op: &mir::Op| match &op.args[0] {
        Arg::Cell(cell) => cell.r#ref.clone(),
        other => panic!("not a cell: {other:?}"),
    };
    assert_eq!(parts.iter().map(|op| cell(op).width).collect::<Vec<_>>(), [4, 4]);
    let displacements: Vec<i64> = parts.iter().map(|op| cell(op).addr.expect("an address").disp).collect();
    let mut sorted = displacements.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(displacements, sorted);
}

/// screen's green/blue fields formerly became an unlowerable address-of far cell.
#[test]
fn test_dynamic_fixed_field_address_reaches_lir_as_pointer_arithmetic() {
    let source = parsed(&fixture("dynamic_fixed_fields.bas"));
    let function = &source.modules[0].functions[0];
    let physical = physical(&source, function, &semantic(&source, 0));
    let lowered = machine(
        &physical.lowered.name,
        &physical.lowered.body,
        &physical.calls,
        &physical.contracts,
        Some(physical.pointer_model.clone()),
    );
    let insns = lowered.insns();
    assert!(!insns.is_empty());
    assert!(
        insns
            .iter()
            .filter_map(|instruction| instruction.what.as_ref())
            .filter(|what| what.op == Operation::Address)
            .flat_map(|what| &what.sources)
            .all(|source| !matches!(source, Loc::Mem(mem) if mem.width != 2))
    );
}

/// sc_selftest's SEG array argument formerly selected illegal ``[bp+bx]``.
#[test]
fn test_segmented_local_array_address_splits_frame_base_from_dynamic_offset() {
    let source = parsed(&fixture("segmented_local_array.bas"));
    let (index, _) = function_named(&source, "PROBE");
    let semantic = semantic(&source, index);
    let addresses: Vec<&mir::Op> = ops(&semantic.body).into_iter().filter(|one| one.kind == Kind::Address).collect();
    assert!(!addresses.is_empty());
    assert!(
        addresses
            .iter()
            .flat_map(|operation| &operation.args)
            .all(|argument| !matches!(argument, Arg::Cell(cell) if cell.r#ref.base.is_some()))
    );
    assert!(ops(&semantic.body).iter().any(|operation| operation.kind == Kind::Add));
}

/// common's g.env.cam_script formerly reached selection as ``lea [abs+offset]``.
#[test]
fn test_byref_fixed_string_field_forms_far_offset_without_absolute_lea() {
    let source = parsed(&fixture("byref_fixed_string_field.bas"));
    let (index, _) = function_named(&source, "FILL");
    let semantic = semantic(&source, index);
    assert!(ops(&semantic.body).iter().all(|operation| {
        operation.args.iter().all(|argument| {
            !(operation.kind == Kind::Address
                && matches!(argument, Arg::Cell(cell)
                    if cell.r#ref.addr.is_some_and(|addr| addr.space == Space::Literal) && cell.r#ref.base.is_none()))
        })
    }));
}

/// common's VAL result formerly vanished because its following load named no MIR use.
#[test]
fn test_runtime_pointer_result_used_as_memory_base_is_an_explicit_mir_use() {
    let source = parsed(&fixture("val.bas"));
    let semantic = semantic(&source, 0);
    let all = ops(&semantic.body);
    let calls: Vec<&&mir::Op> =
        all.iter().filter(|operation| operation.kind == Kind::Call && operation.name == "B$FVAL").collect();
    assert!(!calls.is_empty());
    for call in calls {
        let result = call.defines[0];
        let consumers: Vec<&&mir::Op> = all
            .iter()
            .filter(|operation| {
                operation.args.iter().any(|argument| matches!(argument, Arg::Cell(cell) if cell.r#ref.base == Some(result)))
            })
            .collect();
        assert!(!consumers.is_empty() && consumers.iter().all(|operation| operation.uses.contains(&result)));
    }
}

/// ent copied VEC3 as a fictitious 12-byte register before aggregate lowering.
#[test]
fn test_udt_assignment_is_scalar_memory_copy_not_wide_register_value() {
    let source = parsed(&fixture("aggregate_copy.bas"));
    let (index, _) = function_named(&source, "COPYVEC");
    let semantic = semantic(&source, index);
    assert!(
        ops(&semantic.body)
            .iter()
            .flat_map(|operation| operation.args.iter().chain(&operation.results))
            .all(|argument| !matches!(argument, Arg::Held(held) if held.width > 4))
    );
}

/// screen passed extended values directly to a SINGLE-by-value UGL call.
#[test]
fn test_byval_float_is_stored_at_declared_width_before_stack_push() {
    let source = parsed(&fixture("byval_float.bas"));
    let function = &source.modules[0].functions[0];
    let physical = physical(&source, function, &semantic(&source, 0));
    let lowered = machine(
        &physical.lowered.name,
        &physical.lowered.body,
        &physical.calls,
        &physical.contracts,
        Some(physical.pointer_model.clone()),
    );
    let allocated = float_allocated(&lowered, &physical.calls);
    let pushes: Vec<Loc> = allocated
        .insns()
        .iter()
        .filter_map(|one| one.what.as_ref())
        .filter(|what| what.op == Operation::Push)
        .map(|what| what.sources[0].clone())
        .collect();
    // MIR retains the declared 4-byte and 8-byte values. Machine lowering
    // expands the qword argument into two legal 386 dword pushes, so the raw
    // allocated/assembly shape is three dword pushes (12 stack bytes), not a
    // nonexistent x86 `push qword`.
    assert_eq!(pushes.iter().map(loc_width).collect::<Vec<_>>(), [4, 4, 4]);
}

/// ent.bas reached B$RDIM with stack arguments but an object-raiser GP liveness contract.
#[test]
fn test_redim_stack_contract_uses_typed_rank_cleanup_not_register_arguments() {
    let void = hir::Type::new(0, "void", hir::TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let (instruction, call) = stack_call(
        [0, 9, 4, 257, 0].into_iter().map(|value| Operand::constant(1, value)).collect(),
        "B$RDIM",
        vec![0, 1, 2, 3, 4],
    );
    let block = returning(1, vec![instruction]);
    let function = hir::Function { calls: vec![call], ..hir::Function::new(1, "redim", 0, vec![], vec![], vec![block], 1) };
    let source = vbdos(hir::Module::new(1, "array", vec![void, integer], vec![function.clone()]));
    let semantic = semantic(&source, 0);
    let physical = physical(&source, &function, &semantic);
    let call_op =
        physical.lowered.body.blocks[0].ops.iter().find(|one| one.kind == Kind::Call).expect("the call");
    let contract = &physical.contracts[&call_op.at];
    assert_eq!(contract.cleanup, Some(10));
    assert_eq!(contract.inputs, Some(BTreeSet::new()));
    assert!(contract.established);
    assert!(!machine("redim", &physical.lowered.body, &physical.calls, &physical.contracts, None).insns().is_empty());
}

/// r_bsp reached B$ERAS, whose VBDOS object contract measured cleanup but stayed conservative.
#[test]
fn test_vbdos_erase_uses_typed_stack_call_without_weakening_unknown_effects() {
    let void = hir::Type::new(0, "void", hir::TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let (instruction, call) = stack_call(vec![Operand::constant(1, 0)], "B$ERAS", vec![0]);
    let block = returning(1, vec![instruction]);
    let function = hir::Function { calls: vec![call], ..hir::Function::new(1, "erase", 0, vec![], vec![], vec![block], 1) };
    let source = vbdos(hir::Module::new(1, "array", vec![void, integer], vec![function.clone()]));
    let physical = physical(&source, &function, &semantic(&source, 0));
    let contract = physical.contracts.values().next().expect("a contract");
    assert_eq!(contract.cleanup, Some(2));
    assert_eq!(contract.inputs, Some(BTreeSet::new()));
    assert!(contract.established);
    assert_eq!(contract.writes.name(), "ANY");
}

/// d_poly's SIN must stay an inline float value, not become B$SIN or cross CALL.
#[test]
fn test_qb_inline_sin_reaches_allocated_lir_without_a_runtime_call() {
    let void = hir::Type::new(0, "void", hir::TypeKind::Void, 0);
    let single = float_type(1, "single", 4);
    let values = vec![hir::Value { id: 1, r#type: 1 }, hir::Value { id: 2, r#type: 1 }];
    let result = hir::Place { extent: Some(4), ..hir::Place::new(1, "answer", 1, hir::Storage::Local, -4) };
    let block = returning(
        1,
        vec![
            hir::Instruction::new(1, hir::Op::Fsin, vec![2], vec![Operand::value_ref(1)]),
            hir::Instruction::new(2, hir::Op::Store, vec![], vec![Operand::place_ref(1), Operand::value_ref(2)]),
        ],
    );
    let function = hir::Function {
        parameters: vec![1],
        ..hir::Function::new(1, "wave", 0, values, vec![result], vec![block], 1)
    };
    let source = vbdos(hir::Module::new(1, "trig", vec![void, single], vec![function.clone()]));
    let semantic = semantic(&source, 0);
    // Recognition belongs at the HIR -> MIR boundary. A previous adapter
    // hid SIN as a pseudo CALL until ABI physicalization, turning pure math
    // into an opaque control and memory barrier for every optimizer.
    assert!(ops(&semantic.body).iter().all(|op| op.kind != Kind::Call));
    assert!(ops(&semantic.body).iter().any(|op| op.name == "fsin"));
    let physical = physical(&source, &function, &semantic);
    let operations = &physical.lowered.body.blocks[0].ops;
    assert_eq!(operations[..2].iter().map(|one| one.kind).collect::<Vec<_>>(), [Kind::Fload, Kind::Fsqrt]);
    assert_eq!(operations[1].name, "fsin");
    assert!(mir_text(&physical.lowered).contains(" fsin "));
    assert!(!mir_text(&physical.lowered).contains(" fsqrt "));
    let lowered = machine("wave", &physical.lowered.body, &physical.calls, &physical.contracts, None);
    let allocated = float_allocated(&lowered, &physical.calls);
    let whats: Vec<ir::Semantics> = allocated.insns().iter().filter_map(|one| one.what.clone()).collect();
    assert!(whats.iter().any(|what| what.name.as_deref() == Some("fsin")));
    assert!(!whats.iter().any(|what| what.op == Operation::Call));
    let last = finalized(&allocated, 0).expect("finalizes");
    let inline = last.callees.values().next().expect("an inline callee");
    assert_eq!(inline.code, vec![masm::InlinePart::Bytes(vec![0xd9, 0xfe])]);
    assert!(last.body.insns().iter().any(|one| one.what.as_ref().is_some_and(|what| what.op == Operation::Call)));
    let assembly = masm::text(&masm::Module {
        code: "TRIG_TEXT".into(),
        names: IndexMap::default(),
        externs: vec![],
        publics: vec!["wave".into()],
        data: vec![],
        procedures: vec![masm::Procedure {
            name: "wave".into(),
            public: true,
            far: true,
            body: last.body,
            reserve: 4,
            callees: last.callees,
        }],
        private: BTreeSet::new(),
    })
    .expect("prints");
    assert!(assembly.contains("db 0d9h,0feh"));
    assert!(!assembly.contains("call fsin"));
}

/// A fresh QB procedure must end in RETF n; semantic MIR carries no stack ABI bytes.
#[test]
fn test_qb_finalizer_attaches_callee_cleanup_to_far_return() {
    let returned = lir::Insn::new(
        1,
        Some((1, 1)),
        Some(ir::Semantics { name: Some(String::new()), ..ir::Semantics::new(Operation::Return) }),
        vec![],
        vec![],
    );
    let body = lir::LirBody::new(
        "callee",
        1,
        vec![lir::LirBlock::new(1, vec![Arc::new(returned)])],
        IndexMap::default(),
        IndexMap::default(),
    );
    let last = finalized(&body, 6).expect("finalizes");
    let returned = last.body.insns()[0].what.clone().expect("a return");
    assert_eq!(returned.sources, vec![Loc::Imm(ir::Imm { value: 6, width: 2, address: None })]);
}

/// The `physicalize` half; the `hir.lower` half is in `src/hir/test_hir.rs`.
#[test]
fn test_hir_lowers_whole_pointer_indirect_memory_without_machine_registers() {
    let void = hir::Type::new(0, "void", hir::TypeKind::Void, 0);
    let long = int_type(1, "long", 4);
    let pointer = hir::Type {
        element: Some(1),
        address: hir::AddressKind::Huge,
        ..hir::Type::new(2, "huge*long", hir::TypeKind::Pointer, 4)
    };
    let values = vec![hir::Value { id: 1, r#type: 2 }, hir::Value { id: 2, r#type: 1 }];
    let block = returning(
        1,
        vec![hir::Instruction::new(
            1,
            hir::Op::Load,
            vec![2],
            vec![Operand::IndirectPlace(hir::IndirectPlace { base: 1, offset: 0, r#type: 1, volatile: false, inbounds: false })],
        )],
    );
    let function = hir::Function { parameters: vec![1], ..hir::Function::new(1, "read", 0, values, vec![], vec![block], 1) };
    let source = vbdos(hir::Module::new(1, "pointer", vec![void, long, pointer], vec![function.clone()]));
    let semantic = lower(&crate::hir::decode(&crate::hir::encode(&source, None).unwrap()).unwrap()).unwrap().remove(0);
    // tools/qbstages first exposed that the source ABI adapter had omitted
    // DOS's established huge-pointer model: valid far byte loads reached MIR
    // and then failed at lowering with "needs an established pointer ABI".
    let physical = physical(&source, &function, &semantic);
    assert!(
        !machine("read", &physical.lowered.body, &physical.calls, &physical.contracts, Some(physical.pointer_model.clone()))
            .insns()
            .is_empty()
    );
}

/// RPOINTLEAF treated readonly RPLANEDIST as a write to every descriptor.
#[test]
fn test_qb_module_instantiates_user_callee_modref_on_pointer_actuals() {
    let void = hir::Type::new(0, "void", hir::TypeKind::Void, 0);
    let integer = int_type(1, "integer", 2);
    let pointer = hir::Type {
        element: Some(1),
        address: hir::AddressKind::Near,
        ..hir::Type::new(2, "near*integer", hir::TypeKind::Pointer, 2)
    };
    let (read_call, call) = stack_call(vec![Operand::value_ref(1)], "READ", vec![0]);
    let caller = hir::Function {
        parameters: vec![1],
        calls: vec![hir::CallAbi { callee: Some(1), ..call }],
        ..hir::Function::new(1, "CALLER", 0, vec![hir::Value { id: 1, r#type: 2 }], vec![], vec![returning(1, vec![read_call])], 1)
    };
    let callee = hir::Function {
        parameters: vec![1],
        ..hir::Function::new(
            2,
            "READ",
            0,
            vec![hir::Value { id: 1, r#type: 2 }, hir::Value { id: 2, r#type: 1 }],
            vec![],
            vec![returning(
                1,
                vec![hir::Instruction::new(
                    1,
                    hir::Op::Load,
                    vec![2],
                    vec![Operand::IndirectPlace(hir::IndirectPlace { base: 1, offset: 0, r#type: 1, volatile: false, inbounds: false })],
                )],
            )],
            1,
        )
    };
    let module = hir::Module {
        callables: vec![hir::Callable {
            id: 1,
            name: "READ".into(),
            result_type: None,
            parameter_types: vec![1],
            by_value: vec![false],
            segmented: vec![false],
            arrays: vec![false],
            defined: true,
        }],
        ..hir::Module::new(1, "modref", vec![void, integer, pointer], vec![caller, callee])
    };
    let program = vbdos(module.clone());

    let bodies = qb_compile::_alias_annotated(&module, &module.functions, &lower(&program).unwrap()).expect("annotates");
    let call = ops(&bodies[0].body).into_iter().find(|one| one.kind == Kind::Call).expect("the call");

    assert!(call.memory_complete);
    assert!(!call.loads.is_empty());
    assert!(call.stores.is_empty());
}

/// SCMPABI's B$SCMP ABI site was dropped because STRING_EQ is not Op.CALL.
#[test]
fn test_qb_string_comparison_abi_site_survives_alias_annotation() {
    let source = parsed(&fixture("scmpabi.bas"));
    let (_, function) = function_named(&source, "MATCHES");
    let instruction = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|one| one.id == function.calls[0].instruction)
        .expect("the site");

    assert_eq!(instruction.op, hir::Op::StringEq);
    assert!(listing(&source).contains("call far ptr B$SCMP"));
}

/// IN_KEYSTROKE held a released key forever after GVN kept its first read.
///
/// A BYREF pointee is published storage: an interrupt or another runtime
/// callback may change it without an ordinary source store.  Both the guard
/// and the back-edge condition must therefore remain observable loads.
#[test]
fn test_byref_loop_condition_reloads_the_published_pointee() {
    let source = parsed_as(&fixture("byreflp.bas"), "vbdos", "vbdos");
    let (_, function) = function_named(&source, "WAITKEY");
    let semantic = lower(&source).unwrap().into_iter().find(|one| one.name.ends_with("WAITKEY")).expect("WAITKEY");
    let optimized = optimized(&source, function, &semantic);
    let loads: Vec<&mir::Op> = ops(&optimized.body).into_iter().filter(|one| one.kind == Kind::Load).collect();

    let natural = loops::loops(&optimized.body.blocks, Some(optimized.body.entry));
    let inside: BTreeSet<i64> = natural.iter().flat_map(|one| one.body.iter().copied()).collect();

    assert_eq!(loads.len(), 2);
    assert!(loads.iter().all(|one| one.volatile && one.loads.iter().any(|reference| reference.volatile)));
    assert!(optimized.body.blocks.iter().any(|block| {
        inside.contains(&block.at) && block.ops.iter().any(|one| one.kind == Kind::Load && one.volatile)
    }));
}

/// ENTPHI lost a dynamic-array address after its identity phi edge vanished.
///
/// The source's `CASE 0, 1` join reaches a field through an address value
/// which has the same physical register on both edges.  Its copies therefore
/// emit nothing, but their virtual definition must survive until all LIR
/// control-flow threading is complete.
#[test]
fn test_identity_phi_edge_survives_control_flow_threading() {
    let source = parsed_with(&fixture("entphi.bas"), "vbdos", "vbdos", "row-major", false, false);
    assert!(!object_bytes(&source, "ENTPHI.BAS").expect("emits").is_empty());
}

/// `1 <= n` lowered to `cmp 1, bx`, which x86 cannot encode; UBOUND made it on every array.
#[test]
fn test_a_constant_on_the_left_of_a_comparison_still_encodes() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "LEFT.BAS",
        b"DEFINT A-Z\nDECLARE SUB Show (n)\nShow 3\nSUB Show (n)\n  IF 1 <= n THEN PRINT \"YES\"\nEND SUB\n",
    );
    let program = parsed(&source);

    assert!(!object_bytes(&program, "LEFT.BAS").expect("emits").is_empty());
    assert!(!listing(&program).contains("cmp 1,"));
}

/// sumThree reloaded three descriptors and stored `total` after every add.
///
/// Neither the parameter pointers nor B$UBND can reach a static whose
/// address the module never hands out.
#[test]
fn test_array_parameters_do_not_pin_private_statics_inside_their_loop() {
    let procedure = sum_three(false);
    let loop_ = &backward_loop(&procedure);

    assert!(!["bx", "si", "di"]
        .iter()
        .any(|base| ["2", "10"].iter().any(|disp| loop_.contains(&format!("word ptr [{base}+{disp}]")))));
    assert!(static_stores(loop_) <= 2); // total and index, once each
    assert!(!loop_.contains("call"));
}

/// Every UBOUND called B$UBND, whose unknown writes pinned all memory around it.
///
/// The descriptor holds the bounds; the call remains only where the runtime
/// would raise "Subscript out of range".
#[test]
fn test_array_bounds_are_read_from_the_descriptor() {
    let procedure = sum_three(false);

    assert!(has_indexed_field(&procedure, "16"));
    assert!(has_indexed_field(&procedure, "14"));
}

/// sumThree stored its STATIC `total` and `index` on every iteration.
///
/// Proving the cells held the loop's entry values walked back through the
/// B$LBND fallback and refused every call, although that call's modelled
/// effects cannot reach a private static.
#[test]
fn test_static_locals_are_stored_once_after_the_loop() {
    let procedure = sum_three(false);
    assert!(!backward_loop(&procedure).contains("SUM_THREE$D"));
}

/// LBOUND's runtime call was the fall-through; the descriptor read sat behind three jumps.
///
/// The call only raises "Subscript out of range", so the frontend marks its
/// block cold and layout places it after the return.
#[test]
fn test_the_bound_error_call_is_placed_after_the_hot_path() {
    let procedure = sum_three(false);

    let returned = procedure.find("retf").expect("retf");
    assert!(procedure.find("B$LBND").expect("B$LBND") > returned);
    assert!(procedure.find("B$UBND").expect("B$UBND") > returned);
}

/// An ELSE that only raises an error was laid out before the return.
///
/// Nothing marks it cold: B$SERR never returns, and that is enough.
#[test]
fn test_an_error_statement_is_placed_after_the_hot_path() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "RAISE.BAS",
        b"DEFINT A-Z\nDECLARE SUB Check (n)\nCheck 3\nSUB Check (n)\n  IF n >= 0 THEN\n    PRINT n\n  ELSE\n    ERROR 5\n  END IF\nEND SUB\n",
    );
    let text = listing(&parsed(&source));
    let procedure = &text[text.find("CHECK proc").expect("CHECK proc")..text.find("CHECK endp").expect("CHECK endp")];

    assert!(procedure.find("B$SERR").expect("B$SERR") > procedure.find("retf").expect("retf"));
}

/// LBOUND(a, 1) compared the rank against 1 before reading the descriptor.
///
/// Every allocated array has a first dimension, so only the allocation
/// test may branch to the runtime call.
#[test]
fn test_the_first_dimension_is_not_rank_checked() {
    let procedure = sum_three(false);

    let calls = bound_call_labels(&procedure);
    let branches = jumps(&procedure, false);
    assert_eq!(branches.iter().filter(|(_, _, label)| calls.contains(label)).count(), 2);
}

/// --unchecked-bounds trusts the descriptor: no B$LBND/B$UBND fallback.
#[test]
fn test_unchecked_bounds_read_the_descriptor_without_runtime_calls() {
    let procedure = sum_three(true);

    assert!(!procedure.contains("B$LBND") && !procedure.contains("B$UBND"));
    assert!(has_indexed_field(&procedure, "16"));
}

/// OUT/POKE of a SINGLE raised Unlowered (no one-byte fistp), and the
/// listing printed `out dx` / `in al` without their second operand.
#[test]
fn test_port_io_narrows_a_float_through_integer_and_prints_both_operands() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "PORTS.BAS",
        b"defint a-z\r\np = &H3C8: f! = 41.6\r\nout p, f!\r\npoke 0, f!\r\na = inp(p + 1)\r\n",
    );
    let source = parsed_as(&basic, "qb45", "qb45");
    let lines: BTreeSet<String> = stripped_lines(&listing(&source)).into_iter().collect();
    assert!(lines.contains("out dx, al") && lines.contains("in al, dx"), "{lines:?}");
    assert!(lines.iter().any(|line| line.starts_with("fistp word ptr")), "{lines:?}");
}

/// Qlight printed 3492255: 1000000 was lexed as INTEGER 0x4240 and sign-extended.
#[test]
fn test_an_unsuffixed_decimal_above_32767_is_a_long_literal() {
    let directory = tempfile::TempDir::new().unwrap();
    let (program, function) = function_in(
        &directory,
        "scale.bas",
        "function qlightScale (word as integer) as long\nqlightScale = clng(word) * 1000000\nend function\n",
        "QLIGHTSCALE",
        "qb45",
    );
    let widths: IndexMap<i64, i64> =
        program.modules.iter().flat_map(|module| &module.types).map(|one| (one.id, one.width)).collect();
    let constants: Vec<&hir::Constant> = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .flat_map(|instruction| &instruction.operands)
        .filter_map(|operand| match operand {
            Operand::Constant(constant)
                if matches!(constant.value, hir::Number::Int(1_000_000))
                    || matches!(constant.value, hir::Number::Float(value) if value == 1_000_000.0) =>
            {
                Some(constant)
            }
            _ => None,
        })
        .collect();

    assert!(!constants.is_empty() && constants.iter().all(|one| widths[&one.r#type] == 4));
}

/// Loading TOTAL before B$ERAS let the cleanup call clobber the returned value.
#[test]
fn test_a_long_function_reloads_its_result_after_erasing_local_arrays() {
    let directory = tempfile::TempDir::new().unwrap();
    let (_, function) = function_in(
        &directory,
        "total.bas",
        "function total as long\ndim cells(0 to 0) as integer\ntotal = 42\nend function\n",
        "TOTAL",
        "vbdos",
    );
    let result = function.places.iter().find(|one| one.name == "TOTAL").expect("TOTAL").id;
    let exit = value_exit(&function);
    let calls = callees(exit);
    let loads: Vec<usize> = exit
        .instructions
        .iter()
        .enumerate()
        .filter(|(_, one)| one.op == hir::Op::Load && one.operands == [Operand::place_ref(result)])
        .map(|(at, _)| at)
        .collect();

    assert!(!loads.is_empty() && position(&calls, "B$ERAS") < *loads.last().unwrap());
}

/// The scalar reload must not reorder STRING results: B$SCPF runs before B$STDL frees OTHER.
#[test]
fn test_a_string_function_copies_its_result_before_freeing_other_locals() {
    let directory = tempfile::TempDir::new().unwrap();
    let (_, function) = function_in(
        &directory,
        "pick.bas",
        "function pick as string\ndim other as string\nother = \"kept\"\npick = other\nend function\n",
        "PICK",
        "vbdos",
    );
    let exit = value_exit(&function);
    let calls = callees(exit);
    let copied = &exit.instructions[position(&calls, "B$SCPF")];

    assert!(position(&calls, "B$SCPF") < position(&calls, "B$STDL"));
    assert_eq!(exit.terminator.operands, [Operand::value_ref(copied.results[0])]);
}

// ---- tests/test_qbstages.py ----------------------------------------------

fn capture_program() -> hir::Program {
    let void = hir::Type::new(0, "void", hir::TypeKind::Void, 0);
    let body = returning(1, vec![]);
    let function = hir::Function::new(1, "__main", 0, vec![], vec![], vec![body], 1);
    let statements = hir::DataObject { readonly: true, ..hir::DataObject::new(1, "$qb$statementTable", vec![]) };
    let module = hir::Module { data: vec![statements], ..hir::Module::new(1, "capture", vec![void], vec![function]) };
    vbdos(module)
}

/// qbstages used to lower manually, then assemble the same HIR again for its final listing.
///
/// Identity (`is`) becomes equality. The `tools/qbstages.py` half, which
/// counts parses and lowerings through monkeypatching, is not ported.
#[test]
fn test_stage_observer_uses_one_compilation_and_preserves_object_bytes() {
    let program = capture_program();
    let uncaptured = qb_compile::object_bytes(&program, Path::new("capture.bas"), None, &O2()).expect("emits");
    let mut names: Vec<String> = Vec::new();
    let mut first: Option<hir::Program> = None;
    let mut final_lir: Option<lir::LirBody> = None;
    let mut emitted: Option<lir::LirBody> = None;
    let mut observer = |stage: &Stage| {
        names.push(stage.name.clone());
        match stage.value {
            StageValue::Program(value) if first.is_none() => first = Some(value.clone()),
            StageValue::Lir(value) if stage.name == "final-lir" => final_lir = Some(value.clone()),
            StageValue::Module(value) => emitted = Some(value.procedures[0].body.clone()),
            _ => {}
        }
        Ok(())
    };
    let captured =
        qb_compile::object_bytes(&program, Path::new("capture.bas"), Some(&mut observer as &mut StageObserver), &O2())
            .expect("emits");

    assert_eq!(captured, uncaptured);
    assert_eq!(
        names,
        [
            "hir",
            "source-mir",
            "optimized-mir",
            "physical-mir",
            "optimized-physical-mir",
            "rotated-mir",
            "initial-lir",
            "machine:far-indirect-calls",
            "machine:floatalloc",
            "machine:phielim",
            "machine:twoaddr",
            "machine:coalesce",
            "machine:regalloc",
            "machine:parcopy",
            "machine:peephole",
            "machine:schedule",
            "machine:jumps",
            "final-lir",
            "emitted-assembly",
        ]
    );
    assert_eq!(first.as_ref(), Some(&program));
    assert!(final_lir.is_some());
    assert_eq!(final_lir, emitted);
    // skipped: the runpy/monkeypatch half counting parse and lower calls in tools/qbstages.py
}

// ---- tests/test_qb_frontend_command.py -----------------------------------

/// Adding modern/freestanding to common HIR once made QB's driver advertise both.
#[test]
fn test_common_hir_profiles_do_not_become_qb_frontend_options() {
    let source = qb_driver::ROOT().join("not-read.bas");
    let syntax = |dialect: &str, runtime: &str| {
        qb_driver::syntax_checked(&source, dialect, runtime, &[], "column-major", false, false, false, false, false)
            .expect_err("refused")
            .0
    };
    assert!(syntax("modern", "vbdos").contains("unknown QB dialect"));
    assert!(syntax("vbdos", "freestanding").contains("unknown QB runtime"));
}

//! `tests/test_hir.py` QB cases, part 2; helpers in `test_hir`.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::support::hash::IndexMap;

use super::abi::physicalize;
use super::compile as qb_compile;
use super::driver as qb_driver;
use super::test_hir::*;
use crate::hir::model::Operand;
use crate::hir::{self, lower::Lowered};
use crate::model::mir;
use crate::model::passes::O2;
use crate::objectfile::module::CALL_FAR;
use crate::objectfile::omf;

type Records = Vec<Rc<omf::Record>>;

fn hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text.bytes().filter(|byte| !byte.is_ascii_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

/// `haystack.find(needle, start)`.
fn find(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    (start..=haystack.len().saturating_sub(needle.len())).find(|&at| haystack[at..].starts_with(needle))
}

fn lowered(program: &hir::Program) -> Vec<Lowered> {
    hir::lower(program).expect("lowers")
}

fn ops(body: &Lowered) -> Vec<&mir::Op> {
    body.body.blocks.iter().flat_map(|block| &block.ops).collect()
}

fn width(argument: &mir::Arg) -> u32 {
    match argument {
        mir::Arg::Held(one) => one.width,
        mir::Arg::Const(one) => one.width,
        mir::Arg::Symbol(one) => one.width,
        mir::Arg::FrameAddress(one) => one.width,
        mir::Arg::FrameSelector(one) => one.width,
        mir::Arg::Cell(one) => one.r#ref.width,
        mir::Arg::Opaque(one) => panic!("opaque {} has no width", one.name),
    }
}

fn instructions(function: &hir::Function) -> Vec<&hir::Instruction> {
    function.blocks.iter().flat_map(|block| &block.instructions).collect()
}

fn callees(function: &hir::Function) -> Vec<String> {
    instructions(function)
        .into_iter()
        .filter(|one| one.op == hir::Op::Call)
        .map(|one| one.callee.clone().unwrap_or_default())
        .collect()
}

fn named<'f>(module: &'f hir::Module, name: &str) -> &'f hir::Function {
    module.functions.iter().find(|one| one.name == name).unwrap_or_else(|| panic!("no {name}"))
}

fn place<'f>(function: &'f hir::Function, name: &str) -> &'f hir::Place {
    function.places.iter().find(|one| one.name == name).unwrap_or_else(|| panic!("no {name}"))
}

fn relocations(data: &hir::DataObject) -> Vec<(i64, i64, i64, &'static str)> {
    data.relocations.iter().map(|one| (one.at, one.target, one.addend, one.address.value())).collect()
}

fn data_bytes(data: &hir::DataObject, low: usize, high: usize) -> Vec<u8> {
    data.bytes[low..high].iter().map(|&byte| byte as u8).collect()
}

fn indirect_loads(function: &hir::Function) -> Vec<&hir::IndirectPlace> {
    instructions(function)
        .into_iter()
        .filter(|one| one.op == hir::Op::Load)
        .flat_map(|one| &one.operands)
        .filter_map(|operand| match operand {
            Operand::IndirectPlace(place) => Some(place),
            _ => None,
        })
        .collect()
}

fn code(records: &Records) -> Vec<u8> {
    omf::segment_image(records, 1, omf::segments(records)[1].as_ref().expect("code segment").1)
}

fn names(records: &Records) -> Vec<String> {
    omf::segments(records).into_iter().flatten().map(|(name, _)| name).collect()
}

fn by_name(records: &Records) -> IndexMap<String, (i64, i64)> {
    omf::segments(records)
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| item.map(|(name, size)| (name, (index as i64, size))))
        .collect()
}

fn fixup_rows(records: &Records, seg: i64) -> Vec<(i64, i64, String, i64)> {
    omf::fixups(records)
        .into_iter()
        .filter(|one| one.seg == Some(seg))
        .map(|one| (one.offset, one.loc, one.target, one.index))
        .collect()
}

fn row(offset: i64, loc: i64, target: &str, index: i64) -> (i64, i64, String, i64) {
    (offset, loc, target.to_owned(), index)
}

/// `object_module.of(records).calls`: far calls to externals in the code segment.
fn far_calls(records: &Records) -> BTreeMap<i64, String> {
    let (seg, _name, size) = omf::code_segment(records).expect("a code segment");
    let code = omf::segment_image(records, seg, size);
    let externals = omf::externals(records);
    omf::fixups(records)
        .into_iter()
        .filter(|one| {
            one.seg == Some(seg)
                && one.loc == omf::LOC_PTR32
                && one.target == "external"
                && code.get(one.offset as usize - 1) == Some(&CALL_FAR)
        })
        .map(|one| (one.offset - 1, externals[one.index as usize].clone()))
        .collect()
}

fn word_char(byte: Option<&u8>) -> bool {
    byte.is_some_and(|one| one.is_ascii_alphanumeric() || *one == b'_')
}

fn lower_pair(rest: &[u8]) -> bool {
    rest.len() >= 2 && rest[0].is_ascii_lowercase() && rest[1].is_ascii_lowercase()
}

/// `re.search(r"\bshl (?:word ptr \[[^\n]+\]|[a-z]{2}), 4\b", text)`.
fn shl_by_4(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.as_bytes();
        (0..line.len()).any(|at| {
            if !line[at..].starts_with(b"shl ") || (at > 0 && word_char(line.get(at - 1))) {
                return false;
            }
            let rest = &line[at + 4..];
            let tail = |from: usize| rest[from..].starts_with(b", 4") && !word_char(rest.get(from + 3));
            let memory = rest.starts_with(b"word ptr [")
                && (b"word ptr [".len() + 1..rest.len()).any(|close| rest[close] == b']' && tail(close + 1));
            memory || (lower_pair(rest) && tail(2))
        })
    })
}

/// `re.search(r"\bimul\b[^\n]*, 16\b", text)`.
fn imul_by_16(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.as_bytes();
        (0..line.len()).any(|at| {
            line[at..].starts_with(b"imul")
                && !(at > 0 && word_char(line.get(at - 1)))
                && !word_char(line.get(at + 4))
                && (at + 4..line.len()).any(|from| line[from..].starts_with(b", 16") && !word_char(line.get(from + 4)))
        })
    })
}

/// `re.search(r"add (?:word ptr \[[^]]+\]|[a-z]{2}), 16", text)`.
fn add_16(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.as_bytes();
        (0..line.len()).any(|at| {
            if !line[at..].starts_with(b"add ") {
                return false;
            }
            let rest = &line[at + 4..];
            let memory = rest.starts_with(b"word ptr [")
                && rest[10..]
                    .iter()
                    .position(|&byte| byte == b']')
                    .is_some_and(|close| close > 0 && rest[10 + close + 1..].starts_with(b", 16"));
            memory || (lower_pair(rest) && rest[2..].starts_with(b", 16"))
        })
    })
}

/// `re.search(r"cmp word ptr \[bp-[0-9]+\], 0", text)`.
fn cmp_frame_zero(text: &str) -> bool {
    text.match_indices("cmp word ptr [bp-").any(|(at, prefix)| {
        let rest = &text[at + prefix.len()..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        digits > 0 && rest[digits..].starts_with("], 0")
    })
}

/// `re.search(r"push dword ptr ([^\n]+)\n    push offset ([^\n]+)\n    call far ptr ADDHALF", text)`.
fn pushes_then_calls_addhalf(text: &str) -> bool {
    let lines: Vec<&str> = text.split('\n').collect();
    lines.windows(3).any(|three| {
        three[0].split_once("push dword ptr ").is_some_and(|(_, rest)| !rest.is_empty())
            && three[1].strip_prefix("    push offset ").is_some_and(|rest| !rest.is_empty())
            && three[2].starts_with("    call far ptr ADDHALF")
    })
}

/// The default-type probe emitted only for VBDOS: PDS/QB rejected B$FLEN's missing ABI.
#[test]
fn test_default_typed_len_emits_for_each_microsoft_runtime() {
    for (dialect, runtime) in [("qb45", "qb45"), ("pds71", "pds71"), ("vbdos", "vbdos")] {
        let source = parsed_as(&fixture("default-types.bas"), dialect, runtime);
        assert!(!object_bytes(&source, "default-types.bas").expect("emits").is_empty());
    }
}

/// A fresh MAIN failed runtime initialization when U_FLAG claimed /FPa instead of /FPi.
#[test]
fn test_vbdos_module_header_records_the_measured_compiler_switches() {
    let mut source = parsed(&fixture("emission.bas"));
    source.array_order = hir::ArrayOrder::RowMajor;
    let records = records(&source, "emission.bas");
    let code = code(&records);

    // Measured from VBDOS BC /O /FPi /R /G3 /E.
    assert_eq!(u16::from_le_bytes([code[46], code[47]]), 0x13C4);
}

/// QGL stayed in the local heap scanner: empty `_DATA` moved BC_DATA behind the C runtime.
#[test]
fn test_fresh_basic_object_does_not_predeclare_the_c_data_class() {
    let source = parsed(&fixture("emission.bas"));
    let records = records(&source, "emission.bas");
    let names = names(&records);

    assert!(!names.iter().any(|name| name == "_DATA"));
    assert_eq!(
        names,
        [
            "EMISSION_CODE",
            "BR_DATA",
            "BR_SKYS",
            "COMMON",
            "BC_DATA",
            "NMALLOC",
            "ENMALLOC",
            "BC_FT",
            "BC_CN",
            "BC_DS",
            "BC_SAB",
            "BC_SA",
            "FDATA",
            "FSL_CONST",
        ]
    );

    let segments = omf::segments(&records);
    let dgroup: Vec<String> = omf::groups(&records)["DGROUP"]
        .iter()
        .map(|&index| segments[index as usize].as_ref().unwrap().0.clone())
        .collect();
    assert_eq!(dgroup, names[1..names.len() - 2]);
}

/// PDFPA reached LINK, then BCL71ANR rejected the module during initialization.
#[test]
fn test_pds_alternate_math_module_header_records_the_measured_switch() {
    let path = root().join("frontends/qb/compat/pds71/pdfpa.bas");
    let source =
        qb_driver::parsed(&path, &qb_driver::Frontend { alternate_math: true, ..qb_driver::Frontend::new("pds71", "pds71") }, None)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let records = records(&source, "PDFPA.BAS");
    let code = code(&records);

    // Measured from PDS 7.1 BC /O /G2 /FPa: U_FLAG 0x1088 vs /FPi's 0x1084.
    assert_eq!(source.float_mode, hir::FloatMode::Alternate);
    assert_eq!(u16::from_le_bytes([code[46], code[47]]), 0x1088);
}

/// The full source build printed usage because COMMAND$ became an empty implicit local.
#[test]
fn test_command_line_is_resolved_as_the_zero_argument_runtime_intrinsic() {
    let source = parsed(&fixture("command-line.bas"));
    let lowered = &lowered(&source)[0];
    let calls: Vec<&str> =
        ops(lowered).into_iter().filter(|op| op.kind == mir::Kind::Call).map(|op| op.name.as_str()).collect();

    assert_eq!(calls[..3], ["B$FCMD", "B$LTRM", "B$RTRM"]);
}

/// Fresh SYS_TIME_INIT loaded an implicit local forever instead of calling B$TIMR.
#[test]
fn test_timer_loads_the_single_returned_by_the_runtime_clock() {
    let source = parsed(&fixture("timer-basic.bas"));
    let lowered = &lowered(&source)[0];
    let operations = ops(lowered);
    let timer = |op: &mir::Op| op.kind == mir::Kind::Call && op.name == "B$TIMR";

    assert_eq!(operations.iter().filter(|op| timer(op)).count(), 3);
    for (index, operation) in operations.iter().enumerate() {
        if timer(operation) {
            let following = operations[index + 1];
            assert_eq!(following.kind, mir::Kind::Fload);
            assert!(following.floating.is_some());
            assert_eq!(following.uses, operation.defines);
            assert_eq!(width(&following.results[0]), 10);
        }
    }
}

/// PDS TIMER had the measured zero-byte ABI but no table entry, so object emission refused it.
#[test]
fn test_timer_emits_for_each_microsoft_runtime() {
    for (dialect, runtime) in [("qb45", "qb45"), ("pds71", "pds71"), ("vbdos", "vbdos")] {
        let source = parsed_as(&fixture("timer-basic.bas"), dialect, runtime);
        assert!(!object_bytes(&source, "timer-basic.bas").expect("emits").is_empty());
    }
}

/// SYS_TICK_HZ left its SINGLE on x87, while BC callers passed and read a hidden result slot.
#[test]
fn test_qb_float_function_uses_hidden_near_result_pointer() {
    let source = parsed(&fixture("float-function.bas"));
    let function = named(&source.modules[0], "ADDHALF");
    let abi = function.abi.as_ref().expect("an ABI");
    assert_eq!(abi.parameter_bytes, 6);
    assert_eq!(function.parameters.len(), 2);

    let listing = listing(&source);
    assert!(pushes_then_calls_addhalf(&listing));
    assert!(listing.contains("fstp dword ptr [bx]"));
    assert!(listing.contains("mov ax, bx\n    call far ptr B$EXSA\n    pop bp\n    retf"));

    // The MASM printer omits RETF's immediate; the object preserves it.
    let records = records(&source, "float-function.bas");
    let (segment, offset) = omf::public_definitions(&records).expect("pubdefs")["ADDHALF"];
    let image = omf::segment_image(&records, segment, omf::segments(&records)[segment as usize].as_ref().unwrap().1);
    let procedure = &image[offset as usize..];
    let statement_table = find(procedure, &hex("558bec"), 3);
    assert!(statement_table.is_some_and(|at| at > 0));
    assert!(procedure[..statement_table.unwrap()].ends_with(&hex("ca0600")));
}

/// SYS_MEM_MARK saw only memAvail&'s low word: external LONG returns in DX:AX, not EAX.
#[test]
fn test_qb_long_function_boundary_uses_the_legacy_dx_ax_pair() {
    let external = parsed(&fixture("external-long.bas"));
    let external_function = &external.modules[0].functions[0];
    let external_physical =
        physicalize(&external, external_function, &lowered(&external)[0]).expect("physicalizes");
    let body = &external_physical.lowered;
    let call = ops(body)
        .into_iter()
        .find(|op| external_physical.calls.get(&op.at).map(String::as_str) == Some("MEMAVAIL&"))
        .expect("the MEMAVAIL& call");
    assert_eq!(call.results.iter().map(width).collect::<Vec<_>>(), [2, 2]);
    assert!(ops(body).iter().any(|op| op.kind == mir::Kind::Concat && width(&op.results[0]) == 4));

    let internal = parsed(&fixture("bare-function.bas"));
    let bodies = lowered(&internal);
    let (answer, answer_body) = internal.modules[0]
        .functions
        .iter()
        .zip(&bodies)
        .find(|(function, _)| function.name == "ANSWER&")
        .expect("ANSWER&");
    let answer_physical = physicalize(&internal, answer, answer_body).expect("physicalizes");
    let returned = ops(&answer_physical.lowered)
        .into_iter()
        .find(|op| op.kind == mir::Kind::Return)
        .expect("a return");
    assert_eq!(returned.args.iter().map(width).collect::<Vec<_>>(), [2, 2]);
}

/// SYS_PARSE_ARGS exhausted string space when a native shell preceded B$ENRA.
#[test]
fn test_qb_runtime_frame_establishes_and_zero_initializes_managed_locals() {
    let source = parsed(&fixture("managed-locals.bas"));
    let records = records(&source, "managed-locals.bas");
    let (code_segment, start) = omf::public_definitions(&records).expect("pubdefs")["SHOWCOMMAND"];
    let calls: Vec<String> = far_calls(&records).into_iter().filter(|(at, _)| *at >= start).map(|(_, name)| name).collect();

    assert_eq!(calls[..3], ["B$ENRA", "B$DDIM", "B$FCMD"]);
    assert_eq!(calls.last().map(String::as_str), Some("B$EXSA"));
    let image =
        omf::segment_image(&records, code_segment, omf::segments(&records)[code_segment as usize].as_ref().unwrap().1);
    // VBDOS starts the procedure with MOV CX/MOV BX/CALL.
    assert_eq!(image[start as usize..start as usize + 7], hex("b91800bb01009a"));
}

/// SHOWCOMMAND overwrote B$EXSA's frame link and failed at 0825:0086.
#[test]
fn test_vbdos_managed_locals_begin_below_the_runtime_frame_header() {
    let source = parsed(&fixture("managed-locals.bas"));
    let listing = listing(&source);
    let procedure = between(&listing, "SHOWCOMMAND proc far", "SHOWCOMMAND endp");

    assert!(procedure.contains("mov bx, 1"));
    assert!(procedure.contains("lea ax, [bp-38]"));
    assert!(procedure.contains("lea ax, [bp-42]"));
}

/// SYS read its Game argument at BP-0Eh and later raised error 64 opening the map.
#[test]
fn test_runtime_frame_keeps_parameters_above_bp() {
    let source = parsed(&fixture("emission.bas"));
    let listing = listing(&source);
    let procedure = between(&listing, "ADDONE proc far", "ADDONE endp");

    assert!(procedure.contains("dword ptr [bp+6]"));
    assert!(!procedure.contains("dword ptr [bp-14]"));
}

/// LTRIM/RTRIM results were counted as local handles; raw BC emits BX=1, not 2.
#[test]
fn test_runtime_frame_counts_owned_string_descriptors_not_runtime_temporaries() {
    let source = parsed(&fixture("managed-temporaries.bas"));
    let listing = listing(&source);
    let procedure = between(&listing, "SHOWCOMMAND proc far", "SHOWCOMMAND endp");

    assert!(procedure.contains("mov bx, 1"));
}

/// Nibbles left three Center arguments live until loop i became 0x2020.
#[test]
fn test_source_call_releases_its_materialized_string_argument() {
    let source = parsed_as(&fixture("strtemp.bas"), "vbdos", "qb45");
    let calls = callees(&source.modules[0].functions[0]);

    let show = calls.iter().position(|one| one == "SHOW").expect("SHOW");
    assert_eq!(calls[show - 1..show + 2], ["B$SASS", "SHOW", "B$STDL"]);
}

/// Nibbles pushed four bytes per PrintScore field, then RETF 10 left SP corrupted.
#[test]
fn test_far_array_field_byref_uses_a_near_copy_in_copy_out_slot() {
    let source = parsed_as(&fixture("farbyref.bas"), "vbdos", "qb45");
    let module = &source.modules[0];
    let function = named(module, "WORK");
    let types: IndexMap<i64, &hir::Type> = module.types.iter().map(|one| (one.id, one)).collect();
    let values: IndexMap<i64, &hir::Type> = function.values.iter().map(|one| (one.id, types[&one.r#type])).collect();
    let instructions = instructions(function);
    let at = instructions
        .iter()
        .position(|one| one.op == hir::Op::Call && one.callee.as_deref() == Some("TOUCH"))
        .expect("TOUCH");
    let Operand::ValueRef(argument) = &instructions[at].operands[0] else {
        panic!("TOUCH's argument is not a value");
    };

    assert_eq!(values[&argument.value].name, "near*integer");
    assert!(instructions[at + 1..]
        .iter()
        .any(|one| one.op == hir::Op::Store && matches!(one.operands[0], Operand::IndirectPlace(_))));
}

/// Fresh SYS loaded argv() as a huge pointer and DIR$ raised BASIC error 64.
#[test]
fn test_fixed_string_array_descriptor_carries_a_near_data_offset() {
    let source = parsed(&fixture("string-array-element.bas"));
    let module = &source.modules[0];
    let function = named(module, "COPYFIRST");
    let types: IndexMap<i64, &hir::Type> = module.types.iter().map(|one| (one.id, one)).collect();
    let pointer_types: Vec<&hir::Type> = function
        .values
        .iter()
        .map(|value| types[&value.r#type])
        .filter(|one| one.kind == hir::TypeKind::Pointer)
        .collect();

    assert!(pointer_types.iter().any(|one| one.name == "near*string"));
    assert!(!pointer_types.iter().any(|one| one.name == "huge*string"));
    let calls = callees(function);
    assert!(calls.iter().any(|one| one == "B$ERS1"));
    assert!(!calls.iter().any(|one| one == "B$ERAS"));
    assert!(!object_bytes(&source, "string-array-element.bas").expect("emits").is_empty());
}

/// Fresh SCREEN exited after `ugl`: BASIC startup zeroed its BC_DATA array descriptor.
#[test]
fn test_module_static_numeric_array_has_a_relocated_basic_descriptor() {
    let source = parsed(&fixture("static-array-descriptor.bas"));
    let module = &source.modules[0];
    let main = named(module, "__main");
    let values = place(main, "VALUES");
    let descriptor = place(main, "VALUES$descriptor");
    let data = module.data.iter().find(|one| one.id == descriptor.symbol).expect("descriptor data");

    assert!(data.readonly);
    assert_eq!(descriptor.storage.value(), "static");
    assert_eq!(descriptor.offset, 0);
    let low = descriptor.offset as usize;
    assert_eq!(
        data_bytes(data, low, low + descriptor.extent.expect("extent") as usize),
        hex("00 00 00 00 00 00 00 00 01 40 00 00 04 00 04 00 00 00")
    );
    assert_eq!(
        relocations(data),
        [(0, values.symbol, values.offset, "far"), (10, values.symbol, values.offset, "near")]
    );

    let records = records(&source, "static-array-descriptor.bas");
    let by_name = by_name(&records);
    let (constant, size) = by_name["BC_CN"];
    assert_eq!(
        omf::segment_image(&records, constant, size),
        hex("06 00 00 00 00 00 00 00 01 40 06 00 04 00 04 00 00 00")
    );
    assert_eq!(
        fixup_rows(&records, constant),
        [
            row(0, 1, "segment", by_name["BC_DATA"].0),
            row(2, 2, "group", 1),
            row(10, 1, "segment", by_name["BC_DATA"].0),
        ]
    );
}

/// QB nbody recomputed ``current * 16`` after C and Nib had reduced it.
///
/// A proved array walk must carry the byte offset regardless of which source
/// frontend formed the MIR.  One hundred iterations keep this witness as a
/// loop rather than allowing the independent unroller to erase it.
#[test]
fn test_static_udt_array_loop_carries_a_byte_offset() {
    let directory = tempfile::TempDir::new().unwrap();
    let source = written(
        &directory,
        "stride.bas",
        b"defint a-z

type Vec2i
    x as long
    y as long
end type

type Body
    pos as Vec2i
    vel as Vec2i
end type

declare function walk () as long

dim shared answer as long
answer = walk()
end

function walk () as long static
    dim bodies(0 to 99) as Body
    dim current as integer

    for current = 0 to 99
        bodies(current).pos.x = bodies(current).vel.x
    next current
    walk = bodies(99).pos.x
end function
",
    );

    let program = parsed_as(&source, "vbdos", "vbdos");
    let listing = listing(&program);
    let procedure = between(&listing, "WALK proc far", "WALK endp");

    assert!(!shl_by_4(procedure));
    assert!(!imul_by_16(procedure));
    assert!(add_16(procedure));
    assert!(!cmp_frame_zero(procedure));
}

/// Q45A05 returned dimension 2 for LBOUND(a,1) because its descriptor was source-ordered.
#[test]
fn test_rank_two_descriptor_matches_qb_dimension_order_and_adjusted_offset() {
    let source = parsed_as(&root().join("frontends/qb/compat/qb45/q45a05.bas"), "qb45", "qb45");
    let module = &source.modules[0];
    let main = &module.functions[0];
    let values = place(main, "VALUES");
    let descriptor = module.data.iter().find(|one| one.name == "VALUES$descriptor").expect("descriptor");

    assert_eq!(data_bytes(descriptor, 8, 22), hex("02 40 00 00 02 00 03 00 01 00 02 00 01 00"));
    assert_eq!(
        relocations(descriptor),
        [(0, values.symbol, values.offset, "far"), (10, values.symbol, values.offset - 6, "near")]
    );
}

/// The static descriptor kept its records last dimension first under /R, and
/// biased +0Ah for them, but BC /R reverses the dimensions: record 0 holds the
/// first, 02 00 01 00, as its B$DDIM does.
#[test]
fn test_row_major_rank_two_descriptor_matches_bc_r() {
    let path = root().join("frontends/qb/compat/qb45/q45a05.bas");
    let source = qb_driver::parsed(&path, &qb_driver::Frontend { array_order: "row-major".into(), ..qb_driver::Frontend::new("qb45", "qb45") }, None)
        .expect("parses");
    let module = &source.modules[0];
    let values = place(&module.functions[0], "VALUES");
    let descriptor = module.data.iter().find(|one| one.name == "VALUES$descriptor").expect("descriptor");

    assert_eq!(data_bytes(descriptor, 8, 22), hex("02 40 00 00 02 00 02 00 01 00 03 00 01 00"));
    assert_eq!(
        relocations(descriptor),
        [(0, values.symbol, values.offset, "far"), (10, values.symbol, values.offset - 8, "near")]
    );
}

/// DYNARR wrote a(2).row, leaving a(1).row at zero after Touch a().
#[test]
fn test_static_array_formal_uses_a_lower_bound_adjusted_descriptor() {
    let source = parsed(&fixture("adjudt.bas"));
    let module = &source.modules[0];
    let main = &module.functions[0];
    let values = place(main, "A");
    let descriptor = module.data.iter().find(|one| one.name == "A$descriptor").expect("descriptor");

    // QB's AD_oAdjusted is data - lower*elementWidth.
    assert_eq!(
        relocations(descriptor),
        [(0, values.symbol, values.offset, "far"), (10, values.symbol, values.offset - 6, "near")]
    );
    let records = records(&source, "ADJUDT.BAS");
    let by_name = by_name(&records);
    let constants = by_name["BC_CN"].0;
    let data = by_name["BC_DATA"].0;
    let descriptor_fixups: Vec<_> = fixup_rows(&records, constants).into_iter().filter(|one| one.0 <= 10).collect();
    assert_eq!(descriptor_fixups, [row(0, 1, "segment", data), row(2, 2, "group", 1), row(10, 1, "segment", data)]);
}

/// MOD_TEX first passed a static AD to RDIM, then addressed its far allocation through DGROUP.
#[test]
fn test_dynamic_directive_makes_a_bounded_numeric_array_runtime_owned() {
    let source = parsed(&fixture("dynamic-bounded-array.bas"));
    let module = &source.modules[0];
    let main = named(module, "__main");
    let descriptor = place(main, "VALUES$descriptor");
    let calls = callees(main);

    assert_eq!(descriptor.storage.value(), "module");
    assert!(!module.data.iter().any(|one| one.readonly && one.name == "VALUES$descriptor"));
    assert_eq!(calls, ["B$DDIM", "B$RDIM"]);
    let listing = listing(&source);
    assert!(listing.contains("+2]") && listing.contains("+10]"));
    assert!(listing.contains("mov dword ptr es:[bx], 7"));
}

/// COM_TOKENIZE passed a huge-pointer element to SASS, which reported string-space corruption.
#[test]
fn test_dynamic_string_array_formal_uses_adjusted_near_descriptor_base() {
    let source = parsed(&fixture("string-array-parameter.bas"));
    let listing = listing(&source);
    let procedure = between(&listing, "APPENDONE proc far", "APPENDONE endp");

    assert!(procedure.contains("word ptr [bx+10]") || procedure.contains("word ptr [si+10]"));
    assert!(!procedure.contains("dword ptr [bx]") && !procedure.contains("dword ptr [si]"));
    assert!(procedure.contains("mov bx, 0"));
    let payloads: Vec<&hir::Place> =
        source.modules[0].functions[1].places.iter().filter(|one| one.name.ends_with("$payload")).collect();
    assert!(!payloads.is_empty() && payloads.iter().all(|one| one.address == hir::AddressKind::Far));
    let records = records(&source, "string-array-parameter.bas");
    let fsl = by_name(&records)["FSL_CONST"].0;
    assert!(omf::fixups(&records)
        .iter()
        .any(|fixup| fixup.seg == Some(1) && fixup.target == "segment" && fixup.index == fsl));
}

/// D_SURF applied a one-based array's lower bound twice and hung before its first frame.
#[test]
fn test_dynamic_numeric_array_formal_uses_split_adjusted_far_base() {
    let source = parsed(&fixture("numeric-array-parameter.bas"));
    let listing = listing(&source);
    let procedure = between(&listing, "SETFIRST proc far", "SETFIRST endp");

    assert!(procedure.contains("+2]") && procedure.contains("+10]"));
    assert!(!procedure.contains("dword ptr [bx]") && !procedure.contains("dword ptr [si]"));

    let function = named(&source.modules[0], "SETFIRST");
    assert!(!indirect_loads(function).iter().any(|one| one.offset == 16));
}

/// R_SET_FRUSTUM rebuilt the same split descriptor 38 times; one expression needs one stable base.
#[test]
fn test_numeric_array_descriptor_snapshot_is_reused_until_an_effectful_call() {
    let source = parsed(&fixture("numeric-array-base-reuse.bas"));
    let function = named(&source.modules[0], "COMBINE");
    let loads = indirect_loads(function);

    assert_eq!(loads.iter().filter(|one| one.offset == 2).count(), 2);
    assert_eq!(loads.iter().filter(|one| one.offset == 10).count(), 2);
    assert!(!loads.iter().any(|one| one.offset >= 14));
}

/// Qrender stopped at SYS_ERROR because COM_ARG returned descriptor bytes, not B$SCPF's AX pointer.
#[test]
fn test_string_function_copies_its_local_result_to_the_runtime_temporary_chain() {
    let source = parsed(&fixture("string-function-result.bas"));
    let module = &source.modules[0];
    let function = named(module, "SECONDITEM");
    let types: IndexMap<i64, &hir::Type> = module.types.iter().map(|one| (one.id, one)).collect();
    let result = types[&function.result_type];
    let calls = callees(function);

    assert_eq!(result.kind, hir::TypeKind::Pointer);
    assert_eq!(result.width, 2);
    assert_eq!(types[&result.element.expect("an element")].name, "string");
    assert_eq!(calls.last().map(String::as_str), Some("B$SCPF"));

    let listing = listing(&source);
    let procedure = between(&listing, "SECONDITEM proc far", "SECONDITEM endp");
    assert!(procedure.contains("call far ptr B$SCPF"));
    assert!(procedure.contains("call far ptr B$EXSA"));
}

/// D_SURF's LS_LCHAR called LEN then ASC; a direct literal temporary was consumed and ASC raised error 5.
#[test]
fn test_non_addressable_byref_string_argument_is_copied_to_an_owned_descriptor() {
    let source = parsed(&fixture("asc-literal.bas"));
    let calls = callees(&source.modules[0].functions[0]);
    let index = |name: &str| calls.iter().position(|one| one == name).unwrap_or_else(|| panic!("no {name}"));

    assert!(index("B$SASS") < index("FIRSTCODE"));
}

/// Fresh SYS passed COM_TOKENIZE backwards and its filled argv heap was corrupt.
#[test]
fn test_basic_procedure_arguments_use_pascal_left_to_right_push_order() {
    let source = parsed(&fixture("pascal-call-order.bas"));
    let call = &source.modules[0].functions[0].calls[0];

    assert_eq!(call.cleanup, hir::StackCleanup::Callee);
    assert_eq!(call.order, [0, 1]);
}

/// MAIN's OPEN raised error 52 when a near string reference named only the far payload.
#[test]
fn test_readonly_literals_use_the_measured_near_descriptor_and_far_payload() {
    let source = parsed(&fixture("readonly-data.bas"));
    let records = records(&source, "readonly-data.bas");
    let by_name = by_name(&records);

    assert_eq!(by_name["BC_DATA"].1, 10); // six-byte BASIC prefix plus the writable LONG
    let (descriptor, size) = by_name["BC_CN"];
    assert_eq!(size, 6);
    let (constant, size) = by_name["FSL_CONST"];
    assert_eq!(size, 8);
    assert_eq!(omf::combines(&records)[&constant], 0); // private FAR_DATA, outside DGROUP
    assert_eq!(
        fixup_rows(&records, descriptor),
        [
            row(0, 2, "segment", constant),   // selector word
            row(2, 1, "segment", constant),   // offset of the far string descriptor
            row(4, 1, "segment", descriptor), // selector-word address in DGROUP
        ]
    );
}

/// Every static array carried a descriptor in BC_CN whether code read it or
/// not: deedlines' 29 unread ones cost 522 bytes of DGROUP, and its string
/// space ran out ("Out of string space") where BC's build ran.
#[test]
fn test_a_descriptor_nothing_reads_takes_no_dgroup() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let constants = |text: &str| {
        let source = written(&directory, "T.BAS", text.as_bytes());
        segment(&records(&parsed_as(&source, "qb45", "qb45"), "T.BAS"), "BC_CN").1
    };
    assert_eq!(constants("DEFINT A-Z\r\nDIM a(10)\r\na(3) = 5\r\nPRINT a(3)\r\n"), 0);
    let whole = "DEFINT A-Z\r\nDECLARE SUB s (b())\r\nDIM a(10)\r\na(3) = 5\r\nCALL s(a())\r\nSUB s (b())\r\nPRINT b(3)\r\nEND SUB\r\n";
    assert_eq!(constants(whole), 18);
}

/// deedlines' `g% = 0` shared the label of `DIM SHARED g%(255)`: the scalar
/// sat inside the array's descriptor, and getpal wrote the palette over the
/// interrupt vectors.
#[test]
fn test_a_scalar_and_an_array_of_one_name_do_not_share_storage() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let source = written(&directory, "T.BAS", b"DIM SHARED g%(3)\r\ng% = 7\r\ng%(0) = 5\r\nPRINT g%, g%(0)\r\n");
    let records = records(&parsed_as(&source, "qb45", "qb45"), "T.BAS");
    let (code, size) = by_name(&records)["T_CODE"];
    let image = omf::segment_image(&records, code, size);
    let stored = |value: u8| {
        let at = image
            .windows(6)
            .position(|one| one[..2] == [0xC7, 0x06] && one[4..] == [value, 0])
            .unwrap_or_else(|| panic!("no store of {value}"));
        &image[at + 2..at + 4]
    };
    assert_ne!(stored(7), stored(5));
}

/// PRINT "A" emitted VBDOS's far bridge and printed garbage under QB 4.5 and PDS 7.1.
#[test]
fn test_qb_and_pds_literals_use_their_measured_near_descriptor() {
    for (dialect, runtime) in [("qb45", "qb45"), ("pds71", "pds71")] {
        let source = parsed_as(&fixture("readonly-data.bas"), dialect, runtime);
        let records = records(&source, "readonly-data.bas");
        let by_name = by_name(&records);

        let (descriptor, size) = by_name["BC_CN"];
        assert_eq!(size, 6);
        assert_eq!(omf::segment_image(&records, descriptor, size), hex("01 00 04 00 41 00"));
        assert!(
            !by_name.contains_key("FSL_CONST") && !by_name.contains_key("FDATA") && !by_name.contains_key("QB_LINK")
        );
        assert_eq!(
            names(&records)[1..],
            [
                "BR_DATA", "BR_SKYS", "COMMON", "BC_DATA", "NMALLOC", "ENMALLOC", "BC_FT", "BC_CN", "BC_DS", "BC_SAB",
                "BC_SA",
            ]
        );
        assert_eq!(fixup_rows(&records, descriptor), [row(2, 1, "segment", descriptor)]);
    }
}

/// main.bas needs its handler address registered, not an ordinary CFG edge.
#[test]
fn test_on_error_emits_a_relocated_runtime_registration() {
    let source = parsed(&fixture("on-error-emission.bas"));
    let records = records(&source, "on-error-emission.bas");
    let externals = omf::externals(&records);
    let code = code(&records);
    let registration = find(&code, &hex("b8"), 48);
    // O_ENT is fixed at 48.
    assert_eq!(registration, Some(48)); // mov ax, relocated handler offset
    let registration = registration.unwrap();
    assert_eq!(code[registration + 3..registration + 6], hex("0e 50 9a"));
    let fixups = omf::fixups(&records);
    assert!(fixups
        .iter()
        .any(|one| one.seg == Some(1) && one.target == "external" && externals[one.index as usize] == "B$OEGA"));
    assert!(fixups.iter().any(|one| one.seg == Some(1)
        && one.target == "segment"
        && one.index == 1
        && one.offset == registration as i64 + 1));
}

/// Gorillas ignored ON ERROR GOTO 0, sent a shot error to PaletteError, and resumed corrupt state.
#[test]
fn test_on_error_registrations_follow_source_order() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "ERRSTATE.BAS",
        b"on error goto first\r\n\
          print \"armed first\"\r\n\
          on error goto second\r\n\
          print \"armed second\"\r\n\
          on error goto 0\r\n\
          error 11\r\n\
          end\r\n\
          first:\r\nresume next\r\n\
          second:\r\nresume next\r\n",
    );
    let source = parsed_as(&basic, "qb45", "qb45");
    let records = records(&source, "ERRSTATE.BAS");
    let externals = omf::externals(&records);
    let calls: Vec<omf::Fixup> = omf::fixups(&records)
        .into_iter()
        .filter(|one| one.seg == Some(1) && one.target == "external" && externals[one.index as usize] == "B$OEGA")
        .collect();

    assert_eq!(calls.len(), 3);
    let code = code(&records);
    let last = calls.last().unwrap().offset as usize;
    assert_eq!(code[last - 6..last], hex("b8 00 00 50 50 9a"));
}

/// Q45R35's post-ERROR statement vanished, leaving RESUME NEXT with no target.
#[test]
fn test_resume_next_retains_runtime_statement_entries() {
    let source = parsed_as(&root().join("frontends/qb/compat/qb45/q45r35.bas"), "qb45", "qb45");
    let main = &source.modules[0].functions[0];
    assert!(!main.external_entries.is_empty());

    let listing = listing(&source);
    let statement_table = between(&listing, "$QB$STAT proc near", "$QB$STAT endp");
    assert!(statement_table.contains("db 064h,000h"));
    assert!(statement_table.matches("dw offset").count() >= 2);

    let records = records(&source, "Q45R35.BAS");
    let code = code(&records);
    let header_fixup = omf::fixups(&records)
        .into_iter()
        .find(|one| one.seg == Some(1) && one.offset == 10)
        .expect("the header fixup");
    let statement_at = (i64::from(u16::from_le_bytes([code[10], code[11]])) + header_fixup.disp) as usize;
    assert_eq!(code[statement_at - 3..statement_at], hex("55 8b ec"));
    assert_eq!(u16::from_le_bytes([code[statement_at + 2], code[statement_at + 3]]), 100);
}

/// Q45ER52 optimization made RESUME entries use values defined only from main.
///
/// The resulting temporary-root verification failed at blocks 0x1f, 0x29,
/// and 0x36 instead of emitting an object for the bounds-error test.
#[test]
fn test_resume_statement_entries_are_optimizer_roots() {
    let source = parsed_as(&root().join("frontends/qb/compat/qb45/q45er52.bas"), "qb45", "qb45");
    let function = &source.modules[0].functions[0];
    let optimized = qb_compile::optimized(&source, function, &lowered(&source)[0], &O2()).expect("optimizes");
    let mut entries: Vec<i64> = Vec::new();
    for entry in function.external_entries.iter().copied().chain(function.error_handler) {
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }
    let (checked, _root) = qb_compile::_machine_side_entry(&optimized.body, &entries).expect("roots");

    assert!(mir::verify(&checked).is_empty());
    assert!(!object_bytes(&source, "Q45ER52.BAS").expect("emits").is_empty());
}

/// QGL MAIN's FOR bound was SSA-only, so RESUME side entries bypassed its definition.
#[test]
fn test_for_bounds_survive_resume_statement_side_entries() {
    let directory = tempfile::TempDir::new().unwrap();
    let basic = written(
        &directory,
        "FORRES.BAS",
        b"on error goto handler\r\n\
          dim i as integer, limit as integer\r\n\
          limit = 2\r\n\
          for i = 0 to limit\r\n\
          print i\r\n\
          next i\r\n\
          end\r\n\
          handler:\r\n\
          resume next\r\n",
    );
    let source = parsed_as(&basic, "vbdos", "vbdos");
    assert!(lowered(&source).iter().all(|function| mir::verify(&function.body).is_empty()));
    assert!(!object_bytes(&source, "FORRES.BAS").expect("emits").is_empty());
}

/// A zero-based dynamic array's element names the descriptor offset its
/// frontend proved is the array's first byte; the fact reaches MIR.
#[test]
fn test_a_zero_based_element_reaches_mir_with_its_origin() {
    let directory = tempfile::tempdir().expect("creates a directory");
    let source = written(&directory, "T.BAS", b"DEFINT A-Z\r\nSUB t\r\nDIM a(9)\r\nx = a(3)\r\nEND SUB\r\n");
    let program = parsed(&source);
    let lowered = lowered(&program);
    let body = lowered
        .iter()
        .find(|one| one.name.rsplit('.').next().is_some_and(|name| name.eq_ignore_ascii_case("T")))
        .expect("T is lowered");
    let origins: Vec<mir::Value> = ops(body)
        .iter()
        .flat_map(|op| op.loads.iter().chain(&op.stores))
        .filter_map(|reference| reference.origin)
        .collect();
    assert_eq!(origins.len(), 1, "{origins:?}");
    let defined = ops(body).iter().any(|op| op.defines.contains(&origins[0]));
    assert!(defined, "the origin names a value the body defines");
}

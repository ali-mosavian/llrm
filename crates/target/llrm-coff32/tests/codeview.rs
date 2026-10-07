//! `.debug$S` and `.debug$T`: what LLVM's CodeView reader makes of the C13 an i386 object carries.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use llrm_object::debug::{Enumerator, Field, File, Function, Info, Kind, Line, Location, Range, Reach, Register, Scalar, Type, Variable};
use llrm_object::{Arch, Binding, Definition, Object, Role, Section, Symbol};

fn llvm(tool: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    dirs.push("/usr/lib/llvm-20/bin".into());
    dirs.iter().map(|dir| dir.join(tool)).find(|path| path.exists())
}

fn registers() -> Vec<Register> {
    [("ebp", 22), ("eax", 17), ("ebx", 20)].map(|(name, number)| Register { name: name.into(), bits: 32, dwarf: None, codeview: Some(number) }).to_vec()
}

fn public(name: &str, section: usize, offset: usize) -> Symbol {
    Symbol { name: name.into(), binding: Binding::Public, definition: Definition::Defined { section, offset }, group: None }
}

fn variable(name: &str, r#type: usize, kind: Kind, location: Location) -> Variable {
    Variable { name: name.into(), r#type, kind, location }
}

/// `int f(int x, struct S *p) { int y; ... }` in 16 bytes of code on lines 3 to 5, a global `g`, a
/// struct, an array, an enum and a recursive struct.
fn object() -> Object {
    let text = Section { name: "_TEXT".into(), role: Role::Text, near: true, align: 16, image: vec![0x90; 16], spans: vec![[0, 16]], relocs: Vec::new() };
    let data = Section { name: "_DATA".into(), role: Role::Data, near: true, align: 4, image: vec![0; 8], spans: vec![[0, 8]], relocs: Vec::new() };
    let int = Type::Scalar(Scalar::Int { bytes: 4, signed: true });
    let types = vec![
        int,                                                                                                  // 0
        Type::Scalar(Scalar::Char),                                                                            // 1
        Type::Struct { name: "S".into(), bytes: 8, fields: vec![Field { name: "a".into(), r#type: 0, offset: 0, bits: None }, Field { name: "b".into(), r#type: 1, offset: 4, bits: None }] }, // 2
        Type::Pointer { target: 2, bytes: 4, reach: Reach::Near },                                             // 3
        Type::Procedure { result: Some(0), parameters: vec![0, 3], convention: None },                         // 4
        Type::Array { element: 0, bytes: Some(40) },                                                           // 5
        Type::Enum { name: "E".into(), underlying: 0, enumerators: vec![Enumerator { name: "A".into(), value: 0 }, Enumerator { name: "B".into(), value: -3 }] }, // 6
        Type::Struct { name: "N".into(), bytes: 4, fields: vec![Field { name: "next".into(), r#type: 8, offset: 0, bits: None }] }, // 7
        Type::Pointer { target: 7, bytes: 4, reach: Reach::Near },                                             // 8
    ];
    let function = Function {
        name: "f".into(),
        symbol: 0,
        r#type: 4,
        ranges: vec![Range { section: 0, offset: 0, length: 16 }],
        body: Some((3, 14)),
        far: false,
        module: false,
        variables: vec![
            variable("x", 0, Kind::Parameter, Location::Frame { disp: 8 }),
            variable("p", 3, Kind::Parameter, Location::Register("eax".into())),
            variable("y", 0, Kind::Local, Location::Frame { disp: -4 }),
            variable("arr", 5, Kind::Local, Location::Frame { disp: -44 }),
            variable("e", 6, Kind::Local, Location::Frame { disp: -48 }),
            variable("n", 7, Kind::Local, Location::Frame { disp: -52 }),
        ],
        blocks: vec![llrm_object::debug::Block { ranges: vec![Range { section: 0, offset: 4, length: 8 }], variables: vec![variable("t", 0, Kind::Local, Location::Frame { disp: -56 })], blocks: Vec::new() }],
    };
    let info = Info {
        frame_register: "ebp".into(),
        registers: registers(),
        files: vec![File { name: "f.c".into(), checksum: None }],
        code: vec![Range { section: 0, offset: 0, length: 16 }],
        types,
        functions: vec![function],
        globals: vec![variable("g", 0, Kind::Local, Location::Static { symbol: 1, disp: 0 })],
        lines: vec![Line { section: 0, offset: 0, file: 0, line: 3, column: 0 }, Line { section: 0, offset: 4, file: 0, line: 4, column: 0 }, Line { section: 0, offset: 12, file: 0, line: 5, column: 0 }],
        ..Info::default()
    };
    Object { name: "f.c".into(), arch: Arch::I386, sections: vec![text, data], symbols: vec![public("_f", 0, 0), public("_g", 1, 0)], omf_groups: Vec::new(), debug: Some(info) }
}

fn dump(tool: &str, arguments: &[&str], bytes: &[u8]) -> Option<String> {
    let tool = llvm(tool)?;
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("f.obj");
    std::fs::write(&path, bytes).unwrap();
    let said = Command::new(tool).args(arguments).arg(&path).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
    assert!(said.status.success() && !text.contains("warning") && !text.contains("error"), "{text}");
    Some(text)
}

fn written() -> Vec<u8> {
    llrm_coff32::write(&object()).unwrap()
}

/// Every record of both sections reads, and says what the model said: the function and its
/// parameters and locals with their frame cells and register, the block's own range, the types
/// (a struct reached from its own field is a forward reference first), the global and the lines.
/// A `contains("S_LOCAL")` passed with every offset wrong; these are the values.
#[test]
fn llvm_reads_the_functions_variables_types_and_lines() {
    let Some(text) = dump("llvm-readobj", &["--codeview"], &written()) else { return eprintln!("skipped: no llvm-readobj") };
    for wanted in [
        "FunctionType: int (int, S*) (0x1004)",
        "BasePointerOffset: 8",
        "BasePointerOffset: -44",
        "BaseRegister: EBP (0x16)",
        "Register: EAX (0x11)",
        "CodeOffset: _f+0x4",
        "OffsetStart: _f+0x4",
        "Type: S* (0x1002)",
        "CallingConvention: NearC (0x0)",
        "SizeOf: 40",
        "EnumValue: -3",
        "ForwardReference (0x80)",
        "DataOffset: _g+0x0",
        "LineNumberStart: 5",
        "+0xC [",
        "IsStatement: Yes",
        "DbgStart: 0x3",
        "DbgEnd: 0xE",
        "CodeSize: 0x10",
    ] {
        assert!(text.contains(wanted), "no {wanted}:\n{text}");
    }
    // Two scopes open, two close: the function's and the block's.
    assert_eq!(text.matches("Kind: S_END (0x6)").count(), 2, "{text}");
    assert_eq!(text.matches("IsParameter (0x1)").count(), 2, "{text}");
}

/// lld-link links it with /debug and the PDB holds the function, its two parameters, its locals'
/// types, the block and the three lines at the right addresses.
#[test]
fn lld_link_makes_a_pdb_with_the_functions_parameters_types_and_lines() {
    let (Some(link), Some(pdbutil)) = (llvm("lld-link"), llvm("llvm-pdbutil")) else { return eprintln!("skipped: no lld-link or llvm-pdbutil") };
    let scratch = tempfile::tempdir().unwrap();
    let (obj, exe, pdb) = (scratch.path().join("f.obj"), scratch.path().join("f.exe"), scratch.path().join("f.pdb"));
    std::fs::write(&obj, written()).unwrap();
    let linked = Command::new(link).args(["/machine:x86", "/subsystem:console", "/entry:f", "/nodefaultlib", "/debug"]).arg(format!("/pdb:{}", pdb.display())).arg(format!("/out:{}", exe.display())).arg(&obj).output().unwrap();
    assert!(linked.status.success(), "{}{}", String::from_utf8_lossy(&linked.stdout), String::from_utf8_lossy(&linked.stderr));
    let said = Command::new(pdbutil).args(["dump", "-l", "--symbols", "--types", "--globals"]).arg(&pdb).output().unwrap();
    let text = String::from_utf8_lossy(&said.stdout);
    for wanted in [
        "S_GPROC32 [size = 44] `f`",
        "type = `0x1004 (int (int, S*))`",
        "code size = 16",
        "type=0x0074 (int), flags = param",
        "type=0x1002 (S*), flags = param",
        "register = EBP, offset = -44",
        "register = EAX",
        "S_BLOCK32",
        "code size = 8, addr = 0001:0004",
        "LF_STRUCTURE [size = 24] `S`",
        "sizeof 8",
        "calling conv = cdecl",
        "S_GDATA32 [size = 16] `g`",
        "0001:00000000-00000010, line/addr entries = 3",
        "5 0000000C",
    ] {
        assert!(text.contains(wanted), "no {wanted}:\n{text}");
    }
}

fn function(made: &mut Object) -> &mut Function {
    &mut made.debug.as_mut().unwrap().functions[0]
}

fn refusal(change: impl FnOnce(&mut Object)) -> String {
    let mut made = object();
    change(&mut made);
    llrm_coff32::write(&made).unwrap_err().0
}

/// What C13 as written cannot say is refused by name, never written as something near: a register
/// the target gives no CodeView number, a far function, BASIC's types, a function in two pieces.
#[test]
fn what_the_writer_cannot_say_is_refused_by_name() {
    assert!(refusal(|made| function(made).variables[1].location = Location::Register("zmm0".into())).contains("register zmm0 is not in the target's register file"));
    let unnumbered = |made: &mut Object| made.debug.as_mut().unwrap().registers.push(Register { name: "st0".into(), bits: 80, dwarf: Some(11), codeview: None });
    assert!(refusal(|made| {
        unnumbered(made);
        function(made).variables[1].location = Location::Register("st0".into());
    })
    .contains("register st0 has no CodeView number"));
    assert!(refusal(|made| made.debug.as_mut().unwrap().frame_register = "esp".into()).contains("register esp is not in the target's register file"));
    assert!(refusal(|made| function(made).far = true).contains("f is a far function"));
    assert!(refusal(|made| made.debug.as_mut().unwrap().types.push(Type::Scalar(Scalar::Currency))).contains("no primitive type"));
    assert!(refusal(|made| made.debug.as_mut().unwrap().types.push(Type::FixedString(4))).contains("STRING * n"));
    assert!(refusal(|made| function(made).ranges.push(Range { section: 0, offset: 0, length: 1 })).contains("f is in 2 ranges"));
    assert!(refusal(|made| made.debug.as_mut().unwrap().types[3] = Type::Pointer { target: 2, bytes: 4, reach: Reach::Far }).contains("far or huge"));
}

/// A range's length is 16 bits: a variable live over 130000 bytes is three defranges, each
/// within 0xF000, which add up to the function. One record with a truncated length gave a debugger
/// a variable that ended at byte 0x1F000 % 0x10000.
#[test]
fn a_variable_over_a_long_function_is_written_in_pieces() {
    let mut made = object();
    made.sections[0].image = vec![0x90; 130_000];
    made.sections[0].spans = vec![[0, 130_000]];
    let info = made.debug.as_mut().unwrap();
    info.code[0].length = 130_000;
    info.functions[0].ranges[0].length = 130_000;
    info.functions[0].body = Some((3, 129_990));
    info.functions[0].blocks.clear();
    info.lines.clear();
    let bytes = llrm_coff32::write(&made).unwrap();
    let text = dump("llvm-readobj", &["--codeview"], &bytes).unwrap();
    let lengths: Vec<usize> = text.lines().filter_map(|one| one.trim().strip_prefix("Range: 0x")).map(|one| usize::from_str_radix(one, 16).unwrap()).collect();
    // Seven variables of one piece each, but `x` is three.
    assert!(lengths.iter().all(|&one| one <= 0xF000), "{lengths:?}");
    assert_eq!(lengths.iter().sum::<usize>(), 130_000 * 6, "{lengths:?}");
}

/// A record's length is 16 bits, a line's 31 and a column's 16: what overflows is refused, not
/// written with a wrapped length that makes every later record unreadable. A struct of 5000 fields
/// is a field list over 64 KiB.
#[test]
fn what_overflows_a_field_is_refused_not_wrapped() {
    let huge = |made: &mut Object| {
        let fields = (0..5000).map(|at| Field { name: format!("field_number_{at}"), r#type: 0, offset: at * 4, bits: None }).collect();
        made.debug.as_mut().unwrap().types[2] = Type::Struct { name: "S".into(), bytes: 20_000, fields };
    };
    assert!(refusal(huge).contains("16-bit length"));
    assert!(refusal(|made| made.debug.as_mut().unwrap().lines[0].column = 70_000).contains("does not fit a line entry"));
    assert!(refusal(|made| made.debug.as_mut().unwrap().lines[0].line = 0x8000_0000).contains("does not fit a line entry"));
}

/// A column anywhere turns the line table's columns on for the range.
#[test]
fn a_column_is_written_with_its_line() {
    let mut made = object();
    made.debug.as_mut().unwrap().lines[1].column = 9;
    let text = dump("llvm-readobj", &["--codeview"], &llrm_coff32::write(&made).unwrap()).unwrap();
    assert!(text.contains("Flags: 0x1") && text.contains("ColStart: 9"), "{text}");
}

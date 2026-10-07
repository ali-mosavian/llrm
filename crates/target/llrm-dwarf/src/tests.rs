use llrm_object::debug::{Function, Info, Kind, Line, Location, Range, Reach, Register, Scalar, Type, Variable};
use llrm_object::{Arch, Binding, Object, Section, Symbol, Target};

use super::*;

fn int() -> Type {
    Type::Scalar(Scalar::Int { bytes: 4, signed: true })
}

/// `f(x) { y }` in 16 bytes of code with the body at 4..12, x and y in the frame, lines 3 and 4.
fn object(variables: Vec<Variable>, mut types: Vec<Type>, format: Format) -> Object {
    types.push(Type::Procedure { result: Some(0), parameters: vec![0], convention: None });
    let procedure = types.len() - 1;
    let text = Section { name: "_TEXT".into(), role: Role::Text, near: true, align: 1, image: vec![0x90; 16], spans: vec![[0, 16]], relocs: Vec::new() };
    let function = Function {
        name: "f".into(),
        symbol: 0,
        r#type: procedure,
        ranges: vec![Range { section: 0, offset: 0, length: 16 }],
        body: Some((4, 12)),
        far: false,
        module: false,
        variables,
        blocks: Vec::new(),
    };
    let register = |name: &str, dwarf| Register { name: name.into(), bits: 32, dwarf, codeview: None };
    let info = Info {
        format,
        language: llrm_object::debug::Language::C,
        frame_register: "ebp".into(),
        registers: vec![register("ebp", Some(5)), register("eax", Some(0)), register("ah", None)],
        code: vec![Range { section: 0, offset: 0, length: 16 }],
        files: vec![llrm_object::debug::File { name: "f.c".into(), checksum: None }],
        types,
        functions: vec![function],
        lines: vec![Line { section: 0, offset: 0, file: 0, line: 3, column: 0 }, Line { section: 0, offset: 8, file: 0, line: 4, column: 0 }],
        ..Info::default()
    };
    Object {
        name: "f.c".into(),
        arch: Arch::I386,
        sections: vec![text],
        symbols: vec![Symbol { name: "_f".into(), binding: Binding::Public, definition: Definition::Defined { section: 0, offset: 0 }, group: None }],
        omf_groups: Vec::new(),
        debug: Some(info),
    }
}

fn frame(name: &str, disp: i64) -> Variable {
    Variable { name: name.into(), r#type: 0, kind: Kind::Local, location: Location::Frame { disp } }
}

fn written(variables: Vec<Variable>, types: Vec<Type>, format: Format) -> Result<Object, Unsupported> {
    let made = object(variables, types, format);
    expanded(&made, made.debug.as_ref().unwrap())
}

fn named<'a>(made: &'a Object, name: &str) -> (usize, &'a Section) {
    made.sections.iter().enumerate().find(|(_, one)| one.name == name).unwrap_or_else(|| panic!("no {name}"))
}

/// Known vectors of the DWARF specification (figure 22 and 23's examples).
#[test]
fn leb128_is_the_specifications() {
    assert_eq!(uleb(0), [0]);
    assert_eq!(uleb(127), [0x7F]);
    assert_eq!(uleb(128), [0x80, 0x01]);
    assert_eq!(uleb(624485), [0xE5, 0x8E, 0x26]);
    assert_eq!(sleb(2), [2]);
    assert_eq!(sleb(-1), [0x7F]);
    assert_eq!(sleb(-127), [0x81, 0x7F]);
    assert_eq!(sleb(-128), [0x80, 0x7F]);
    assert_eq!(sleb(-123456), [0xC0, 0xBB, 0x78]);
    assert_eq!(sleb(63), [0x3F]);
    assert_eq!(sleb(64), [0xC0, 0x00]);
}

/// A unit's length counts what follows it, its abbreviation offset is a reference to the
/// abbreviation section (the linker places it, so one object's offset is not another's), and the
/// header is the version's: 5 puts the unit type and the address size before the offset.
#[test]
fn a_unit_header_says_its_length_its_version_and_where_its_abbreviations_are() {
    for (version, offset_at, size_at) in [(5u16, 8usize, 7usize), (4, 6, 10)] {
        let made = written(vec![frame("x", 8)], vec![int()], Format::Dwarf { version }).unwrap();
        let (abbrev, _) = named(&made, ".debug_abbrev");
        let (_, info) = named(&made, ".debug_info");
        assert_eq!(u32::from_le_bytes(info.image[..4].try_into().unwrap()) as usize, info.image.len() - 4, "version {version}");
        assert_eq!(u16::from_le_bytes([info.image[4], info.image[5]]), version);
        assert_eq!(info.image[size_at], 4, "an i386 address");
        let reloc = info.relocs.iter().find(|one| one.at == offset_at).unwrap_or_else(|| panic!("no relocation at the abbreviation offset, version {version}"));
        assert_eq!((reloc.target, reloc.addend), (Target::Section(abbrev), 0));
    }
}

/// Each string, each address and each reference between sections is a relocation, not a number
/// that holds only for the first object a linker places.
#[test]
fn addresses_and_cross_section_offsets_are_relocations() {
    let made = written(vec![frame("x", 8)], vec![int()], Format::Default).unwrap();
    let (_, info) = named(&made, ".debug_info");
    let to_symbol = info.relocs.iter().filter(|one| matches!(one.target, Target::Symbol(0))).count();
    let to_section = info.relocs.iter().filter(|one| matches!(one.target, Target::Section(_))).count();
    // The unit's low_pc and the function's; the abbreviations, the line table and each string.
    assert_eq!(to_symbol, 2, "{:?}", info.relocs);
    assert!(to_section >= 5, "{:?}", info.relocs);
    let (_, line) = named(&made, ".debug_line");
    assert!(line.relocs.iter().any(|one| matches!(one.target, Target::Symbol(0))), "the sequence's address");
}

/// A function's body start is a row marked prologue_end, and its end epilogue_begin, even where
/// no source line begins there: gdb stops a breakpoint on a function after the prologue by it.
#[test]
fn a_bodys_start_and_end_are_marked_in_the_line_table() {
    let made = written(vec![frame("x", 8)], vec![int()], Format::Default).unwrap();
    let (_, line) = named(&made, ".debug_line");
    // After the header, the program: set_address, then rows. DW_LNS_set_prologue_end is 10, epilogue_begin 11.
    let program = &line.image[line.image.len() - 40..];
    assert!(program.contains(&10) && program.contains(&11), "{program:?}");
}

/// A far pointer has no DWARF type: it is refused where a variable uses it, and only there.
#[test]
fn an_unwritable_type_is_refused_where_it_is_used_and_nowhere_else() {
    let far = Type::Pointer { target: 0, bytes: 6, reach: Reach::Far };
    let mut used = frame("x", 8);
    used.r#type = 1;
    let why = written(vec![used], vec![int(), far.clone()], Format::Default).err().expect("refused").0;
    assert!(why.contains("far or huge pointer"), "{why}");
    // Nothing uses it: the unit is written without it.
    assert!(written(vec![frame("x", 8)], vec![int(), far], Format::Default).is_ok());
}

/// BASIC's array (its bounds are a descriptor's) and a register with no DWARF number are refused
/// by name, not written as something else.
#[test]
fn what_dwarf_cannot_say_is_refused_by_name() {
    let array = Type::Array { element: 0, bytes: None };
    let mut basic = frame("a", 8);
    basic.r#type = 1;
    assert!(written(vec![basic], vec![int(), array], Format::Default).unwrap_err().0.contains("BASIC array"));
    let high = Variable { name: "h".into(), r#type: 0, kind: Kind::Local, location: Location::Register("ah".into()) };
    assert!(written(vec![high], vec![int()], Format::Default).unwrap_err().0.contains("register ah has no DWARF number"));
    // A list of one that holds neither a frame cell nor a register (a static, say) has no expression here.
    let range = Range { section: 0, offset: 0, length: 4 };
    let moved = Variable { name: "m".into(), r#type: 0, kind: Kind::Local, location: Location::List(vec![(range, Location::Static { symbol: 0, disp: 0 })]) };
    assert!(written(vec![moved], vec![int()], Format::Default).unwrap_err().0.contains("holds frame cells and registers"));
}

/// A format this writer does not write is refused with which: an object cannot carry the
/// information of another format.
#[test]
fn a_format_that_is_not_dwarf_is_refused() {
    assert!(written(Vec::new(), vec![int()], Format::CodeView).unwrap_err().0.contains("CodeView"));
    assert!(written(Vec::new(), vec![int()], Format::TurboDebugger).unwrap_err().0.contains("Turbo Debugger"));
    assert!(written(Vec::new(), vec![int()], Format::Dwarf { version: 3 }).unwrap_err().0.contains("version 3"));
}

/// A register is DW_OP_reg0 + its number, and a number past 31 is DW_OP_regx.
#[test]
fn a_registers_location_is_its_dwarf_number() {
    let made = written(vec![Variable { name: "r".into(), r#type: 0, kind: Kind::Local, location: Location::Register("eax".into()) }], vec![int()], Format::Default).unwrap();
    let (_, info) = named(&made, ".debug_info");
    assert!(info.image.windows(2).any(|pair| pair == [1, 0x50]), "a one-byte expression, DW_OP_reg0");
}

/// A test that cannot run says so on stderr, and fails where `LLRM_REQUIRE_DWARF` is set, so a gate that
/// has the tools cannot pass by skipping.
fn skipped(reason: &str) {
    // Written to the stderr itself, which the harness does not capture: seen when the test passes.
    let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("SKIPPED: {reason}\n").as_bytes());
    assert!(std::env::var_os("LLRM_REQUIRE_DWARF").is_none(), "LLRM_REQUIRE_DWARF is set, and: {reason}");
}

fn dwarfdump() -> Option<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    dirs.push("/usr/lib/llvm-20/bin".into());
    dirs.iter().flat_map(|dir| [dir.join("llvm-dwarfdump"), dir.join("llvm-dwarfdump-20")]).find(|path| path.exists())
}

/// llvm-dwarfdump verifies the unit in an ELF32, an ELF64 and a Mach-O object, whose addresses are 4, 8 and 8
/// bytes, for DWARF 4 and 5, with a struct, an array, a pointer, a static and a register variable.
#[test]
fn llvm_dwarfdump_verifies_the_unit_in_either_class_and_version() {
    let Some(dump) = dwarfdump() else {
        skipped("llvm-dwarfdump is not installed");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    // 32 and 64: ELF32 and ELF64; 0: Mach-O, which is 64-bit.
    for (arch, bits) in [(Arch::I386, 32), (Arch::X8664, 64), (Arch::X8664, 0)] {
        for version in [4u16, 5] {
            let types = vec![
                int(),
                Type::Struct { name: "pt".into(), bytes: 8, fields: vec![llrm_object::debug::Field { name: "x".into(), r#type: 0, offset: 0, bits: None }, llrm_object::debug::Field { name: "f".into(), r#type: 0, offset: 4, bits: Some((3, 5)) }] },
                Type::Array { element: 1, bytes: Some(32) },
                Type::Pointer { target: 1, bytes: 4, reach: Reach::Near },
            ];
            let mut made = object(
                vec![
                    frame("x", 8),
                    Variable { name: "s".into(), r#type: 2, kind: Kind::Local, location: Location::Frame { disp: -40 } },
                    Variable { name: "p".into(), r#type: 3, kind: Kind::Parameter, location: Location::Register("eax".into()) },
                    Variable { name: "g".into(), r#type: 0, kind: Kind::Local, location: Location::Static { symbol: 0, disp: 4 } },
                    // In eax over the first seven bytes, then in its frame cell: a list of two.
                    Variable {
                        name: "q".into(),
                        r#type: 0,
                        kind: Kind::Parameter,
                        location: Location::List(vec![(Range { section: 0, offset: 0, length: 7 }, Location::Register("eax".into())), (Range { section: 0, offset: 7, length: 9 }, Location::Frame { disp: -4 })]),
                    },
                ],
                types,
                Format::Dwarf { version },
            );
            made.arch = arch;
            let bytes = match bits {
                32 => llrm_elf32::write(&made),
                64 => llrm_elf64::write(&made),
                _ => llrm_macho::write(&made),
            }
            .unwrap();
            let path = scratch.path().join("x.o");
            std::fs::write(&path, bytes).unwrap();
            let said = std::process::Command::new(&dump).arg("--verify").arg(&path).output().unwrap();
            let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
            assert!(said.status.success() && text.trim_end().ends_with("No errors.") && !text.contains("warning"), "{bits} DWARF {version}:\n{text}");
            let shown = std::process::Command::new(&dump).arg("--debug-info").arg(&path).output().unwrap();
            let shown = String::from_utf8_lossy(&shown.stdout);
            for expected in ["DW_AT_name\t(\"pt\")", "DW_AT_bit_size", "DW_AT_upper_bound\t(3)", "DW_OP_reg0", "DW_OP_addr"] {
                assert!(shown.contains(expected), "{bits} DWARF {version}: no {expected} in\n{shown}");
            }
            // The list: register 0 over [0, 7) and fbreg -4 over [7, 16), in the section of lists its version has.
            let lists = std::process::Command::new(&dump).arg(if version >= 5 { "--debug-loclists" } else { "--debug-loc" }).arg(&path).output().unwrap();
            let lists = String::from_utf8_lossy(&lists.stdout);
            assert!(lists.contains("DW_OP_reg0 ") && lists.contains("DW_OP_fbreg -4"), "{bits} DWARF {version}: the list in\n{lists}");
        }
    }
}

/// A variable the optimiser removed (a list of no entry) is a DIE with a name and a type and no location, which
/// a debugger shows as optimized out; one with a location has it.
#[test]
fn a_removed_variable_has_no_location_and_the_others_have_theirs() {
    let Some(dump) = dwarfdump() else {
        skipped("llvm-dwarfdump is not installed");
        return;
    };
    let removed = Variable { name: "gone".into(), r#type: 0, kind: Kind::Parameter, location: Location::List(Vec::new()) };
    let made = written(vec![removed, frame("x", 8)], vec![int()], Format::Default).unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("x.o");
    std::fs::write(&path, llrm_elf32::write(&made).unwrap()).unwrap();
    let said = std::process::Command::new(&dump).arg("--verify").arg(&path).output().unwrap();
    assert!(String::from_utf8_lossy(&said.stdout).trim_end().ends_with("No errors."), "{}", String::from_utf8_lossy(&said.stdout));
    let shown = std::process::Command::new(&dump).arg("--debug-info").arg(&path).output().unwrap();
    let shown = String::from_utf8_lossy(&shown.stdout).into_owned();
    let parameter = &shown[shown.find("DW_AT_name\t(\"gone\")").expect("the parameter")..];
    let parameter = &parameter[..parameter.find("DW_TAG").unwrap_or(parameter.len())];
    assert!(!parameter.contains("DW_AT_location"), "{parameter}");
    let local = &shown[shown.find("DW_AT_name\t(\"x\")").expect("the local")..];
    assert!(local.contains("DW_OP_fbreg +8"), "{local}");
}

use llrm_debug::{
    DebugOptions, Described, Function, Info, Kind, Line, Location, Range, Reach, Register, Scalar, Type, Variable,
};
use llrm_object::{Binding, Object, Section, Symbol, Target};

use super::*;

fn int() -> Type {
    Type::Scalar(Scalar::Int { bytes: 4, signed: true })
}

/// `f(x) { y }` in 16 bytes of code with the body at 4..12, x and y in the
/// frame, lines 3 and 4.
fn object(
    variables: Vec<Variable>,
    mut types: Vec<Type>,
    format: Format,
) -> Described {
    types.push(Type::Procedure { result: Some(0), parameters: vec![0], convention: None });
    let procedure = types.len() - 1;
    let text = Section {
        name: "_TEXT".into(),
        role: Role::Text,
        near: true,
        align: 1,
        image: vec![0x90; 16],
        spans: vec![[0, 16]],
        relocs: Vec::new(),
    };
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
        frame: Vec::new(),
        all_calls: false,
        calls: Vec::new(),
    };
    let register = |name: &str, dwarf| Register { name: name.into(), bits: 32, dwarf, codeview: None };
    let info = Info {
        options: DebugOptions { format, ..Default::default() },
        language: llrm_debug::Language::C,
        target: Some(llrm_debug::DebugTarget::of(32).with_registers(
            "ebp",
            "",
            vec![register("ebp", Some(5)), register("eax", Some(0)), register("ah", None)],
        )),
        code: vec![Range { section: 0, offset: 0, length: 16 }],
        files: vec![llrm_debug::File { name: "f.c".into(), checksum: None }],
        types,
        functions: vec![function],
        lines: vec![
            Line { section: 0, offset: 0, file: 0, line: 3, column: 0 },
            Line { section: 0, offset: 8, file: 0, line: 4, column: 0 },
        ],
        ..Info::default()
    };
    Described {
        debug: Some(info),
        object: Object {
            name: "f.c".into(),
            bits: 32,
            sections: vec![text],
            symbols: vec![Symbol {
                name: "_f".into(),
                binding: Binding::Public,
                definition: Definition::Defined { section: 0, offset: 0 },
                size: 0,
                group: None,
            }],
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        },
    }
}

fn frame(
    name: &str,
    disp: i64,
) -> Variable {
    Variable { name: name.into(), r#type: 0, kind: Kind::Local, location: Location::Frame { disp } }
}

fn written(
    variables: Vec<Variable>,
    types: Vec<Type>,
    format: Format,
) -> Result<Object, Unsupported> {
    let made = object(variables, types, format);
    expanded(&made, made.debug.as_ref().unwrap())
}

fn named<'a>(
    made: &'a Object,
    name: &str,
) -> (usize, &'a Section) {
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

/// A unit's length counts what follows it, its abbreviation offset is a
/// reference to the abbreviation section (the linker places it, so one object's
/// offset is not another's), and the header is the version's: 5 puts the unit
/// type and the address size before the offset.
#[test]
fn a_unit_header_says_its_length_its_version_and_where_its_abbreviations_are() {
    for (version, offset_at, size_at) in [(5u16, 8usize, 7usize), (4, 6, 10)] {
        let made = written(vec![frame("x", 8)], vec![int()], Format::Dwarf { version }).unwrap();
        let (abbrev, _) = named(&made, ".debug_abbrev");
        let (_, info) = named(&made, ".debug_info");
        assert_eq!(
            u32::from_le_bytes(info.image[..4].try_into().unwrap()) as usize,
            info.image.len() - 4,
            "version {version}"
        );
        assert_eq!(u16::from_le_bytes([info.image[4], info.image[5]]), version);
        assert_eq!(info.image[size_at], 4, "an i386 address");
        let reloc = info
            .relocs
            .iter()
            .find(|one| one.at == offset_at)
            .unwrap_or_else(|| panic!("no relocation at the abbreviation offset, version {version}"));
        assert_eq!((reloc.target, reloc.addend), (Target::Section(abbrev), 0));
    }
}

/// Each string, each address and each reference between sections is a
/// relocation, not a number that holds only for the first object a linker
/// places.
#[test]
fn addresses_and_cross_section_offsets_are_relocations() {
    let made = written(vec![frame("x", 8)], vec![int()], Format::Default).unwrap();
    let (_, info) = named(&made, ".debug_info");
    let to_symbol = info.relocs.iter().filter(|one| matches!(one.target, Target::Symbol(0))).count();
    let to_section = info.relocs.iter().filter(|one| matches!(one.target, Target::Section(_))).count();
    // The unit's low_pc and the function's; the abbreviations, the line table
    // and each string.
    assert_eq!(to_symbol, 2, "{:?}", info.relocs);
    assert!(to_section >= 5, "{:?}", info.relocs);
    let (_, line) = named(&made, ".debug_line");
    assert!(line.relocs.iter().any(|one| matches!(one.target, Target::Symbol(0))), "the sequence's address");
}

/// A function's body start is a row marked prologue_end, and its end
/// epilogue_begin, even where no source line begins there: gdb stops a
/// breakpoint on a function after the prologue by it.
#[test]
fn a_bodys_start_and_end_are_marked_in_the_line_table() {
    let made = written(vec![frame("x", 8)], vec![int()], Format::Default).unwrap();
    let (_, line) = named(&made, ".debug_line");
    // After the header, the program: set_address, then rows.
    // DW_LNS_set_prologue_end is 10, epilogue_begin 11.
    let program = &line.image[line.image.len() - 40..];
    assert!(program.contains(&10) && program.contains(&11), "{program:?}");
}

/// A merged location is line 0 (`merged_line`): the row is DWARF's "no line",
/// a signed advance back from line 3 and forward to line 4, the addresses still
/// rising.
#[test]
fn a_line_zero_row_is_a_valid_no_line_row_in_the_line_table() {
    let mut made = object(vec![frame("x", 8)], vec![int()], Format::Default);
    made.debug.as_mut().unwrap().lines.insert(1, Line { section: 0, offset: 4, file: 0, line: 0, column: 0 });
    let made = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
    let (_, line) = named(&made, ".debug_line");
    // DW_LNS_advance_line is 3: line 3 to 0 is -3 (0x7D), 0 to 4 is +4.
    assert!(line.image.windows(2).any(|pair| pair == [3, 0x7D]), "{:?}", line.image);
    assert!(line.image.windows(2).any(|pair| pair == [3, 4]), "{:?}", line.image);
}

/// The name of a variable is in the unit's strings if its DIE was written.
fn names(
    object: &Object,
    name: &str,
) -> bool {
    let wanted = format!("{name}\0");
    object
        .sections
        .iter()
        .any(|one| one.name == ".debug_str" && one.image.windows(wanted.len()).any(|at| at == wanted.as_bytes()))
}

/// A far or huge pointer is a pointer with Open Watcom's DW_AT_address_class
/// (far16 2, huge16 3, far32 5): a segment and an offset, still pointing at its
/// target. It was refused, and the variable with it.
#[test]
fn a_far_pointer_is_written_with_its_address_class() {
    let far = Type::Pointer { target: 0, bytes: 4, reach: Reach::Far };
    let huge = Type::Pointer { target: 0, bytes: 4, reach: Reach::Huge };
    let mut one = frame("xfar", 8);
    one.r#type = 1;
    let mut two = frame("xhuge", 12);
    two.r#type = 2;
    let made = written(vec![one, two], vec![int(), far.clone(), huge], Format::Default).expect("written");
    assert!(names(&made, "xfar") && names(&made, "xhuge"));
    // DW_AT_address_class (0x33) as a one-byte constant is in an abbreviation.
    let abbrev = made.sections.iter().find(|one| one.name == ".debug_abbrev").expect("abbreviations");
    assert!(abbrev.image.windows(2).any(|two| two == [0x33, 0x0b]));
    // A pointer of a width no address class names is left out with its
    // variable.
    let odd = Type::Pointer { target: 0, bytes: 3, reach: Reach::Far };
    let mut three = frame("xodd", 8);
    three.r#type = 1;
    let made = written(vec![three, frame("xnear", 12)], vec![int(), odd], Format::Default).expect("written");
    assert!(!names(&made, "xodd") && names(&made, "xnear"));
}

/// BASIC's array (its bounds are a descriptor's) leaves its variable out, never
/// written as something else; a place DWARF has no expression for (a register
/// with no number, a static in a list) is left out of the variable, which is
/// written with no location: a refused compile was `-g` failing a program that
/// uses `ah` or an array (GORILLA).
#[test]
fn what_dwarf_cannot_say_is_left_out() {
    let array = Type::Array { element: 0, bytes: None };
    let mut basic = frame("arr", 8);
    basic.r#type = 1;
    let made = written(vec![basic], vec![int(), array], Format::Default).expect("written without the array");
    assert!(!names(&made, "arr"));
    let high = Variable { name: "h".into(), r#type: 0, kind: Kind::Local, location: Location::Register("ah".into()) };
    let kept = written(vec![high], vec![int()], Format::Default).expect("written without a location");
    assert!(kept.sections.iter().any(|one| one.name == ".debug_str" && one.image.windows(2).any(|two| two == b"h\0")));
    // A list of one that holds neither a frame cell nor a register (a static,
    // say) has no expression here.
    let range = Range { section: 0, offset: 0, length: 4 };
    let moved = Variable {
        name: "m".into(),
        r#type: 0,
        kind: Kind::Local,
        location: Location::List(vec![(range, Location::Static { symbol: 0, disp: 0 })]),
    };
    assert!(written(vec![moved], vec![int()], Format::Default).is_ok());
}

/// A format this writer does not write is refused with which: an object cannot
/// carry the information of another format.
#[test]
fn a_format_that_is_not_dwarf_is_refused() {
    assert!(written(Vec::new(), vec![int()], Format::CodeView).unwrap_err().0.contains("CodeView"));
    assert!(written(Vec::new(), vec![int()], Format::TurboDebugger).unwrap_err().0.contains("Turbo Debugger"));
    assert!(written(Vec::new(), vec![int()], Format::Dwarf { version: 1 }).unwrap_err().0.contains("version 1"));
}

/// A register is DW_OP_reg0 + its number, and a number past 31 is DW_OP_regx.
#[test]
fn a_registers_location_is_its_dwarf_number() {
    let made = written(
        vec![Variable { name: "r".into(), r#type: 0, kind: Kind::Local, location: Location::Register("eax".into()) }],
        vec![int()],
        Format::Default,
    )
    .unwrap();
    let (_, info) = named(&made, ".debug_info");
    assert!(info.image.windows(2).any(|pair| pair == [1, 0x50]), "a one-byte expression, DW_OP_reg0");
}

/// A test that cannot run says so on stderr, and fails where
/// `LLRM_REQUIRE_DWARF` is set, so a gate that has the tools cannot pass by
/// skipping.
fn skipped(reason: &str) {
    // Written to the stderr itself, which the harness does not capture: seen
    // when the test passes.
    let _ = std::io::Write::write_all(&mut std::io::stderr(), format!("SKIPPED: {reason}\n").as_bytes());
    assert!(!llrm_support::env_set("LLRM_REQUIRE_DWARF"), "LLRM_REQUIRE_DWARF is set, and: {reason}");
}

fn dwarfdump() -> Option<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> =
        std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    dirs.push("/usr/lib/llvm-20/bin".into());
    dirs.iter().flat_map(|dir| [dir.join("llvm-dwarfdump"), dir.join("llvm-dwarfdump-20")]).find(|path| path.exists())
}

/// llvm-dwarfdump verifies the unit in an ELF32, an ELF64 and a Mach-O object,
/// whose addresses are 4, 8 and 8 bytes, for DWARF 4 and 5, with a struct, an
/// array, a pointer, a static and a register variable.
#[test]
fn llvm_dwarfdump_verifies_the_unit_in_either_class_and_version() {
    let Some(dump) = dwarfdump() else {
        skipped("llvm-dwarfdump is not installed");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    // 32 and 64: ELF32 and ELF64; 0: Mach-O, which is 64-bit.
    for (arch, bits) in [(32u8, 32), (64u8, 64), (64u8, 0)] {
        for version in [4u16, 5] {
            let types = vec![
                int(),
                Type::Struct {
                    name: "pt".into(),
                    bytes: 8,
                    fields: vec![
                        llrm_debug::Field { name: "x".into(), r#type: 0, offset: 0, bits: None },
                        llrm_debug::Field { name: "f".into(), r#type: 0, offset: 4, bits: Some((3, 5)) },
                    ],
                    union: false,
                },
                Type::Array { element: 1, bytes: Some(32) },
                Type::Pointer { target: 1, bytes: 4, reach: Reach::Near },
            ];
            let mut made = object(
                vec![
                    frame("x", 8),
                    Variable {
                        name: "s".into(),
                        r#type: 2,
                        kind: Kind::Local,
                        location: Location::Frame { disp: -40 },
                    },
                    Variable {
                        name: "p".into(),
                        r#type: 3,
                        kind: Kind::Parameter,
                        location: Location::Register("eax".into()),
                    },
                    Variable {
                        name: "g".into(),
                        r#type: 0,
                        kind: Kind::Local,
                        location: Location::Static { symbol: 0, disp: 4 },
                    },
                    // In eax over the first seven bytes, then in its frame
                    // cell: a list of two.
                    Variable {
                        name: "q".into(),
                        r#type: 0,
                        kind: Kind::Parameter,
                        location: Location::List(vec![
                            (Range { section: 0, offset: 0, length: 7 }, Location::Register("eax".into())),
                            (Range { section: 0, offset: 7, length: 9 }, Location::Frame { disp: -4 }),
                        ]),
                    },
                ],
                types,
                Format::Dwarf { version },
            );
            made.bits = arch;
            let bytes = match bits {
                32 => objects("elf", &made),
                64 => objects("elf", &made),
                _ => objects("macho", &made),
            }
            .unwrap();
            let path = scratch.path().join("x.o");
            std::fs::write(&path, bytes).unwrap();
            let said = std::process::Command::new(&dump).arg("--verify").arg(&path).output().unwrap();
            let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
            assert!(
                said.status.success() && text.trim_end().ends_with("No errors.") && !text.contains("warning"),
                "{bits} DWARF {version}:\n{text}"
            );
            let shown = std::process::Command::new(&dump).arg("--debug-info").arg(&path).output().unwrap();
            let shown = String::from_utf8_lossy(&shown.stdout);
            for expected in
                ["DW_AT_name\t(\"pt\")", "DW_AT_bit_size", "DW_AT_upper_bound\t(3)", "DW_OP_reg0", "DW_OP_addr"]
            {
                assert!(shown.contains(expected), "{bits} DWARF {version}: no {expected} in\n{shown}");
            }
            // The list: register 0 over [0, 7) and fbreg -4 over [7, 16), in
            // the section of lists its version has.
            let lists = std::process::Command::new(&dump)
                .arg(if version >= 5 { "--debug-loclists" } else { "--debug-loc" })
                .arg(&path)
                .output()
                .unwrap();
            let lists = String::from_utf8_lossy(&lists.stdout);
            assert!(
                lists.contains("DW_OP_reg0 ") && lists.contains("DW_OP_fbreg -4"),
                "{bits} DWARF {version}: the list in\n{lists}"
            );
        }
    }
}

/// A variable the optimiser removed (a list of no entry) is a DIE with a name
/// and a type and no location, which a debugger shows as optimized out; one
/// with a location has it.
#[test]
fn a_removed_variable_has_no_location_and_the_others_have_theirs() {
    let Some(dump) = dwarfdump() else {
        skipped("llvm-dwarfdump is not installed");
        return;
    };
    let removed =
        Variable { name: "gone".into(), r#type: 0, kind: Kind::Parameter, location: Location::List(Vec::new()) };
    let made = written(vec![removed, frame("x", 8)], vec![int()], Format::Default).unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("x.o");
    std::fs::write(&path, objects("elf", &Described::from(made.clone())).unwrap()).unwrap();
    let said = std::process::Command::new(&dump).arg("--verify").arg(&path).output().unwrap();
    assert!(
        String::from_utf8_lossy(&said.stdout).trim_end().ends_with("No errors."),
        "{}",
        String::from_utf8_lossy(&said.stdout)
    );
    let shown = std::process::Command::new(&dump).arg("--debug-info").arg(&path).output().unwrap();
    let shown = String::from_utf8_lossy(&shown.stdout).into_owned();
    let parameter = &shown[shown.find("DW_AT_name\t(\"gone\")").expect("the parameter")..];
    let parameter = &parameter[..parameter.find("DW_TAG").unwrap_or(parameter.len())];
    assert!(!parameter.contains("DW_AT_location"), "{parameter}");
    let local = &shown[shown.find("DW_AT_name\t(\"x\")").expect("the local")..];
    assert!(local.contains("DW_OP_fbreg +8"), "{local}");
}

/// A function the code gives no frame register is found from the canonical
/// frame address: `-g` kept `ebp` for every function with a variable, which
/// cost the code a `push ebp` a build without `-g` does not have. A cell the
/// register would have addressed at `disp` is `disp - bias` from the address.
#[test]
fn a_frame_cell_is_found_from_the_canonical_frame_address_where_the_code_keeps_no_register() {
    let Some(dump) = dwarfdump() else {
        skipped("llvm-dwarfdump is not installed");
        return;
    };
    let mut made = object(vec![frame("x", -4)], vec![int()], Format::Default);
    made.debug.as_mut().unwrap().frame_base = llrm_debug::FrameBase::Cfa { bias: 8, far_bias: 8 };
    let made = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("x.o");
    std::fs::write(&path, objects("elf", &Described::from(made.clone())).unwrap()).unwrap();
    let shown = std::process::Command::new(&dump).arg("--debug-info").arg(&path).output().unwrap();
    let shown = String::from_utf8_lossy(&shown.stdout).into_owned();
    assert!(shown.contains("DW_AT_frame_base\t(DW_OP_call_frame_cfa)"), "{shown}");
    assert!(shown.contains("DW_OP_fbreg -12"), "{shown}");
}

/// A function called far has a longer return address, so the frame address is
/// further from the register its cells are numbered by: a 16-bit far function's
/// `[bp-2]` is `bp+6-8` from the address, not `bp+4-6`. The offsets were
/// computed for a near call and read the saved BP in place of the first
/// parameter (dosbox-harness on a real run).
#[test]
fn a_function_called_far_finds_its_cells_from_the_longer_frame_address() {
    let Some(dump) = dwarfdump() else {
        skipped("llvm-dwarfdump is not installed");
        return;
    };
    let fbreg = |far: bool| {
        let mut made = object(vec![frame("x", -2)], vec![int()], Format::Default);
        let info = made.debug.as_mut().unwrap();
        info.frame_base = llrm_debug::FrameBase::Cfa { bias: 4, far_bias: 6 };
        info.functions[0].far = far;
        let made = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("x.o");
        std::fs::write(&path, objects("elf", &Described::from(made.clone())).unwrap()).unwrap();
        let shown = std::process::Command::new(&dump).arg("--debug-info").arg(&path).output().unwrap();
        String::from_utf8_lossy(&shown.stdout).into_owned()
    };
    assert!(fbreg(false).contains("DW_OP_fbreg -6"), "{}", fbreg(false));
    assert!(fbreg(true).contains("DW_OP_fbreg -8"), "{}", fbreg(true));
}

/// `.debug_frame`: the CIE says the frame address is `esp+4` and the return
/// address is DWARF column 8 just below it; the function's FDE says each change
/// by the shortest instruction: an offset change alone, a register
/// change alone, both, a register saved (`DW_CFA_offset`, the offset in units
/// of -4) and restored.
#[test]
fn a_functions_frame_rows_are_the_cfa_instructions_the_dwarf_standard_gives() {
    use llrm_debug::FrameRow;
    let row = |offset, register: &str, cfa, saved: &[(&str, i64)]| FrameRow {
        offset,
        cfa_register: register.into(),
        cfa_offset: cfa,
        saved: saved.iter().map(|&(name, at)| (name.to_owned(), at)).collect(),
    };
    let mut made = object(Vec::new(), vec![int()], Format::Dwarf { version: 4 });
    let info = made.debug.as_mut().unwrap();
    let register = |name: &str, number| Register { name: name.into(), bits: 32, dwarf: Some(number), codeview: None };
    let file = info.registers_mut(32);
    file.registers.extend([register("esp", 4), register("eip", 8)]);
    file.return_register = "eip".into();
    info.functions[0].frame = vec![
        row(0, "esp", 4, &[]),
        row(1, "esp", 8, &[("ebp", -8)]),
        row(3, "ebp", 8, &[("ebp", -8)]),
        row(70, "esp", 4, &[]),
    ];
    let written = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
    let image = &named(&written, ".debug_frame").1.image;
    let cie = [0x10, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 4, 0, 4, 0, 1, 0x7C, 8, 0x0C, 4, 4, 0x88, 1];
    assert_eq!(image[..20], cie);
    // Past the length, the CIE's offset, the address and its range.
    let fde = &image[20 + 16..];
    let expected = [0x41, 0x0E, 8, 0x85, 2, 0x42, 0x0D, 5, 0x02, 67, 0x0C, 4, 4, 0xC5];
    assert_eq!(fde[..expected.len()], expected, "{fde:x?}");
    assert!(fde[expected.len()..].iter().all(|&one| one == 0), "padded with DW_CFA_nop");
}

/// A base type is named as the source spells it: `int` and `unsigned long`
/// where the frontend said so, and `uint32_t` where it did not. Every base type
/// was `int32_t`/`uint32_t`, which is what gdb printed for a C `int`.
#[test]
fn a_base_type_is_named_as_the_source_spells_it() {
    let strings = |types: Vec<Type>| {
        let made = written(Vec::new(), types, Format::Dwarf { version: 4 }).unwrap();
        named(&made, ".debug_str").1.image.clone()
    };
    let has = |image: &[u8], name: &str| {
        image
            .windows(name.len() + 2)
            .any(|window| window[0] == 0 && &window[1..=name.len()] == name.as_bytes() && window[name.len() + 1] == 0)
            || image.starts_with(name.as_bytes())
    };
    let spelled =
        strings(vec![Type::Basic { name: "unsigned long".into(), scalar: Scalar::Int { bytes: 4, signed: false } }]);
    assert!(has(&spelled, "unsigned long") && !has(&spelled, "uint32_t"), "{:?}", String::from_utf8_lossy(&spelled));
    let plain = strings(vec![Type::Scalar(Scalar::Int { bytes: 4, signed: false })]);
    assert!(has(&plain, "uint32_t"), "{:?}", String::from_utf8_lossy(&plain));
}

/// The unit header's address size was `object.arch.bits() / 8`, so a machine
/// with no `Arch` variant could not be written: a target says it in
/// `Info::target`.
/// `target` as the machine, with the registers the information had.
fn retarget(
    info: &mut Info,
    mut target: llrm_debug::DebugTarget,
) {
    target.registers = info.registers_mut(0).clone().into();
    info.target = Some(target);
}

#[test]
fn a_target_with_three_byte_addresses_writes_dwarf_with_three_byte_addresses() {
    let mut made = object(vec![frame("x", 8)], vec![int()], Format::Dwarf { version: 5 });
    retarget(
        made.debug.as_mut().unwrap(),
        llrm_debug::DebugTarget {
            address_bytes: 3,
            segmented: false,
            codeview_machine: None,
            segment_base: None,
            far_pointer: None,
            registers: Default::default(),
            entry_values: false,
        },
    );
    let written = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
    let (_, info) = named(&written, ".debug_info");
    assert_eq!(info.image[7], 3, "the unit header's address size");
}

/// Segment fixups came from constants of the object crate, so a 16-bit target
/// that named none still got x86's; the target says the kind or the writer
/// refuses.
#[test]
fn a_segmented_address_takes_the_targets_segment_fixup() {
    let mut made = object(vec![frame("x", 8)], vec![int()], Format::Dwarf { version: 4 });
    made.bits = 16;
    let info = made.debug.as_mut().unwrap();
    retarget(info, llrm_debug::DebugTarget::of(16));
    let error = expanded(&made, made.debug.as_ref().unwrap()).unwrap_err();
    assert!(error.0.contains("segment fixup"), "{error:?}");
    let kind = llrm_object::FixupKind(llrm_object::FixupKind::FIRST_TARGET.0 + 9);
    retarget(
        made.debug.as_mut().unwrap(),
        llrm_debug::DebugTarget { segment_base: Some(kind), ..llrm_debug::DebugTarget::of(16) },
    );
    let written = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
    assert!(written.sections.iter().flat_map(|one| &one.relocs).any(|one| one.kind == kind));
}

/// `object` through x86's writer for `format`, which expands its debug
/// information first.
fn objects(
    format: &str,
    object: &Described,
) -> Result<Vec<u8>, llrm_object::Unsupported> {
    use llrm_debug::DebugObjectTarget;
    llrm_x86::mc::X86Objects.write_described(format, &object.object, object.debug.as_ref())
}

/// `f` in two pieces of one section (0..6 and 10..16), and the unit's code the
/// same: the writer refused it ("function f has 2 ranges: one is written"); it
/// is `DW_AT_ranges` over a list in `.debug_ranges` (2-4) or `.debug_rnglists`
/// (5).
fn split(
    version: u16,
    unit: bool,
) -> Described {
    let mut made = object(vec![frame("x", 8)], vec![int()], Format::Dwarf { version });
    let pieces = vec![Range { section: 0, offset: 0, length: 6 }, Range { section: 0, offset: 10, length: 6 }];
    let info = made.debug.as_mut().unwrap();
    info.functions[0].ranges = pieces.clone();
    if unit {
        info.code = pieces;
    }
    made
}

fn abbreviates(
    made: &Object,
    attribute: u8,
    form: u8,
) -> bool {
    named(made, ".debug_abbrev").1.image.windows(2).any(|pair| pair == [attribute, form])
}

#[test]
fn a_function_of_two_ranges_has_a_list_in_debug_ranges_before_5() {
    for version in [2u16, 3, 4] {
        let made = split(version, false);
        let made = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
        let (_, ranges) = named(&made, ".debug_ranges");
        // Two pairs of addresses and the two zeros that end the list.
        assert_eq!(ranges.image, vec![0; 24], "version {version}");
        let addends: Vec<i64> = ranges.relocs.iter().map(|one| one.addend).collect();
        assert_eq!(addends, [0, 6, 10, 16], "version {version}");
        let form = if version < 4 { 0x06 } else { 0x17 };
        assert!(abbreviates(&made, 0x55, form), "DW_AT_ranges, version {version}");
    }
}

#[test]
fn a_function_of_two_ranges_has_a_list_in_debug_rnglists_in_5() {
    let made = split(5, false);
    let made = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
    let (_, lists) = named(&made, ".debug_rnglists");
    // unit_length, 5, address size, no segments, one list, its offset (from
    // the offsets), two start_length entries and the end.
    let mut want = vec![0u8; 0];
    want.extend(25u32.to_le_bytes());
    want.extend([5, 0, 4, 0]);
    want.extend(1u32.to_le_bytes());
    want.extend(4u32.to_le_bytes());
    want.extend([0x07, 0, 0, 0, 0, 6, 0x07, 0, 0, 0, 0, 6, 0x00]);
    assert_eq!(lists.image, want);
    assert!(abbreviates(&made, 0x55, 0x23), "DW_AT_ranges as DW_FORM_rnglistx");
    assert!(abbreviates(&made, 0x74, 0x17), "DW_AT_rnglists_base, which DW_FORM_rnglistx counts from");
}

#[test]
fn a_unit_of_two_code_ranges_has_ranges_and_a_base_of_zero() {
    for version in [4u16, 5] {
        let made = split(version, true);
        let made = expanded(&made, made.debug.as_ref().unwrap()).unwrap();
        let (_, info) = named(&made, ".debug_info");
        assert!(abbreviates(&made, 0x55, if version == 5 { 0x23 } else { 0x17 }), "version {version}");
        assert!(abbreviates(&made, 0x11, 0x01), "a low_pc beside the ranges, version {version}");
        assert!(!abbreviates(&made, 0x12, 0x06) || version < 4, "no high_pc on the unit");
        assert_eq!(abbreviates(&made, 0x74, 0x17), version == 5, "rnglists_base, version {version}");
        assert!(info.relocs.iter().any(|one| one.at > 0), "version {version}");
    }
}

#[test]
fn a_segmented_function_of_two_ranges_is_refused_by_name() {
    let mut made = split(4, false);
    made.bits = 16;
    retarget(made.debug.as_mut().unwrap(), llrm_debug::DebugTarget::of(16));
    let why = expanded(&made, made.debug.as_ref().unwrap()).err().expect("refused");
    assert!(why.0.contains("segmented"), "{}", why.0);
}

/// A lexical block in two pieces on a segmented target was refused ("a DIE of
/// several code ranges on a segmented target"), so every optimised 16-bit
/// `-g` compile with a nested scope failed. It is written over its first byte
/// to its last, as the formats with no list of ranges do.
#[test]
fn a_segmented_block_of_two_ranges_is_written_over_its_hull() {
    let mut made = object(vec![frame("x", 8)], vec![int()], Format::Dwarf { version: 4 });
    made.bits = 16;
    let kind = llrm_object::FixupKind(llrm_object::FixupKind::FIRST_TARGET.0 + 9);
    retarget(
        made.debug.as_mut().unwrap(),
        llrm_debug::DebugTarget { segment_base: Some(kind), ..llrm_debug::DebugTarget::of(16) },
    );
    let pieces = vec![Range { section: 0, offset: 2, length: 2 }, Range { section: 0, offset: 6, length: 2 }];
    made.debug.as_mut().unwrap().functions[0].blocks =
        vec![llrm_debug::Block { ranges: pieces, variables: Vec::new(), blocks: Vec::new(), inlined: None }];
    let written = expanded(&made, made.debug.as_ref().unwrap());
    assert!(written.is_ok(), "{:?}", written.err().map(|why| why.0));
}

/// Mach-O's writer found the offsets of one debug section into another among
/// the relocations and put them in the fields itself; the DWARF writer says
/// where a format has none (`doesDwarfUseRelocationsAcrossSections`), so the
/// fields hold the offsets and no relocation names a debug section.
#[test]
fn a_format_without_relocations_across_sections_gets_the_offsets_in_the_fields() {
    let made = object(vec![frame("x", 8)], vec![int()], Format::Default);
    let info = made.debug.as_ref().unwrap();
    let relocated = expanded_for(&made, info, true).unwrap();
    let plain = expanded_for(&made, info, false).unwrap();
    let (_, relocated_info) = named(&relocated, ".debug_info");
    let first = made.sections.len();
    let (_, plain_info) = named(&plain, ".debug_info");
    let across = |section: &Section| {
        section.relocs.iter().filter(|one| matches!(one.target, Target::Section(home) if home >= first)).count()
    };
    assert!(across(relocated_info) >= 5 && across(plain_info) == 0, "{:?}", plain_info.relocs);
    assert_ne!(relocated_info.image, plain_info.image, "the strings' offsets are in the fields");
    assert_eq!(
        relocated_info.relocs.len() - across(relocated_info),
        plain_info.relocs.len(),
        "addresses stay relocated"
    );
}

/// A function that lists its calls gets `call_all_calls`, a declaration DIE
/// for the callee it does not define, and one call-site DIE per call with the
/// return pc, the origin and the arguments; the unit verifies. Without the
/// writer there were none, so a debugger could not show the arguments of a
/// caller's callee.
#[test]
fn call_sites_are_dies_with_origin_return_pc_and_arguments() {
    use llrm_debug::{CallOrigin, CallParameter, CallSite, CallValue, Declaration};
    let Some(dump) = dwarfdump() else {
        skipped("llvm-dwarfdump is not installed");
        return;
    };
    let scratch = tempfile::tempdir().unwrap();
    for version in [4u16, 5] {
        let mut made = object(Vec::new(), vec![int()], Format::Dwarf { version });
        let info = made.debug.as_mut().unwrap();
        info.declarations = vec![Declaration { name: "g".into(), r#type: 1, external: true }];
        info.functions[0].all_calls = true;
        info.functions[0].calls = vec![CallSite {
            section: 0,
            offset: 8,
            origin: Some(CallOrigin::Declaration(0)),
            parameters: vec![
                CallParameter { register: "eax".into(), value: CallValue::Constant(7) },
                CallParameter { register: "eax".into(), value: CallValue::Entry("eax".into()) },
                CallParameter { register: "eax".into(), value: CallValue::Register { name: "ebp".into(), offset: 0 } },
            ],
        }];
        let path = scratch.path().join("x.o");
        std::fs::write(&path, objects("elf", &made).unwrap()).unwrap();
        let said = std::process::Command::new(&dump).arg("--verify").arg(&path).output().unwrap();
        let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
        assert!(said.status.success() && text.trim_end().ends_with("No errors."), "DWARF {version}:\n{text}");
        let shown = std::process::Command::new(&dump).arg("--debug-info").arg(&path).output().unwrap();
        let shown = String::from_utf8_lossy(&shown.stdout);
        let entry = if version >= 5 { "DW_OP_entry_value" } else { "DW_OP_GNU_entry_value" };
        let (site, param, all, origin, ret, value) = if version >= 5 {
            (
                "DW_TAG_call_site\n",
                "DW_TAG_call_site_parameter",
                "DW_AT_call_all_calls",
                "DW_AT_call_origin",
                "DW_AT_call_return_pc",
                "DW_AT_call_value",
            )
        } else {
            (
                "DW_TAG_GNU_call_site\n",
                "DW_TAG_GNU_call_site_parameter",
                "DW_AT_GNU_all_call_sites",
                "DW_AT_abstract_origin",
                "DW_AT_low_pc",
                "DW_AT_GNU_call_site_value",
            )
        };
        for expected in [site, param, all, origin, ret, value, "DW_AT_declaration", "DW_OP_lit7", entry, "DW_OP_breg5"]
        {
            assert!(shown.contains(expected), "DWARF {version}: no {expected:?} in\n{shown}");
        }
    }
}

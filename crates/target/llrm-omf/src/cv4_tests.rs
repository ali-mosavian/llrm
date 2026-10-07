use llrm_object::debug::{Field, Function, Info, Kind, Language, Location, Range, Scalar as S, Type as T, Variable};
use llrm_object::{Arch, Binding, Definition, Object, Role, Section, Symbol};

use super::sections;

const SYMBOLS: usize = 0;
const TYPES: usize = 1;

fn int() -> T {
    T::Scalar(S::Int { bytes: 2, signed: false })
}

fn variable(name: &str, r#type: usize, kind: Kind, location: Location) -> Variable {
    Variable { name: name.into(), r#type, kind, location }
}

/// `f` in 0x16 bytes of code with `types` and `variables`, its type the last of `types`.
fn object(arch: Arch, types: Vec<T>, variables: Vec<Variable>) -> Object {
    let text = Section { name: "_TEXT".into(), role: Role::Text, near: true, align: 1, image: vec![0x90; 0x16], spans: vec![[0, 0x16]], relocs: Vec::new() };
    let range = Range { section: 0, offset: 0, length: 0x16 };
    let function = Function { name: "f".into(), symbol: 0, r#type: types.len() - 1, ranges: vec![range], body: Some((6, 0x16)), far: false, module: false, variables, blocks: Vec::new() };
    let info = Info { language: Language::C, frame_register: if arch == Arch::I8086 { "bp" } else { "ebp" }.into(), code: vec![range], types, functions: vec![function], ..Info::default() };
    Object {
        name: "t.obj".into(),
        arch,
        sections: vec![text],
        symbols: vec![Symbol { name: "_f".into(), binding: Binding::Public, definition: Definition::Defined { section: 0, offset: 0 }, group: None }],
        omf_groups: Vec::new(),
        debug: Some(info),
    }
}

/// A table's records as (code, data), signature checked.
fn records(image: &[u8], padded: bool) -> Vec<(u16, Vec<u8>)> {
    assert_eq!(image[..4], [1, 0, 0, 0], "the CodeView 4 signature");
    let mut out = Vec::new();
    let mut at = 4;
    while at < image.len() {
        let length = usize::from(u16::from_le_bytes([image[at], image[at + 1]]));
        let code = u16::from_le_bytes([image[at + 2], image[at + 3]]);
        out.push((code, image[at + 4..at + 2 + length].to_vec()));
        if padded {
            assert_eq!((at + 2 + length) % 4, 0, "type record {} is not on four bytes", out.len() - 1);
        }
        at += 2 + length;
    }
    out
}

fn procedure() -> T {
    T::Procedure { result: None, parameters: vec![0, 0], convention: None }
}

/// ML 6.11's /Zi object for `f proc near c, a:word, b:word; local x:word` has these records for the procedure:
/// LF_ARGLIST `02 00 21 00 21 00 f2 f1`, LF_PROCEDURE `03 00 00 00 02 00 <arglist>`, S_GPROC16 with zeros
/// for the scope links and `S_BPREL16` of 4, 6 and -2. llrm's records are byte for byte those, ML having
/// put the procedure type before its argument list where llrm puts it after (a reader takes either).
#[test]
fn a_16_bit_procedure_is_the_records_ml_writes() {
    let made = object(
        Arch::I8086,
        vec![int(), procedure()],
        vec![variable("a", 0, Kind::Parameter, Location::Frame { disp: 4 }), variable("b", 0, Kind::Parameter, Location::Frame { disp: 6 }), variable("x", 0, Kind::Local, Location::Frame { disp: -2 })],
    );
    let [symbols, types] = sections(&made, made.debug.as_ref().unwrap()).unwrap();
    assert_eq!(types.name, "$$TYPES");
    assert_eq!(symbols.name, "$$SYMBOLS");
    assert_eq!(
        records(&types.image, true),
        [(0x0201, vec![2, 0, 0x21, 0, 0x21, 0, 0xF2, 0xF1]), (0x0008, vec![3, 0, 0, 0, 2, 0, 0, 0x10])]
    );
    let all = records(&symbols.image, false);
    let proc16 = [vec![0; 12], vec![0x16, 0, 6, 0, 0x16, 0, 0, 0, 0, 0, 0x01, 0x10, 0, 1, b'f']].concat();
    // The type is 0x1001, the second record; ML's `00 10` is its own numbering.
    assert_eq!(
        all[2..],
        [
            (0x0105, proc16),
            (0x0100, vec![4, 0, 0x21, 0, 1, b'a']),
            (0x0100, vec![6, 0, 0x21, 0, 1, b'b']),
            (0x0100, vec![0xFE, 0xFF, 0x21, 0, 1, b'x']),
            (0x0006, vec![]),
        ]
    );
    // The procedure's offset and segment are one far pointer, which LINK fills in.
    assert_eq!(symbols.relocs.len(), 1);
    // After the signature, the object name and compile records, the procedure's header, links and three lengths.
    assert_eq!(symbols.relocs[0].at, 4 + (all[0].1.len() + 4) + (all[1].1.len() + 4) + 4 + 12 + 6);
}

fn only_function(made: &mut Object, change: impl FnOnce(&mut Function)) {
    change(&mut made.debug.as_mut().unwrap().functions[0]);
}

fn written(made: &Object) -> (Vec<(u16, Vec<u8>)>, Vec<(u16, Vec<u8>)>, Vec<llrm_object::Reloc>) {
    let [symbols, types] = sections(made, made.debug.as_ref().unwrap()).unwrap();
    (records(&symbols.image, false), records(&types.image, true), symbols.relocs)
}

/// A struct that holds a pointer to itself: the struct's index is taken before its members are, so the
/// pointer names it (0x1000) and the field list that follows has the member `next`, the struct record
/// then has the field list's index. Made after its members, the pointer had nothing to name.
#[test]
fn a_struct_that_points_to_itself_is_named_before_its_members_and_has_a_udt() {
    let node = T::Struct {
        name: "node".into(),
        bytes: 4,
        fields: vec![Field { name: "next".into(), r#type: 2, offset: 0, bits: None }, Field { name: "v".into(), r#type: 0, offset: 2, bits: None }],
        union: false,
    };
    let types = vec![T::Scalar(S::Int { bytes: 2, signed: true }), node, T::Pointer { target: 1, bytes: 2, reach: llrm_object::debug::Reach::Near }, T::Procedure { result: None, parameters: vec![2], convention: None }];
    let (symbols, types, _) = written(&object(Arch::I8086, types, Vec::new()));
    let member = |kind: u16, offset: u8, name: &[u8], pad: &[u8]| [vec![0x06, 0x04, kind as u8, (kind >> 8) as u8, 3, 0, offset, 0, name.len() as u8], name.to_vec(), pad.to_vec()].concat();
    assert_eq!(
        types,
        [
            (0x0005, [vec![2, 0, 0x02, 0x10, 0, 0, 0, 0, 0, 0, 4, 0, 4], b"node".to_vec(), vec![0xF3, 0xF2, 0xF1]].concat()),
            (0x0002, vec![0, 0, 0x00, 0x10, 0, 0, 0, 0]),
            (0x0204, [member(0x1001, 0, b"next", &[0xF3, 0xF2, 0xF1]), member(0x0011, 2, b"v", &[0xF2, 0xF1])].concat()),
            (0x0201, vec![1, 0, 0x01, 0x10]),
            (0x0008, vec![3, 0, 0, 0, 1, 0, 0x03, 0x10]),
        ]
    );
    assert_eq!(symbols.last().unwrap(), &(0x0004, [vec![0x00, 0x10, 4], b"node".to_vec()].concat()), "S_UDT names the struct");
}

/// ML's `.386 flat` /Zi object: S_GPROC32 is the three links, three four-byte lengths, a four-byte offset and a
/// two-byte segment, the type, the flags and the name; its locals are `S_BPREL32` with a four-byte offset. The
/// offset and the segment are two fixups here (an offset32 and a base), ML's one 16:32 pointer's two halves.
#[test]
fn a_32_bit_procedure_has_four_byte_fields_and_two_fixups() {
    let mut made = object(Arch::I386, vec![T::Scalar(S::Int { bytes: 4, signed: true }), T::Procedure { result: None, parameters: vec![0], convention: None }], vec![variable("a", 0, Kind::Parameter, Location::Frame { disp: 8 })]);
    made.debug.as_mut().unwrap().functions[0].body = Some((3, 5));
    made.debug.as_mut().unwrap().functions[0].ranges[0].length = 5;
    let (symbols, types, relocs) = written(&made);
    // A 32-bit program's four-byte integer is T_INT4.
    assert_eq!(types[0], (0x0201, vec![1, 0, 0x74, 0]));
    let proc32 = [vec![0; 12], vec![5, 0, 0, 0, 3, 0, 0, 0, 5, 0, 0, 0], vec![0; 6], vec![0x01, 0x10, 0, 1, b'f']].concat();
    assert_eq!(symbols[2..], [(0x0205, proc32), (0x0200, vec![8, 0, 0, 0, 0x74, 0, 1, b'a']), (0x0006, vec![])]);
    assert_eq!(relocs.iter().map(|one| (one.kind, one.at - relocs[0].at)).collect::<Vec<_>>(), [(llrm_object::Kind::Abs { width: 4 }, 0), (llrm_object::Kind::SegmentBase, 4)]);
}

/// Far is the function's, not the type's: the same procedure type called far is a second record whose call
/// byte is 1, and the symbol's flag bit 2 says it returns far. Written as near, a debugger stepping out of
/// a far function popped the wrong return address.
#[test]
fn a_far_function_has_a_far_procedure_type_and_the_far_flag() {
    let mut made = object(Arch::I8086, vec![int(), procedure()], Vec::new());
    only_function(&mut made, |one| one.far = true);
    let (symbols, types, _) = written(&made);
    assert_eq!(types[1], (0x0008, vec![3, 0, 1, 0, 2, 0, 0, 0x10]));
    assert_eq!(symbols[2].1.last().copied(), Some(b'f'));
    assert_eq!(symbols[2].1[symbols[2].1.len() - 3], 0x04, "flags: far return");
}

/// `local`'s records and the type index its `S_BPREL16` names, for a function of no parameters: the function's own
/// two records (an empty argument list at 0x1000 and the procedure at 0x1001) come first.
fn local_of(types: Vec<T>, local: usize) -> (Vec<(u16, Vec<u8>)>, u16) {
    let mut all = types;
    all.push(T::Procedure { result: None, parameters: Vec::new(), convention: None });
    let made = object(Arch::I8086, all, vec![variable("v", local, Kind::Local, Location::Frame { disp: -2 })]);
    let (symbols, types, _) = written(&made);
    let bprel = symbols.iter().find(|(code, _)| *code == 0x0100).expect("the local");
    (types[2..].to_vec(), u16::from_le_bytes([bprel.1[2], bprel.1[3]]))
}

/// Every type a C program has, by the layout cv4f.h gives: an array with its byte size and index type, an
/// enum with its field list of enumerators (a negative one tagged), a bit field as its own record that the
/// member names, a union (LF_UNION, with no derived list or vshape), and `const`. Each record ends where the
/// next starts on four bytes, with the filler `LF_PAD` counts down.
#[test]
fn a_c_programs_types_are_the_records_cv4f_h_describes() {
    let short = || T::Scalar(S::Int { bytes: 2, signed: true });
    let field = |name: &str, bits| Field { name: name.into(), r#type: 0, offset: 0, bits };
    let (records, at) = local_of(vec![short(), T::Array { element: 0, bytes: Some(20) }], 1);
    assert_eq!((records, at), (vec![(0x0003, vec![0x11, 0, 0x21, 0, 0x14, 0, 0, 0xF1])], 0x1002), "array");

    let enumerators = vec![llrm_object::debug::Enumerator { name: "a".into(), value: 0 }, llrm_object::debug::Enumerator { name: "b".into(), value: -1 }];
    let (records, at) = local_of(vec![short(), T::Enum { name: "e".into(), underlying: 0, enumerators }], 1);
    let list = [vec![0x03, 0x04, 3, 0, 0, 0, 1, b'a'], vec![0x03, 0x04, 3, 0, 0x00, 0x80, 0xFF, 1, b'b', 0xF3, 0xF2, 0xF1]].concat();
    assert_eq!((records, at), (vec![(0x0204, list), (0x0007, vec![2, 0, 0x11, 0, 0x02, 0x10, 0, 0, 1, b'e', 0xF2, 0xF1])], 0x1003), "enum");

    let (records, at) = local_of(vec![short(), T::Struct { name: "s".into(), bytes: 2, fields: vec![field("a", Some((0, 3)))], union: false }], 1);
    assert_eq!(
        (records, at),
        (
            vec![
                (0x0005, vec![1, 0, 0x04, 0x10, 0, 0, 0, 0, 0, 0, 2, 0, 1, b's', 0xF2, 0xF1]),
                (0x0206, vec![3, 0, 0x11, 0]),
                (0x0204, vec![0x06, 0x04, 0x03, 0x10, 3, 0, 0, 0, 1, b'a', 0xF2, 0xF1]),
            ],
            0x1002
        ),
        "a struct with a bit field: the struct is numbered before its members"
    );
    let (records, at) = local_of(vec![short(), T::Struct { name: "u".into(), bytes: 2, fields: vec![field("a", None)], union: true }], 1);
    assert_eq!(at, 0x1002);
    assert_eq!(records[0], (0x0006, vec![1, 0, 0x03, 0x10, 0, 0, 2, 0, 1, b'u', 0xF2, 0xF1]), "a union has no derived list or vshape");

    let (records, at) = local_of(vec![short(), T::Qualified { target: 0, constant: true, volatile: false }], 1);
    assert_eq!((records, at), (vec![(0x0001, vec![1, 0, 0x11, 0])], 0x1002), "const");
}

/// A pointer to a primitive is a primitive pointer (the mode in the high byte), which needs no record; one to
/// anything else is LF_POINTER.
#[test]
fn a_pointer_to_a_primitive_is_a_primitive_pointer_and_to_a_record_is_a_record() {
    let short = || T::Scalar(S::Int { bytes: 2, signed: true });
    let pointer = |reach, bytes| T::Pointer { target: 0, bytes, reach };
    use llrm_object::debug::Reach::{Far, Huge, Near};
    for (reach, bytes, expected) in [(Near, 2, 0x0111), (Far, 4, 0x0211), (Huge, 4, 0x0311)] {
        let (records, at) = local_of(vec![short(), pointer(reach, bytes)], 1);
        assert_eq!((records.len(), at), (0, expected), "{reach:?}");
    }
    let (records, at) = local_of(vec![short(), T::Array { element: 0, bytes: Some(2) }, T::Pointer { target: 1, bytes: 2, reach: Near }], 2);
    // The array is first, the pointer second; near is kind 0, and ML writes four zero bytes after the type.
    assert_eq!((records[1].clone(), at), ((0x0002, vec![0, 0, 0x02, 0x10, 0, 0, 0, 0]), 0x1003));
}

/// What CodeView 4 has no record for is left out and the rest is written: a record names one place for a whole
/// scope, so a value that is in a register for only part of it (a register parameter, until the body starts)
/// and one the optimiser removed have none; one that is in a register throughout is S_REGISTER.
#[test]
fn a_value_in_a_register_throughout_is_s_register_and_one_part_of_the_time_is_left_out() {
    let whole = Range { section: 0, offset: 0, length: 0x16 };
    let part = Range { section: 0, offset: 0, length: 5 };
    let reg = |name: &str, location| variable(name, 0, Kind::Parameter, location);
    let mut made = object(
        Arch::I8086,
        vec![int(), T::Procedure { result: None, parameters: Vec::new(), convention: None }],
        vec![
            reg("r", Location::Register("ax".into())),
            reg("w", Location::List(vec![(whole, Location::Register("cx".into()))])),
            reg("p", Location::List(vec![(part, Location::Register("dx".into()))])),
            reg("g", Location::List(Vec::new())),
            variable("c", 0, Kind::Local, Location::Frame { disp: -2 }),
        ],
    );
    made.debug.as_mut().unwrap().registers = [("ax", 9), ("cx", 10), ("dx", 11)].map(|(name, number)| llrm_object::debug::Register { name: name.into(), bits: 16, dwarf: None, codeview: Some(number) }).to_vec();
    let (symbols, ..) = written(&made);
    let found: Vec<(u16, Vec<u8>)> = symbols[3..].to_vec();
    assert_eq!(
        found,
        [
            (0x0002, vec![0x21, 0, 9, 0, 1, b'r']),
            (0x0002, vec![0x21, 0, 10, 0, 1, b'w']),
            (0x0100, vec![0xFE, 0xFF, 0x21, 0, 1, b'c']),
            (0x0006, vec![]),
        ]
    );
}

/// A lexical block is S_BLOCK16 (the two links, a length, an offset and a segment, an empty name) with its own
/// variables and S_END, its offset a fixup against the section it is in.
#[test]
fn a_block_is_a_scope_with_its_variables_and_an_end() {
    let mut made = object(Arch::I8086, vec![int(), T::Procedure { result: None, parameters: Vec::new(), convention: None }], Vec::new());
    only_function(&mut made, |one| {
        one.blocks = vec![llrm_object::debug::Block { ranges: vec![Range { section: 0, offset: 8, length: 6 }], variables: vec![variable("i", 0, Kind::Local, Location::Frame { disp: -4 })], blocks: Vec::new() }];
    });
    let (symbols, _, relocs) = written(&made);
    assert_eq!(
        symbols[3..],
        [(0x0107, [vec![0; 8], vec![6, 0, 0, 0, 0, 0, 0]].concat()), (0x0100, vec![0xFC, 0xFF, 0x21, 0, 1, b'i']), (0x0006, vec![]), (0x0006, vec![])]
    );
    assert_eq!(relocs.len(), 2, "the procedure's and the block's address");
    assert_eq!(relocs[1].addend, 8);
}

/// The program of tests/inputs/cv4/p1.asm as the model says it, written, reads as what ML's own object of it
/// reads as (`cv4info`, the same reader): the procedure and its arguments, the parameters and locals with a struct's
/// members, and the struct's name. The two writers agree on what the program is, not on the order of records.
#[test]
fn the_writers_object_reads_as_mls_for_the_same_program() {
    use crate::{cv4info, omf, write};
    let ml = {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/cv4/p1.obj");
        let shape = cv4info::shape(&omf::parse(&std::fs::read(path).unwrap()).unwrap());
        shape.into_iter().filter(|one| !one.starts_with("DATA")).collect::<Vec<_>>()
    };
    let point = T::Struct {
        name: "point".into(),
        bytes: 4,
        fields: vec![Field { name: "px".into(), r#type: 0, offset: 0, bits: None }, Field { name: "py".into(), r#type: 0, offset: 2, bits: None }],
        union: false,
    };
    let made = object(
        Arch::I8086,
        vec![int(), point, procedure()],
        vec![
            variable("a", 0, Kind::Parameter, Location::Frame { disp: 4 }),
            variable("b", 0, Kind::Parameter, Location::Frame { disp: 6 }),
            variable("x", 0, Kind::Local, Location::Frame { disp: -2 }),
            variable("q", 1, Kind::Local, Location::Frame { disp: -6 }),
        ],
    );
    let ours = cv4info::shape(&omf::parse(&write::write(&made).unwrap()).unwrap());
    assert_eq!(ours, ml);
}

/// The 32-bit records have no Microsoft reader here (llvm-readobj reads C13, not CodeView 4 in OMF), so the check
/// is ML's own flat object of tests/inputs/cv4/p2.asm: a struct of two dwords, a procedure of two dword
/// parameters, a dword and a struct local. Written by llrm, it reads as ML's does through the one reader.
/// ML's `dword` is T_ULONG and C's `unsigned int` is T_UINT4, the one name that is mapped.
#[test]
fn the_32_bit_writers_object_reads_as_mls_flat_object_does() {
    use crate::{cv4info, omf, write};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/cv4/p2.obj");
    let ml: Vec<String> = cv4info::shape(&omf::parse(&std::fs::read(path).unwrap()).unwrap()).into_iter().filter(|one| !one.starts_with("DATA")).collect();
    let dword = T::Scalar(S::Int { bytes: 4, signed: false });
    let point = T::Struct {
        name: "point".into(),
        bytes: 8,
        fields: vec![Field { name: "px".into(), r#type: 0, offset: 0, bits: None }, Field { name: "py".into(), r#type: 0, offset: 4, bits: None }],
        union: false,
    };
    let made = object(
        Arch::I386,
        vec![dword, point, T::Procedure { result: None, parameters: vec![0, 0], convention: None }],
        vec![
            variable("a", 0, Kind::Parameter, Location::Frame { disp: 8 }),
            variable("b", 0, Kind::Parameter, Location::Frame { disp: 12 }),
            variable("x", 0, Kind::Local, Location::Frame { disp: -4 }),
            variable("q", 1, Kind::Local, Location::Frame { disp: -12 }),
        ],
    );
    let ours: Vec<String> = cv4info::shape(&omf::parse(&write::write(&made).unwrap()).unwrap()).into_iter().map(|one| one.replace("UINT4", "UNSIGNED LONG")).collect();
    assert_eq!(ours, ml);
}

/// A pointer to a function reaches it far or near, and the procedure it points to is called as that reach says: a far
/// pointer's is a far call (LF_PROCEDURE call kind 1), a near one's a near call. The procedure type was one record
/// called near, whatever pointed to it, so a debugger took a call through `int (far *fp)(int)` for a near one.
#[test]
fn a_pointer_to_a_function_points_to_a_procedure_called_as_far_as_the_pointer_reaches() {
    use llrm_object::debug::Reach::{Far, Near};
    let short = T::Scalar(S::Int { bytes: 2, signed: true });
    let types = vec![
        short,
        T::Procedure { result: Some(0), parameters: vec![0], convention: None },
        T::Pointer { target: 1, bytes: 4, reach: Far },
        T::Pointer { target: 1, bytes: 2, reach: Near },
        T::Procedure { result: None, parameters: Vec::new(), convention: None },
    ];
    let made = object(Arch::I8086, types, vec![variable("p", 2, Kind::Local, Location::Frame { disp: -4 }), variable("q", 3, Kind::Local, Location::Frame { disp: -6 })]);
    let (_, table, _) = written(&made);
    let index = |one: usize| 0x1000 + one as u16;
    let one_parameter: Vec<(u16, u8)> = table.iter().enumerate().filter(|(_, (leaf, data))| *leaf == 0x0008 && data[4] == 1).map(|(at, (_, data))| (index(at), data[2])).collect();
    // Two procedures of one parameter: the far one (call 1) and the near one (call 0).
    let call = |wanted: u8| one_parameter.iter().find(|(_, kind)| *kind == wanted).map(|(at, _)| *at);
    let (far, near) = (call(1).expect("a far procedure"), call(0).expect("a near procedure"));
    let target = |reach: u16| table.iter().find(|(leaf, data)| *leaf == 0x0002 && u16::from_le_bytes([data[0], data[1]]) == reach).map(|(_, data)| u16::from_le_bytes([data[2], data[3]]));
    assert_eq!((target(1), target(0)), (Some(far), Some(near)), "a far pointer to the far procedure, a near one to the near");
}

use std::rc::Rc;

use super::*;
use crate::cvinfo;
use llrm_support::pyrepr::Repr;
use crate::omf::{self, Record};

/// An object holding only `written`'s two segments, named as BC names its.
fn object(written: &Written) -> Vec<Rc<Record>> {
    let pascal = |name: &str| [&[name.len() as u8][..], name.as_bytes()].concat();
    let mut records = vec![
        Record::new(omf::THEADR, pascal("P.BAS")),
        Record::new(omf::LNAMES, ["", "$$SYMBOLS", "DEBSYM", "$$TYPES", "DEBTYP"].iter().flat_map(|one| pascal(one)).collect()),
    ];
    for (name, bytes) in [(2u8, &written.symbols), (4, &written.types)] {
        let size = (bytes.len() as u16).to_le_bytes();
        records.push(Record::new(omf::SEGDEF, vec![0x28, size[0], size[1], name, name + 1, 1]));
    }
    for (segment, bytes) in [(1u8, &written.symbols), (2, &written.types)] {
        for (n, chunk) in bytes.chunks(1000).enumerate() {
            let at = ((n * 1000) as u16).to_le_bytes();
            records.push(Record::new(omf::LEDATA, [&[segment, at[0], at[1]][..], chunk].concat()));
        }
    }
    records.into_iter().map(Rc::new).collect()
}

/// The shape, and the kind of each $$TYPES record, sorted: QB 4.5 and the
/// later compilers encode one BYREF parameter differently.
fn shape(records: &[Rc<Record>]) -> (Vec<String>, Vec<String>) {
    let mut kinds: Vec<String> = cvinfo::type_table(records).values().map(|one| one.repr().split('(').next().unwrap_or("").to_owned()).collect();
    kinds.sort();
    (cvinfo::parse(records).shape(), kinds)
}

fn bc(name: &str) -> (Vec<String>, Vec<String>) {
    // BC's /Zi objects: the corpus's, and those only this reads.
    let path = crate::testing::fixtures().join(name);
    let path = if path.exists() { path } else { crate::testing::fixtures().join("../codeview").join(name) };
    let found = shape(&omf::read(path).expect("reads"));
    assert!(!found.0.is_empty(), "{name} carries no /Zi symbols");
    found
}

fn procedure(name: &str, r#type: TypeId, locals: &[(&str, TypeId, i16)]) -> Procedure {
    Procedure {
        name: name.into(),
        symbol: name.into(),
        r#type,
        length: 0,
        debug_start: 0,
        debug_end: 0,
        far: true,
        locals: locals.iter().map(|&(name, r#type, bp)| Local { name: name.into(), r#type, bp }).collect(),
        statics: Vec::new(),
    }
}

fn data(name: &str, r#type: TypeId) -> Data {
    Data { name: name.into(), r#type, symbol: name.into(), displacement: 0 }
}

/// byref2.bas: two FUNCTIONs of one BYREF parameter each.
fn byref2(names: [&str; 6], extra: Option<&str>) -> Module {
    let [half, doubled, n1, n2, d, s] = names;
    let types = vec![
        Type::Scalar(Scalar::Float32),
        Type::Reference(0),
        Type::Procedure { result: Some(0), parameters: vec![1] },
        Type::Scalar(Scalar::Float64),
        Type::Reference(3),
        Type::Procedure { result: Some(3), parameters: vec![4] },
        Type::Scalar(Scalar::Int16),
    ];
    let mut data = vec![data(d, 3), data(s, 0)];
    data.extend(extra.map(|name| self::data(name, 6)));
    Module {
        name: None,
        start: "main".into(),
        length: 0,
        types,
        procedures: vec![procedure(half, 2, &[(n1, 1, 8)]), procedure(doubled, 5, &[(n2, 4, 8)])],
        data,
    }
}

/// Each dialect's BC /Zi object of byref2.bas reads back as ours does.
#[test]
fn byref_parameters_read_as_bc_writes_them() {
    let vbdos = byref2(["Half", "Doubled", "n", "n", "d", "s"], None);
    assert_eq!(shape(&object(&written(&vbdos, Flavor::default()).expect("writes"))), bc("byref2-v-g3-zi.obj"));
    let pds = byref2(["HALF!", "DOUBLED#", "N!", "N#", "D#", "S!"], None);
    assert_eq!(shape(&object(&written(&pds, Flavor::default()).expect("writes"))), bc("byref2-p-g2-zi.obj"));
    let qb45 = byref2(["HALF!", "DOUBLED#", "N!", "N#", "D#", "S!"], Some("__bseg%"));
    assert_eq!(shape(&object(&written(&qb45, Flavor { qb45: true }).expect("writes"))), bc("byref2-q-o-zi.obj"));
}

/// byval.bas: a BYVAL LONG beside a BYREF one.
#[test]
fn a_byval_parameter_is_its_scalar() {
    let module = |names: [&str; 6]| {
        let [add_ref, add_val, n1, n2, a, b] = names;
        Module {
            types: vec![
                Type::Scalar(Scalar::Int32),
                Type::Reference(0),
                Type::Procedure { result: Some(0), parameters: vec![1] },
                Type::Procedure { result: Some(0), parameters: vec![0] },
            ],
            procedures: vec![procedure(add_ref, 2, &[(n1, 1, 6)]), procedure(add_val, 3, &[(n2, 0, 6)])],
            data: vec![data(a, 0), data(b, 0)],
            ..Module::default()
        }
    };
    let vbdos = module(["AddRef", "AddVal", "n", "n", "a", "b"]);
    assert_eq!(shape(&object(&written(&vbdos, Flavor::default()).expect("writes"))), bc("byval-v-g3-zi.obj"));
    let pds = module(["ADDREF&", "ADDVAL&", "N&", "N&", "A&", "B&"]);
    assert_eq!(shape(&object(&written(&pds, Flavor::default()).expect("writes"))), bc("byval-p-g2-zi.obj"));
}

/// udt.bas: a TYPE of two LONGs, one of it and an array of it.
#[test]
fn a_structure_and_an_array_of_it_read_as_bc_writes_them() {
    let module = |names: [&str; 5], extra: Option<&str>| {
        let [coord, x, y, c, pts] = names;
        let field = |name: &str, offset| Field { name: name.into(), r#type: 0, offset, bits: None };
        let mut data = vec![data(c, 1), data(pts, 2)];
        data.extend(extra.map(|name| self::data(name, 3)));
        Module {
            types: vec![
                Type::Scalar(Scalar::Int32),
                Type::Struct { name: coord.into(), bytes: 8, fields: vec![field(x, 0), field(y, 4)] },
                Type::Array(1),
                Type::Scalar(Scalar::Int16),
            ],
            data,
            ..Module::default()
        }
    };
    for (object_name, names, flavor, extra) in [
        ("udt-v-g3-zi.obj", ["Coord", "x", "y", "c", "pts"], Flavor::default(), None),
        ("udt-p-g2-zi.obj", ["COORD", "X", "Y", "C", "PTS"], Flavor::default(), None),
        ("udt-q-o-zi.obj", ["COORD", "X", "Y", "C", "PTS"], Flavor { qb45: true }, Some("__bseg%")),
    ] {
        assert_eq!(shape(&object(&written(&module(names, extra), flavor).expect("writes"))), bc(object_name), "{object_name}");
    }
}

/// A near pointer and an array in place, C's kinds, read back as written.
#[test]
fn a_pointer_and_an_array_in_place_read_back() {
    let module = Module {
        types: vec![
            Type::Scalar(Scalar::Int16),
            Type::Pointer { target: 0, reach: Reach::Near },
            Type::Sized { element: 0, bytes: 10 },
        ],
        data: vec![data("p", 1), data("a", 2)],
        ..Module::default()
    };
    let (read, _) = shape(&object(&written(&module, Flavor::default()).expect("writes")));
    assert_eq!(read, ["DATA a: 10 BYTES OF INTEGER", "DATA p: BYREF INTEGER"]);
    for reach in [Reach::Far, Reach::Huge] {
        let module = Module { types: vec![Type::Scalar(Scalar::Int16), Type::Pointer { target: 0, reach }], data: vec![data("p", 1)], ..Module::default() };
        let (read, _) = shape(&object(&written(&module, Flavor::default()).expect("writes")));
        assert_eq!(read, ["DATA p: BYREF INTEGER"], "{reach:?}");
    }
}

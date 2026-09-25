//! Port of `tests/test_omf_addends.py`. Python's `canonical(records, ...) is
//! records` becomes: every returned record is the same `Rc` as the input's.

use std::path::Path;

use super::*;
use crate::frontends::bc::{blocks, fppatches};
use crate::objectfile::module;

fn word(code: &[u8], at: i64) -> i64 {
    i64::from(u16::from_le_bytes([code[at as usize], code[at as usize + 1]]))
}

#[test]
fn test_borland_indexed_array_address_includes_encoded_addend() {
    // Rebuilt d_faces drew zero triangles: 0CA0h in the instruction was lost.
    let path = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/regressions/d_faces-borland.obj");
    let found = module::of(&omf::read(path).unwrap()).unwrap();
    let mapped = blocks::code_map(&found).unwrap();
    let original = fppatches::native_records(&found, &mapped.starts);
    let before = module::of(&original).unwrap();
    let records = canonical(&original, found.seg, found.code.len() as i64).unwrap();
    let found = module::of(&records).unwrap();
    let address = found.resolve(0x9F7, 0xCA0);
    assert_eq!(address.disp, 0xCA0);
    assert_eq!(&found.code[0x9F7..0x9F9], b"\0\0");
    let again = canonical(&records, found.seg, found.code.len() as i64).unwrap();
    assert!(again.len() == records.len() && again.iter().zip(&records).all(|(a, b)| Rc::ptr_eq(a, b)));
    assert!(blocks::code_map(&found).is_ok());
    let offsets = |records: &[Rc<Record>]| -> IndexMap<i64, omf::Fixup> {
        omf::fixups(records)
            .into_iter()
            .filter(|fix| fix.seg == Some(found.seg) && fix.loc == omf::LOC_OFF16)
            .map(|fix| (fix.offset, fix))
            .collect()
    };
    let old = offsets(&original);
    let new = offsets(&records);
    assert_eq!(old.keys().collect::<BTreeSet<_>>(), new.keys().collect::<BTreeSet<_>>());
    for (&at, fix) in &old {
        let expected = (fix.disp + word(&before.code, at)) & 0xFFFF;
        assert_eq!((new[&at].disp + word(&found.code, at)) & 0xFFFF, expected, "{at:#x}");
        assert_eq!((&new[&at].target, new[&at].index, new[&at].frame), (&fix.target, fix.index, fix.frame), "{at:#x}");
    }
}

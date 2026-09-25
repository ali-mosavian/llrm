//! Port of `qbopt/objectfile/addends.py`: relocation addends moved out of
//! the code bytes and into their fixups.

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::omf::{self, Record};
use llrm_support::hash::IndexMap;

pub fn canonical(records: &[Rc<Record>], segment: i64, size: i64) -> Result<Vec<Rc<Record>>, String> {
    let image = omf::segment_image(records, segment, size);
    // keyed by id(record)
    let mut edits: IndexMap<*const Record, Vec<(usize, usize, Vec<u8>)>> = IndexMap::default();
    let mut cleared: BTreeSet<i64> = BTreeSet::new();
    for fixup in omf::fixups(records) {
        if fixup.seg != Some(segment) || fixup.loc != omf::LOC_OFF16 {
            continue;
        }
        let at = fixup.offset;
        if at + 2 > image.len() as i64 {
            return Err("relocation addend extends beyond code segment".to_owned());
        }
        let addend = u16::from_le_bytes([image[at as usize], image[at as usize + 1]]) as i64;
        if addend == 0 {
            continue;
        }
        if fixup.selfrel || cleared.contains(&at) || cleared.contains(&(at + 1)) {
            return Err("unsupported relative or overlapping relocation addend".to_owned());
        }
        let displacement = (fixup.disp + addend) & 0xFFFF;
        let raw = if fixup.disp_pos.is_some() {
            omf::reemit(&fixup, None, Some(displacement)).expect("the fixup carries a displacement")
        } else {
            // FIXDAT.P omits target displacement; clearing it adds a word
            // after the existing frame/target data, including threaded forms.
            let old = fixup.raw();
            let mut raw = old[..2].to_vec();
            raw.push(old[2] & !4);
            raw.extend_from_slice(&old[3..]);
            raw.extend_from_slice(&(displacement as u16).to_le_bytes());
            raw
        };
        edits.entry(Rc::as_ptr(&fixup.record)).or_default().push((fixup.lo, fixup.hi, raw));
        cleared.extend([at, at + 1]);
    }
    if cleared.is_empty() {
        return Ok(records.to_vec());
    }
    let chunks: IndexMap<*const Record, (i64, Vec<u8>)> = omf::ledata(records)
        .into_iter()
        .filter(|(_record, seg, _offset, _payload)| *seg == segment)
        .map(|(record, _seg, offset, payload)| (Rc::as_ptr(&record), (offset, payload)))
        .collect();
    let mut result = Vec::new();
    for record in records {
        let id = Rc::as_ptr(record);
        if let Some(edited) = edits.get(&id) {
            let mut edited = edited.clone();
            edited.sort();
            let mut end = 0;
            let mut pieces: Vec<u8> = Vec::new();
            for (lo, hi, raw) in edited {
                pieces.extend_from_slice(&record.body[end.min(record.body.len())..lo.max(end).min(record.body.len())]);
                pieces.extend_from_slice(&raw);
                end = hi;
            }
            pieces.extend_from_slice(&record.body[end.min(record.body.len())..]);
            result.push(Rc::new(Record::new(record.r#type, pieces)));
        } else if let Some((offset, payload)) = chunks.get(&id) {
            let changed: Vec<u8> = payload
                .iter()
                .enumerate()
                .map(|(index, &byte)| if cleared.contains(&(offset + index as i64)) { 0 } else { byte })
                .collect();
            let mut body = record.body[..record.body.len() - payload.len()].to_vec();
            body.extend_from_slice(&changed);
            result.push(Rc::new(Record::new(record.r#type, body)));
        } else {
            result.push(record.clone());
        }
    }
    Ok(result)
}


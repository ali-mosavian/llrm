//! Where a module's data goes: the segments its frontend put each object
//! in, the rest in the default data segment.

use std::collections::{BTreeSet, HashMap};

use llrm_mir::{GlobalId, GlobalKind, Linkage, Module};
use llrm_support::hash::IndexMap;

use crate::backend::{globals, masm};
use crate::hir::model;
use crate::objectfile::module::Space;

/// A module's data objects by segment, in the order they come, each by its
/// global's name, which the pipeline keeps where it keeps the global; and
/// the segments holding far objects, which stay out of the data group.
pub struct Placed {
    segments: Vec<(String, Vec<String>)>,
    private: BTreeSet<String>,
}

impl Placed {
    /// `hir`'s placement, `data` its objects' globals in `module`.
    pub fn of(module: &Module, hir: &model::Module, data: &HashMap<i64, GlobalId>) -> Self {
        let mut segments: Vec<(String, Vec<String>)> = Vec::new();
        for object in &hir.data {
            let (Some(segment), Some(name)) = (&object.segment, data.get(&object.id).and_then(|&id| module.global(id).name.clone())) else { continue };
            match segments.iter_mut().find(|(one, _)| one == segment) {
                Some((_, names)) => names.push(name),
                None => segments.push((segment.clone(), vec![name])),
            }
        }
        let far = hir.data.iter().filter(|one| matches!(one.address, model::AddressKind::Far | model::AddressKind::Huge));
        Self { segments, private: far.filter_map(|one| one.segment.clone()).collect() }
    }

    /// `built`'s data laid out: the default segment first, then each placed
    /// one; the compiler's constants in `constants`, else the default. Each
    /// variable aligned as stated, public where external; each declared one
    /// an extern, near where it is in `data_space`.
    pub fn lay_out(&self, built: &mut masm::Module, module: &Module, data_space: u32, constants: Option<&str>, far_bss: bool, segment_bytes: Option<usize>) -> Result<(), String> {
        let (default, items) = built.data.pop().ok_or("an assembled module without its data segment")?;
        // What assembly adds after the globals' data: its constant pool.
        let pool: Vec<masm::Datum> = items.into_iter().skip_while(|one| !matches!(one, masm::Datum::Label(label) if label.name.starts_with("$K"))).collect();
        let placed: BTreeSet<&str> = self.segments.iter().flat_map(|(_, names)| names).map(String::as_str).collect();
        let mut unplaced = Vec::new();
        for (at, global) in module.globals.iter().enumerate() {
            let GlobalKind::Variable(variable) = &global.kind else { continue };
            if variable.initializer.is_none() {
                let size = if global.address_space == data_space { "byte" } else { "far-byte" };
                built.externs.push((built.names[&(Space::External, at as i64)].clone(), size.to_owned()));
            } else if !global.name.as_deref().is_some_and(|name| placed.contains(name)) {
                unplaced.push(GlobalId(at as u32));
            }
        }
        built.externs.sort();
        let mut segments = vec![(default, unplaced)];
        for (segment, names) in &self.segments {
            segments.push((segment.clone(), names.iter().filter_map(|name| module.named(name)).collect()));
        }
        let mut bss: Vec<(String, Vec<GlobalId>)> = Vec::new();
        for (segment, members) in segments {
            // What the image need not store (`stored_zero`): an object of zeros in far data, where the
            // start-up zeroes far uninitialised data, in a segment of its own beside its segment.
            let (zero, members): (Vec<GlobalId>, Vec<GlobalId>) = if far_bss && self.private.contains(&segment) {
                members.into_iter().partition(|&global| stored_zero(module, global, &built.names))
            } else {
                (Vec::new(), members)
            };
            if !zero.is_empty() {
                bss.push((format!("{segment}_BSS"), zero));
            }
            let mut items = Vec::new();
            for global in members {
                let GlobalKind::Variable(variable) = &module.global(global).kind else { continue };
                if let Some(to) = variable.align {
                    items.push(masm::Datum::Align(masm::Align { to: to as i64 }));
                }
                for datum in globals::datums(module, global, &built.names)? {
                    items.push(match datum {
                        masm::Datum::Bytes(bytes) if uninitialized(&segment) => masm::Datum::Fill(masm::Fill { size: bytes.len() as i64, byte: None }),
                        datum => datum,
                    });
                }
                if module.global(global).linkage == Linkage::External {
                    built.publics.push(built.names[&(globals::space(module, global), i64::from(global.0))].clone());
                }
            }
            // An object past 64K runs on through further segments, each a full 64K
            // and paragraph aligned, which the linker lays end to end: the segment
            // value of a huge pointer's next 64K is this one's plus 0x1000.
            let parts = split(items, &segment, segment_bytes)?;
            for (at, part) in parts.into_iter().enumerate() {
                if at == 0 {
                    built.data.push((segment.clone(), part));
                } else {
                    let name = format!("{segment}_{at}");
                    built.private.insert(name.clone());
                    built.data.push((name, part));
                }
            }
        }
        for (segment, objects) in bss {
            let mut items = Vec::new();
            for global in objects {
                if let GlobalKind::Variable(variable) = &module.global(global).kind
                    && let Some(to) = variable.align
                {
                    items.push(masm::Datum::Align(masm::Align { to: to as i64 }));
                }
                let size = globals::datums(module, global, &built.names)?.iter().map(|one| match one {
                    masm::Datum::Bytes(bytes) => bytes.len(),
                    _ => 0,
                }).sum::<usize>();
                items.push(masm::Datum::Label(masm::Label { name: built.names[&(Space::Segment, i64::from(global.0))].clone() }));
                items.push(masm::Datum::Fill(masm::Fill { size: size as i64, byte: None }));
                if module.global(global).linkage == Linkage::External {
                    built.publics.push(built.names[&(globals::space(module, global), i64::from(global.0))].clone());
                }
            }
            for (at, part) in split(items, &segment, segment_bytes)?.into_iter().enumerate() {
                let name = if at == 0 { segment.clone() } else { format!("{segment}_{at}") };
                built.private.insert(name.clone());
                built.far_bss.insert(name.clone());
                built.data.push((name, part));
            }
        }
        match constants {
            Some(segment) if !pool.is_empty() => built.data.push((segment.to_owned(), pool)),
            _ => built.data[0].1.extend(pool),
        }
        built.private.extend(self.private.iter().cloned());
        Ok(())
    }
}

/// What one segment can hold: a 16-bit offset's range.
/// `items` cut into segments of at most `limit` bytes (64K where segments are), each but the last full. Only
/// the one object a huge segment holds may be cut: it is alone, so its
/// bytes start at offset 0.
fn split(items: Vec<masm::Datum>, segment: &str, limit: Option<usize>) -> Result<Vec<Vec<masm::Datum>>, String> {
    use masm::Datum;
    let limit = limit.unwrap_or(usize::MAX / 2);
    let mut parts: Vec<Vec<Datum>> = vec![Vec::new()];
    let mut used = 0;
    let mut total = 0;
    let mut objects = 0;
    for datum in items {
        if matches!(datum, Datum::Label(_) | Datum::Object(_)) {
            objects += 1;
        }
        // What is left of a datum to place, as bytes or fill, may be cut anywhere.
        let (mut rest, make): (usize, Box<dyn Fn(usize, usize) -> Datum>) = match datum {
            Datum::Bytes(bytes) => {
                let whole = bytes.clone();
                (bytes.len(), Box::new(move |from, to| Datum::Bytes(whole[from..to].to_vec())))
            }
            Datum::Fill(masm::Fill { size, byte }) => (size as usize, Box::new(move |from, to| Datum::Fill(masm::Fill { size: (to - from) as i64, byte }))),
            Datum::Align(masm::Align { to }) => {
                let pad = (-(total as i64)).rem_euclid(to) as usize;
                // Kept as asked where it fits, for the segment to be aligned
                // as it says; cut into fill only where it straddles a 64K end.
                if used + pad <= limit {
                    used += pad;
                    total += pad;
                    parts.last_mut().expect("a part").push(Datum::Align(masm::Align { to }));
                    continue;
                }
                (pad, Box::new(move |from, to| Datum::Fill(masm::Fill { size: (to - from) as i64, byte: Some(0) })))
            }
            other => {
                let size = match &other {
                    Datum::Pointer(masm::Pointer { bytes, .. }) => *bytes as usize,
                    Datum::SegmentWord(_) => 2,
                    _ => 0,
                };
                if size > 0 && used == limit {
                    parts.push(Vec::new());
                    used = 0;
                }
                if used + size > limit {
                    return Err(format!("{segment}: an address straddles the end of a 64K segment"));
                }
                used += size;
                total += size;
                parts.last_mut().expect("a part").push(other);
                continue;
            }
        };
        let mut from = 0;
        while rest > 0 {
            if used == limit {
                parts.push(Vec::new());
                used = 0;
            }
            let room = limit - used;
            let take = rest.min(room);
            parts.last_mut().expect("a part").push(make(from, from + take));
            from += take;
            rest -= take;
            used += take;
            total += take;
        }
    }
    if parts.len() > 1 && objects > 1 {
        return Err(format!("{segment}: a huge object shares its segment with another"));
    }
    Ok(parts)
}

/// Whether `global`'s data is all zeros, with no address in it: what an image need not store. The one
/// decision that zero data is uninitialised data, whatever shape its initializer has.
fn stored_zero(module: &Module, global: GlobalId, names: &IndexMap<(Space, i64), String>) -> bool {
    if !matches!(module.global(global).kind, GlobalKind::Variable(_)) {
        return false;
    }
    globals::datums(module, global, names)
        .is_ok_and(|datums| datums.iter().all(|one| matches!(one, masm::Datum::Label(_)) || matches!(one, masm::Datum::Bytes(bytes) if bytes.iter().all(|byte| *byte == 0))))
}

/// Whether the object format keeps no bytes for `segment`.
fn uninitialized(segment: &str) -> bool {
    masm::SEGMENTS.get(segment) == Some(&".data?")
}

#[cfg(test)]
mod tests {
    use super::*;
    use masm::{Datum, Fill, Label, Pointer};

    fn size(part: &[Datum]) -> usize {
        part.iter()
            .map(|one| match one {
                Datum::Bytes(bytes) => bytes.len(),
                Datum::Fill(Fill { size, .. }) => *size as usize,
                Datum::Pointer(Pointer { bytes, .. }) => *bytes as usize,
                _ => 0,
            })
            .sum()
    }

    fn label() -> Datum {
        Datum::Label(Label { name: "_big".into() })
    }

    /// An 80000-byte object was one 80000-byte segment, which no object file holds.
    /// A flat target has no 64K segment: an 80000-byte object was cut in two (and refused beside
    /// another), as a huge one is, so a flat program with a large array did not link.
    #[test]
    fn a_target_without_segments_does_not_cut_an_object() {
        let parts = split(vec![label(), Datum::Fill(Fill { size: 80000, byte: None })], "S", None).unwrap();
        assert_eq!(parts.len(), 1);
    }

    #[test]
    fn test_an_object_past_64k_is_cut_into_full_segments_and_a_rest() {
        let parts = split(vec![label(), Datum::Fill(Fill { size: 80000, byte: None })], "S", Some(0x1_0000)).unwrap();
        assert_eq!(parts.iter().map(|part| size(part)).collect::<Vec<_>>(), [65536, 14464]);
        assert!(matches!(parts[0][0], Datum::Label(_)) && !parts[1].iter().any(|one| matches!(one, Datum::Label(_))));
    }

    #[test]
    fn test_an_object_of_exactly_64k_is_one_segment() {
        let parts = split(vec![label(), Datum::Bytes(vec![1; 65536])], "S", Some(0x1_0000)).unwrap();
        assert_eq!(parts.iter().map(|part| size(part)).collect::<Vec<_>>(), [65536]);
    }

    #[test]
    fn test_an_address_after_a_full_segment_starts_the_next() {
        let pointer = Datum::Pointer(Pointer { name: "_x".into(), offset: 0, far: true, bytes: 4 });
        let parts = split(vec![label(), Datum::Bytes(vec![0; 65536]), pointer], "S", Some(0x1_0000)).unwrap();
        assert_eq!(parts.iter().map(|part| size(part)).collect::<Vec<_>>(), [65536, 4]);
    }

    #[test]
    fn test_a_huge_object_shares_its_segment_with_none() {
        let two = vec![label(), Datum::Bytes(vec![0; 40000]), label(), Datum::Bytes(vec![0; 40000])];
        assert!(split(two, "S", Some(0x1_0000)).unwrap_err().contains("shares its segment"));
    }

    /// A variable's `align 4` became bytes of fill before the object file
    /// saw it, so the segment never learned its widest request.
    #[test]
    fn test_an_align_request_survives_for_the_segment_to_keep() {
        let parts = split(vec![label(), Datum::Bytes(vec![1]), Datum::Align(masm::Align { to: 4 }), Datum::Bytes(vec![2])], "S", Some(0x1_0000)).unwrap();
        assert!(parts[0].iter().any(|one| matches!(one, Datum::Align(masm::Align { to: 4 }))), "{:?}", parts[0]);
    }
}

//! Where a module's data goes: the segments its frontend put each object
//! in, the rest in the default data segment.

use std::collections::{BTreeSet, HashMap};

use llrm_mir::{GlobalId, GlobalKind, Linkage, Module};

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
        let far = hir.data.iter().filter(|one| one.address == model::AddressKind::Far);
        Self { segments, private: far.filter_map(|one| one.segment.clone()).collect() }
    }

    /// `built`'s data laid out: the default segment first, then each placed
    /// one; the compiler's constants in `constants`, else the default. Each
    /// variable aligned as stated, public where external; each declared one
    /// an extern, near where it is in `data_space`.
    pub fn lay_out(&self, built: &mut masm::Module, module: &Module, data_space: u32, constants: Option<&str>) -> Result<(), String> {
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
        for (segment, members) in segments {
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
            built.data.push((segment, items));
        }
        match constants {
            Some(segment) if !pool.is_empty() => built.data.push((segment.to_owned(), pool)),
            _ => built.data[0].1.extend(pool),
        }
        built.private.extend(self.private.iter().cloned());
        Ok(())
    }
}

/// Whether the object format keeps no bytes for `segment`.
fn uninitialized(segment: &str) -> bool {
    masm::SEGMENTS.get(segment) == Some(&".data?")
}

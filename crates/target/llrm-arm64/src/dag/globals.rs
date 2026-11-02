//! What the DAG path reads of the module's globals, by the id a
//! `GlobalAddress` node carries: the linker's name, whether the address comes
//! from the GOT, and the alignment a pointer to it has.

use std::collections::HashMap;
use std::rc::Rc;

use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{GlobalKind, Module};

use crate::machine::Platform;

pub struct Global {
    pub name: String,
    pub got: bool,
    /// `GlobalValue::getPointerAlignment`: the declared alignment, else the
    /// type's preferred one; 1 for a function.
    pub align: u64,
    /// The allocation size of the variable's type; `None` for a function (not
    /// sized).
    pub size: Option<u64>,
}

#[derive(Clone, Default)]
pub struct Globals(Rc<HashMap<u32, Global>>, Option<Platform>);

impl Globals {
    pub fn of(
        module: &Module,
        layout: &DataLayout,
        platform: Platform,
    ) -> Globals {
        let mut by_id = HashMap::new();
        for (at, global) in module.globals.iter().enumerate() {
            let id = GlobalId(at as u32);
            let Some(name) = global.name.owned() else { continue };
            let align = match &global.kind {
                GlobalKind::Function(_) => 1,
                GlobalKind::Variable(variable) => {
                    variable.align.unwrap_or_else(|| layout.pref_align(&module.context.types, variable.ty))
                }
            };
            let size = match &global.kind {
                GlobalKind::Function(_) => None,
                GlobalKind::Variable(variable) => Some(layout.alloc_size(&module.context.types, variable.ty)),
            };
            by_id.insert(
                id.0,
                Global {
                    name: crate::data::symbol_name(platform, &name),
                    got: crate::isel::global_needs_got(module, platform, id),
                    align,
                    size,
                },
            );
        }
        Globals(Rc::new(by_id), Some(platform))
    }

    /// The linker's name of a routine the legalizer named (`memset`).
    pub fn external(
        &self,
        name: &str,
    ) -> String {
        self.1.map_or_else(|| name.to_owned(), |platform| crate::data::symbol_name(platform, name))
    }

    pub fn get(
        &self,
        id: u32,
    ) -> Option<&Global> {
        self.0.get(&id)
    }
}

//! What the DAG path reads of the module's globals, by the id a
//! `GlobalAddress` node carries: the linker's name, whether the address comes
//! from the GOT, and the alignment a pointer to it has.

use llrm_support::hash::HashMap;
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
        let mut by_id = HashMap::default();
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
                    got: global_needs_got(module, platform, id),
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

    /// The prefix of a label the linker never sees: `l` on Mach-O, `.L` on ELF.
    pub fn local_prefix(&self) -> &'static str {
        match self.1 {
            Some(Platform::Linux) => ".L",
            _ => "l",
        }
    }

    pub fn get(
        &self,
        id: u32,
    ) -> Option<&Global> {
        self.0.get(&id)
    }
}

/// Whether the module states "PIC Level" itself: a module with no flags at all
/// (the frontends', hand-written) is static code here, though
/// `Module::position_independent` takes it as position independent.
fn flagged_pic(module: &Module) -> bool {
    module.named_metadata.iter().any(|(name, _)| name == "llvm.module.flags") && module.position_independent()
}

/// Whether an access to `global` goes through the GOT.
pub(crate) fn global_needs_got(
    module: &Module,
    platform: Platform,
    global: GlobalId,
) -> bool {
    use llrm_mir::{GlobalKind, Linkage};
    let global = module.global(global);
    let defined = match &global.kind {
        GlobalKind::Function(function) => !function.is_declaration(),
        GlobalKind::Variable(variable) => variable.initializer.is_some(),
    };
    let local = matches!(global.linkage, Linkage::Internal | Linkage::Private);
    // Darwin reaches anything not defined here through the GOT; Linux static
    // code reaches all of it directly, and position-independent code (the
    // module's "PIC Level" flag) goes through the GOT for what is not defined
    // here, as the pin's `shouldAssumeDSOLocal` (clang -fPIE marks a definition
    // `dso_local`).
    // A hidden or protected declaration binds inside the module: the pin loads
    // it directly (`shouldAssumeDSOLocal`), not through the GOT, at Linux
    // PIE.
    let bound_here = global.visibility != llrm_mir::module::Visibility::Default;
    (platform == Platform::Darwin || (flagged_pic(module) && !bound_here)) && !local && !defined
}

//! A program: the modules compiled together, resolved by name and never
//! merged, and what they share -- one datalayout, the target, and where
//! the segments put data and the stack. A module's analyses and passes
//! read the shared state through `ProgramProxy`.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::context::GlobalId;
use crate::datalayout::DataLayout;
use crate::module::{GlobalValue, Module};
use crate::target::Machine;

/// DGROUP as the program links it: its member segments, the addresses cut
/// in each, and which are COMMON. The BC route fills it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DataGroup {
    pub members: Vec<String>,
    pub landmarks: BTreeMap<String, BTreeSet<i64>>,
    pub common: BTreeSet<String>,
}

/// Where the program's segments put its data and its stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentLayout {
    /// The address space of the data group, which a near pointer reaches
    /// through DS.
    pub data_space: u32,
    /// The allocas' address space, which reaches the stack through SS.
    pub stack_space: u32,
    /// SS = DS: the stack lies in the data group.
    pub stack_in_data: bool,
    pub data_group: DataGroup,
}

impl SegmentLayout {
    /// The segments of a program laid out as `layout` states, SS = DS.
    pub fn of(layout: &DataLayout) -> Self {
        Self { data_space: 0, stack_space: layout.alloca_space, stack_in_data: true, data_group: DataGroup::default() }
    }

    /// Whether every pointer in `space` reaches program data: the data
    /// group's or the stack's.
    pub fn program_data(&self, space: u32) -> bool {
        space == self.data_space || space == self.stack_space
    }
}

pub struct Program {
    pub modules: Vec<Module>,
    pub layout: DataLayout,
    pub target: Rc<dyn Machine>,
    pub segments: SegmentLayout,
}

impl Program {
    /// `modules` compiled together for `target`, under the one datalayout
    /// they state.
    pub fn new(modules: Vec<Module>, target: Rc<dyn Machine>) -> Result<Self, String> {
        let stated: BTreeSet<Option<&str>> = modules.iter().map(|one| one.datalayout.as_deref()).collect();
        if stated.len() > 1 {
            return Err(format!("modules state different datalayouts: {stated:?}"));
        }
        let layout = match stated.into_iter().next().flatten() {
            Some(text) => DataLayout::parse(text)?,
            None => DataLayout::default(),
        };
        let segments = SegmentLayout::of(&layout);
        Ok(Self { modules, layout, target, segments })
    }

    /// The global `name` resolves to: the module defining it, else the
    /// first declaring it.
    pub fn resolve(&self, name: &str) -> Option<(usize, GlobalId)> {
        let named = || self.modules.iter().enumerate().filter_map(|(at, module)| Some((at, module.named(name)?)));
        named().find(|&(at, id)| defines(self.modules[at].global(id))).or_else(|| named().next())
    }
}

fn defines(global: &GlobalValue) -> bool {
    global.function().is_none_or(|function| !function.is_declaration())
}

/// What a module's analyses and passes may read of its program, read-only.
#[derive(Clone)]
pub struct ProgramProxy {
    pub layout: DataLayout,
    pub target: Rc<dyn Machine>,
    pub segments: SegmentLayout,
}

impl ProgramProxy {
    /// A program of `module` alone for `target`.
    pub fn of(module: &Module, target: Rc<dyn Machine>) -> Rc<Self> {
        let layout = module.datalayout.as_deref().map_or_else(|| Ok(DataLayout::default()), DataLayout::parse).expect("a module's datalayout parses");
        Rc::new(Self { segments: SegmentLayout::of(&layout), layout, target })
    }
}

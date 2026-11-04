//! `BatchAA` for the DAG combiner: `WebAssemblySubtarget::useAA` is true, so
//! `DAGCombiner::mayAlias` asks alias analysis about two accesses whose
//! addresses it cannot compare ([WebAssemblySubtarget.cpp
//! L81](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblySubtarget.cpp#L81)).
//!
//! The verdict is `llrm_mir::alias::alias`: underlying objects, then `!tbaa`.

use std::cell::OnceCell;

use llrm_codegen::backend::selectiondag::MemLocation;
use llrm_codegen::backend::selectiondag::chain_combine::AliasOracle;
use llrm_mir::alias::{Alias, Location, alias};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::memory::{Callees, callees};
use llrm_mir::module::{Function, Module};

pub struct IrAlias<'a> {
    pub module: &'a Module,
    pub function: &'a Function,
    pub layout: &'a DataLayout,
    callees: OnceCell<Callees>,
}

impl<'a> IrAlias<'a> {
    pub fn new(
        module: &'a Module,
        function: &'a Function,
        layout: &'a DataLayout,
    ) -> IrAlias<'a> {
        IrAlias { module, function, layout, callees: OnceCell::new() }
    }
}

impl AliasOracle for IrAlias<'_> {
    fn no_alias(
        &self,
        a: MemLocation,
        b: MemLocation,
    ) -> bool {
        let callees = self.callees.get_or_init(|| callees(self.module));
        let location =
            |one: MemLocation| Location { pointer: one.pointer, bytes: u64::from(one.bytes), tbaa: one.tbaa };
        alias(
            &self.module.context,
            self.layout,
            callees,
            &self.module.metadata,
            self.function,
            location(a),
            location(b),
        ) == Alias::No
    }
}

//! `BatchAA` for the DAG combiner: `WebAssemblySubtarget::useAA` is true, so
//! `DAGCombiner::mayAlias` asks alias analysis about two accesses whose
//! addresses it cannot compare ([WebAssemblySubtarget.cpp
//! L81](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/WebAssembly/WebAssemblySubtarget.cpp#L81)).
//!
//! The verdict is the pin's default pipeline over one `BatchAAResults`'s query
//! state: BasicAA (underlying objects, GEP offsets, `aliasPHI`, `aliasSelect`),
//! scoped no-alias, then type-based.

use std::cell::RefCell;

use llrm_analysis::aa::{AAInfo, AAQueryInfo, AAResults, AliasResult, LocationSize, MemoryLocation};
use llrm_analysis::aggregate::AggregateAA;
use llrm_analysis::memory::Unit;
use llrm_codegen::backend::selectiondag::MemLocation;
use llrm_codegen::backend::selectiondag::chain_combine::AliasOracle;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{Function, Module};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::underlying::get_underlying_object;

pub struct IrAlias<'a> {
    unit: Unit<'a>,
    /// `BatchAAResults`: the answers of one function's queries; the IR does not
    /// change while the DAG is combined.
    batch: RefCell<AAQueryInfo>,
}

impl<'a> IrAlias<'a> {
    pub fn new(
        module: &'a Module,
        function: &'a Function,
        layout: &'a DataLayout,
    ) -> IrAlias<'a> {
        IrAlias { unit: Unit::of(module, layout, function), batch: RefCell::new(AAQueryInfo::default()) }
    }
}

impl AliasOracle for IrAlias<'_> {
    fn no_alias(
        &self,
        a: MemLocation,
        b: MemLocation,
    ) -> bool {
        let location = |one: MemLocation| MemoryLocation {
            ptr: one.pointer,
            size: LocationSize::Precise(u64::from(one.bytes)),
            aa_info: AAInfo { tbaa: one.tbaa, ..AAInfo::default() },
        };
        let mut stack = AggregateAA::default_pipeline(&self.unit);
        stack.alias(&location(a), &location(b), &mut self.batch.borrow_mut(), None) == AliasResult::NoAlias
    }

    fn same_underlying_object(
        &self,
        locations: &[MemLocation],
    ) -> bool {
        let query = Query {
            globals: self.unit.globals,
            ..Query::bare(self.unit.context, self.unit.function, self.unit.layout)
        };
        let mut objects = locations.iter().map(|one| get_underlying_object(&query, one.pointer, 0));
        let first = objects.next();
        objects.all(|object| Some(object) == first)
    }
}

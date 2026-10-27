# Cross-cutting cost centers (w-6, parity 8eb24e4b2d)

Self Ir by symbol, not by pass, from the d_faces -O2 callgrind of the ledger command (dist, real `-o`) against the pinned clang's on the same input
(`tools/ledger/costcenters.py`; a function belongs to the one center its own name matches, generics stripped; two matching is an error, not first-wins; centers and "other" sum to the total).

## qbq-llrm
| cost center | ours M | % | pin M | % |
|---|--:|--:|--:|--:|
| SipHash (DefaultHasher, RandomState) | 5.0 | 0.49 | 0.0 | 0.00 |
| FxHash / other Hasher::write | 1.7 | 0.17 | 3.0 | 0.59 |
| hash table probing (hashbrown / DenseMap, StringMap) | 38.2 | 3.77 | 33.5 | 6.59 |
| malloc / free / realloc | 6.6 | 0.66 | 14.8 | 2.90 |
| memcpy / memmove / memset / memcmp | 39.3 | 3.88 | 9.1 | 1.79 |
| drop_in_place / Drop | 0.0 | 0.00 | 0.0 | 0.00 |
| Rc / RefCell / Cell borrow | 2.3 | 0.23 | 0.0 | 0.00 |
| Vec / SmallVector growth | 23.3 | 2.30 | 1.1 | 0.22 |
| bounds / panic paths | 0.0 | 0.00 | 0.0 | 0.00 |
| string formatting / printing | 5.2 | 0.51 | 1.9 | 0.37 |
| other | 891.2 | 87.99 | 445.6 | 87.53 |
| total | 1012.8 | 100 | 509.0 | 100 |

cost-center check: 0 symbols in two centers over 8202 ours + 16974 pin symbols

SipHash (DefaultHasher, RandomState): llrm_transforms::reassociate::digest 1.13; <llrm_mir::opcode::Attribute as core::hash::Hash>::hash_slic 0.85; <hashbrown::raw::RawTable<(u32, alloc::sync::Arc<llrm_codegen:: 0.45; llrm_transforms::settled::slice 0.37; <llrm_mir::passes::Analyses>::invalidate 0.29
FxHash / other Hasher::write: <llrm_c::translate::Types>::of 0.33; <llrm_transforms::earlycse::Walk>::run 0.25; <llrm_mir::context::Context>::constant 0.25; <hashbrown::raw::RawTable<((&str, &str, &str), core::option: 0.15; <llrm_mir::types::Types>::intern 0.14
hash table probing (hashbrown / DenseMap, StringMap): <llrm_mir::lazyvalueinfo::LazyValueInfoCache>::insert_result 1.54; <llrm_codegen::model::lir::LirBody>::with_blocks 1.19; <indexmap::map::IndexMap<llrm_mir::module::InstId, (core::op 1.01; llrm_codegen::backend::twoaddr::tied 0.83; <indexmap::map::IndexMap<(i64, i64), u32, core::hash::BuildH 0.80
malloc / free / realloc: <alloc::raw_vec::RawVecInner>::finish_grow 20.39; _mi_malloc_generic 0.98; <hashbrown::raw::RawTableInner>::fallible_with_capacity::<al 0.68; llrm_codegen::backend::allocate::live_rows_numbered::<llrm_codegen 0.55; <llrm_analysis::scev::ScalarEvolution>::new 0.17
memcpy / memmove / memset / memcmp: _mi_theap_realloc_zero 11.88; <llrm_mir::facts::Fact>::of_attribute 1.93; __rustc::__rust_realloc 1.62; _mi_page_malloc_zero 1.10; <llrm_codegen::backend::isel::Selector>::candidates 0.96
drop_in_place / Drop: core::ptr::drop_glue::<toml::value::Value> 0.00; llrm_codegen::backend::regclass::collected 0.00; <llrm_transforms::pipeline::Run>::slice 0.00; <llrm_transforms::pipeline::Fixed as llrm_mir::passes::Funct 0.00; <core::cell::once::OnceCell<std::collections::hash::map::Has 0.00
Rc / RefCell / Cell borrow: <llrm_analysis::manager::Annotated as llrm_mir::passes::Anal 3.90; llrm_codegen::backend::peep::guards::upper_zero 3.82; llrm_codegen::backend::peep::guards::dead 3.01; llrm_analysis::induction::_entered 2.71; llrm_codegen::backend::liverangeshrink::shrunk 1.27
Vec / SmallVector growth: <llrm_mir::facts::Facts>::of 2.13; <indexmap::map::IndexMap<llrm_mir::module::InstId, llrm_anal 2.01; <llrm_mir::module::Function>::create_instruction 1.36; llrm_c::compile::selected_checking 1.33; <alloc::vec::Vec<llrm_mir::module::InstId> as alloc::vec::sp 1.19
string formatting / printing: <llrm_transforms::pipeline::Fixed>::fixed 0.76; llrm_transforms::settled::slice 0.68; <llrm_mir::module::Function>::unique_name 0.61; llrm_omf::write::write 0.60; llrm_codegen::backend::masm::addressed_pass 0.59


## QCport
| cost center | ours M | % | pin M | % |
|---|--:|--:|--:|--:|
| SipHash (DefaultHasher, RandomState) | 3.3 | 0.42 | 0.0 | 0.00 |
| FxHash / other Hasher::write | 1.5 | 0.18 | 2.6 | 0.61 |
| hash table probing (hashbrown / DenseMap, StringMap) | 27.5 | 3.49 | 26.0 | 6.11 |
| malloc / free / realloc | 5.4 | 0.69 | 12.4 | 2.91 |
| memcpy / memmove / memset / memcmp | 29.6 | 3.75 | 8.0 | 1.88 |
| drop_in_place / Drop | 0.0 | 0.00 | 0.0 | 0.00 |
| Rc / RefCell / Cell borrow | 1.7 | 0.22 | 0.0 | 0.00 |
| Vec / SmallVector growth | 17.3 | 2.20 | 1.0 | 0.23 |
| bounds / panic paths | 0.0 | 0.00 | 0.0 | 0.00 |
| string formatting / printing | 3.9 | 0.50 | 1.8 | 0.42 |
| other | 698.3 | 88.55 | 374.0 | 87.85 |
| total | 788.6 | 100 | 425.8 | 100 |

cost-center check: 0 symbols in two centers over 7980 ours + 17049 pin symbols

SipHash (DefaultHasher, RandomState): llrm_transforms::reassociate::digest 0.85; <llrm_mir::opcode::Attribute as core::hash::Hash>::hash_slic 0.46; <hashbrown::raw::RawTable<(u64, ())>>::reserve_rehash::<hash 0.24; <hashbrown::raw::RawTable<(u32, alloc::sync::Arc<llrm_codegen:: 0.24; llrm_transforms::settled::slice 0.21
FxHash / other Hasher::write: <llrm_c::translate::Types>::of 0.29; <llrm_mir::context::Context>::constant 0.22; <llrm_transforms::earlycse::Walk>::run 0.20; <hashbrown::raw::RawTable<((&str, &str, &str), core::option: 0.15; <llrm_mir::types::Types>::intern 0.12
hash table probing (hashbrown / DenseMap, StringMap): <llrm_mir::lazyvalueinfo::LazyValueInfoCache>::insert_result 1.18; <indexmap::map::IndexMap<llrm_mir::module::InstId, (core::op 0.89; <llrm_codegen::model::lir::LirBody>::with_blocks 0.88; llrm_codegen::backend::twoaddr::tied 0.65; <indexmap::map::IndexMap<(i64, i64), u32, core::hash::BuildH 0.64
malloc / free / realloc: <alloc::raw_vec::RawVecInner>::finish_grow 15.89; _mi_malloc_generic 0.80; <hashbrown::raw::RawTableInner>::fallible_with_capacity::<al 0.47; llrm_codegen::backend::allocate::live_rows_numbered::<llrm_codegen 0.33; llrm_analysis::alias::calls_by_attributes::{closure#4} 0.11
memcpy / memmove / memset / memcmp: _mi_theap_realloc_zero 8.79; __rustc::__rust_realloc 1.40; <llrm_mir::facts::Fact>::of_attribute 0.94; <llrm_codegen::backend::isel::Selector>::candidates 0.80; _mi_page_malloc_zero 0.70
drop_in_place / Drop: core::ptr::drop_glue::<toml::value::Value> 0.00; <core::cell::once::OnceCell<std::collections::hash::map::Has 0.00; llrm_codegen::backend::regclass::collected 0.00; <llrm_transforms::pipeline::Fixed as llrm_mir::passes::Funct 0.00; <alloc::sync::Arc<std::sys::fs::unix::InnerReadDir>>::drop_s 0.00
Rc / RefCell / Cell borrow: llrm_codegen::backend::peep::guards::upper_zero 2.90; <llrm_analysis::manager::Annotated as llrm_mir::passes::Anal 2.15; llrm_analysis::induction::_entered 2.14; llrm_codegen::backend::peep::guards::dead 1.92; llrm_codegen::backend::liverangeshrink::shrunk 1.03
Vec / SmallVector growth: <llrm_mir::module::Function>::create_instruction 1.29; llrm_c::compile::selected_checking 1.25; <llrm_mir::facts::Facts>::of 0.99; <indexmap::map::IndexMap<llrm_mir::module::InstId, llrm_anal 0.98; <llrm_mir::module::Function>::log 0.91
string formatting / printing: <llrm_mir::module::Function>::unique_name 0.55; llrm_transforms::settled::slice 0.51; llrm_omf::write::write 0.47; llrm_codegen::backend::masm::addressed_pass 0.35; llrm_codegen::backend::classes::iced 0.35


## What the residual is made of (qbq-llrm, parity 26d6c9e9a, `tools/ledger/residual.py`)
The residual of the ledger is 102.6 M with the front end taken out; under `driver::compiled` 119.7 M lies outside every listed pass (the profile's share of
`compiled`, which holds the HIR translation too). By module, M: `llrm_codegen::backend` 24.0 (passes the buckets do not list: saverestore/shrinkwrap,
machinelicm, relayout, combiner, machinesched, fixupsetcc: an instrument gap, they have pin counterparts), alloc/hash/core/std 13.0, `llrm_mir::module` 11.9,
`globalsmodref` 9.2 + `globalsaa` 2.9 (GlobalsAA computed outside the pass runs, 7 times; the pin computes it once per RecomputeGlobalsAAPass),
`llrm_hir::mir` 6.4, libc mem* 5.1, `llrm_mir::opcode` 2.9, `cfg` 2.6, `callgraph` 2.3, the rest below 2 M each.
Steps (w-6): list the backend passes in their pin buckets (11-06); GlobalsAA once (11-13).

## Backend passes now in their buckets (w-6, parity 69b60cafc)
The residual held 24 M of backend passes no bucket listed (`machinesched`, `latecleanup`, `machinecp`, `machinelicm`, `liverangeshrink`, `fixupsetcc`, `combiner`, `fixupbw`, `pushfold`, `cmpsub`, `leaopt`, `fixupleas`, `flagscopy`, `callframe`, `optimizephis`, `processimpdefs`). The pin's side already listed its counterparts, so the clean-up ratio read low. `ledger4.py` lists them and refuses any `LIRTransform` of 0.1 M or more that no bucket lists. qbq at 69b60cafc, ours/pin M: clean-up 143.6/49.4 (2.91x) -> 172.3/49.4 (3.48x); regalloc 145.4/67.2 -> 146.5/67.3; residual 103.1 -> 73.4; total 943.1 unchanged. QCport: clean-up 101.8/42.5 -> 123.2/42.5 (2.90x), residual 70.9 -> 48.4.

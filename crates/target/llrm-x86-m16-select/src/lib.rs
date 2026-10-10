//! x86-m16's instruction selection: its selector (`SELECTOR`), the peephole
//! rules that go with it (`RULES`) and its effect rows (`rows`), generated
//! by build.rs from `llrm-x86-m16`'s definition directory. `llrm-driver` binds
//! them; the target's data crate stays free of `llrm-core`.

include!(concat!(env!("OUT_DIR"), "/select.rs"));

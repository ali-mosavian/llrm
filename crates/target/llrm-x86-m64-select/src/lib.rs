//! x86-64's instruction selection: its selector (`SELECTOR`), the peephole
//! rules that go with it (`RULES`), generated
//! by build.rs from `llrm-x86-m64`'s definition directory. `llrm-driver` binds
//! them; the target's data crate stays free of `llrm-codegen`.

include!(concat!(env!("OUT_DIR"), "/select.rs"));

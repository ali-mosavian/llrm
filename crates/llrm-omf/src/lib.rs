//! OMF object files: records, the code segment as a module, CodeView debug info.

pub mod addends;
pub mod cvinfo;
pub mod module;
pub mod omf;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

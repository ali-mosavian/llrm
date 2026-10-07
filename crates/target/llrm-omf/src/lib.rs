//! OMF object files: records, the code segment as a module, CodeView debug info.

pub mod addends;
pub mod codeview;
pub mod cvinfo;
pub mod cvwrite;
pub mod module;
pub mod omf;
pub mod write;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

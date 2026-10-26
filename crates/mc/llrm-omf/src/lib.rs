//! OMF object files: records, the code segment as a module, CodeView debug
//! info.

/// CodeView 4, its older BASIC dialect and Turbo Debugger's records name one
/// place for a whole scope.
pub const LOCATION_RANGES: bool = false;
pub const CFA_LOCATIONS: bool = false;

pub mod addends;
pub mod codeview;
pub mod cv4;
pub mod cv4info;
pub mod cvinfo;
pub mod cvwrite;
pub mod module;
pub mod omf;
pub mod td;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod write;

//! Machine-code layout, symbols, fixups, and encoding.

mod layout;
mod model;
mod verify;

pub use layout::{
    AlignmentOwner, FragmentLayout, LayoutError, MCLayout, SymbolLayout, layout,
};
pub use model::*;
pub use verify::verify;

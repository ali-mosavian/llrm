//! LLVM's register-allocation structures over the translated live ranges of
//! [`crate::liveness`], translated from llvm/lib/CodeGen at d1106deb71cc:
//! `LiveRegMatrix` and `VirtRegMap` with the target's register units.

pub mod matrix;
pub mod virtregmap;

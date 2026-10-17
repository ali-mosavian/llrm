//! LLVM's greedy register allocator, translated from llvm/lib/CodeGen at
//! d1106deb71cc (RegAllocBase, RegAllocGreedy, RegAllocEvictionAdvisor,
//! LiveRegMatrix, VirtRegMap, AllocationOrder), over the translated live range
//! structures of [`crate::liveness`]. Shadow mode: it runs beside the
//! allocator in `backend::allocate` and is compared with it; nothing selects it
//! yet.

pub mod matrix;
pub mod virtregmap;

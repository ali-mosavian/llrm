//! LLVM's greedy register allocator, translated from llvm/lib/CodeGen at
//! d1106deb71cc (RegAllocBase, RegAllocGreedy, RegAllocEvictionAdvisor,
//! LiveRegMatrix, VirtRegMap, AllocationOrder), over the translated live range
//! structures of [`crate::liveness`]. Shadow mode: it runs beside the
//! allocator in `backend::allocate` and is compared with it; nothing selects it
//! yet.

pub mod edgebundles;
pub mod greedy;
pub mod hoistspill;
pub mod inlinespiller;
pub mod interferencecache;
pub mod liverangeedit;
pub mod matrix;
pub mod regionsplit;
pub mod remat;
pub mod shadow;
pub mod spill;
pub mod spillplacement;
pub mod spliteditor;
pub mod splitkit;
pub mod tii;
pub mod virtregmap;

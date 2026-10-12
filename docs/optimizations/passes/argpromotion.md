> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/argpromotion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/argpromotion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/argpromotion.rs sha256:a7af83e40d476a7356bf724377bd2c0c56f7ed8eb94fd1161edbd23914537177 -->

# argpromotion

LLVM's ArgumentPromotion: a function only direct calls reach takes, in place
of a pointer it only reads, the fields it reads through it. Each caller
loads them before the call; a recursive call passes its own on. `&mut [T]`
in Nib, a struct by pointer in C, a BASIC array descriptor are one case.

A parameter is promoted when every use of it is a load of a fixed offset (or
the same parameter passed on to the function itself), the loads are safe
where the callers make them (`dereferenceable` covers the fields) and read
what nothing writes while the function runs (`noalias readonly`, or a
function that writes nothing), and there are few of them. Priced by the
target for the level being built: the loads each caller gains against the
loads the function loses, in clocks or in bytes.

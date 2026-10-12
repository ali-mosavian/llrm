> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/exactaddress.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/exactaddress.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/exactaddress.rs sha256:1752f3609409745f0fc07c3bfb40bd1e33dfe199f10450ad382c533fd8c6700f -->

# exactaddress

Fold the chain computing an exact address into its 32-bit form.

Where lowering proved a cell exact (`Mem::exact`), the 16-bit sum of its
registers equals the 32-bit sum of the leaves of the affine chain
(`affine::step`) that computed them, once those leaves' upper halves are
zero (`upperzero`): `mov si,ax; shl si,1; add cx,S[si]` becomes
`add cx,S[eax*2]` behind the 67h prefix, and the chain goes. A far cell
keeps its origin as the base; only its offset folds. Where the access's
loop writes a leaf only below its upper half, one `movzx` in the loop's
preheader makes it zero for every trip.

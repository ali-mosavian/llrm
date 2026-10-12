> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/sharedstores.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/sharedstores.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/sharedstores.rs sha256:b5c36cdbe6de272a2b2565e04144f134904235a05715720684a606ad06ddada2 -->

# sharedstores

Tuned for size, a run of stores of one literal is the literal in a dead
register, loaded once, then register stores: `mov dword ptr [bp-8],0` is 8
bytes, `mov [bp-8],eax` 4, and the one `xor eax,eax` 3. Two stores or more
(one dword zero, too) are fewer bytes; the load costs a clock, so -Os only.

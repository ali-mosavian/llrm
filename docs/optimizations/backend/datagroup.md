> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/datagroup.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/datagroup.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/datagroup.rs sha256:a1844fd44f9510eaa423ed105eefc1b1f62fc92187ed3bc4b40901cfb8ed6331 -->

# datagroup

The data segment register as a fourth selector register.

Where the machine's stack lives in the data group, an access to the data
group can go through the stack segment instead. Allocation may then place
a selector in the data segment register between the points that need it to
hold the data group (`target::needs_data_group`); this pass makes the rest
of the body agree: every access that would read the data group through it
is prefixed with the stack segment while it may hold something else, and
it is restored before each point that needs it and at each exit.

The runtime's interrupt handlers load their own data segment, so a value
held here is safe from them.

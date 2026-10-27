# Pass documentation sources

Programs whose compiler output is quoted in the examples of source headers.

| File | Command | Header |
| --- | --- | --- |
| `spares.c` | `llrm-c -O2 -m16 --dump DIR -S spares.c`; stages `19-addresssink.ll`, `20-spares.ll` | `crates/opt/llrm-transforms/src/spares.rs` |
| `calleefacts.nib` | `ISEL_DUMP=1 llrm-nib -O2 -fno-inline-functions -fno-inline-functions-called-once -fno-ipa-cp-clone -march=i386 -S calleefacts.nib`; stage `FarIndirectCalls` | `calleefacts.rs`, `callregs.rs` in `crates/codegen/llrm-codegen/src/backend/` |

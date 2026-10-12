> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/postings.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/postings.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/postings.rs sha256:75bb97df55f548049edb1866797f4ffbcc092b95127b379e23c0b2c3c9b55ed2 -->

# postings

Where each value is defined and read in a body, kept as the body changes.

A spill or split rewrites the instructions that name its values and keeps
every other `Arc<Insn>`, so the answers of the spiller's scans, which look
for the few values a rewrite is about, need not walk the body: they read the
occurrences of those values. The postings follow the body the allocator asks
about: asked of a body, they compare each block's instructions by identity
with the body they were made of and redo only the blocks that differ (all of
them, for another function).

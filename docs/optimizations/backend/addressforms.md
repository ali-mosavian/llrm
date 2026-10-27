> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/addressforms.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/addressforms.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/addressforms.rs sha256:bc17362a87896e2afc57c6f704d9d83472f66d848da6258f46e79b4581389c88 -->

# addressforms

Widens word definitions so a 67h address can take them as dword
components.

## How it works
The selector folds `base + index*scale` of word values into one
32-bit, address-size-prefixed form such as `[ebx+esi*4]` (`widened` in
`isel.rs`). That form reads whole dwords, so each word value it names
must be zero above bit 15. `promote` makes it so at the definition and
adds no register: the value keeps its one definition and its live range.

Two definitions can be widened. A plain `mov r16,m16` or `mov r16,r16`
becomes `movzx r32,...`. A far-pointer load (`les`, `lfs`, `lgs`) must
keep its form, so its offset goes to a fresh word temporary and a
`movzx` into the original value follows it. Any other definition is an
error: the selector only marks values that are one of these.

```text
 plain load                far-pointer load
 v1:2 = mov m16            v1:2, sel = les m32
    |                         |
    v                         v
 v1:4 = movzx m16          t:2, sel = les m32
                           v1:4 = movzx t:2
```

Definitions are rewritten from the last in each block to the first, so
inserting a follower does not move one still waiting.

## Example
No honest example: no dump or test prints the body before and after
`promote`. `isel_tests.rs`
`test_a_non_negative_typed_index_is_scaled_in_the_67h_form` asserts the
`movzx` it makes and the `[ebx+eax*2]` form that reads it.

## Used by
`isel.rs`, after a function is selected, for the values the selector
marked; with none marked the blocks pass through unchanged.

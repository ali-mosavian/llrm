> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/arrival.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/arrival.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/arrival.rs sha256:8c62e2c0e446fb8942635d370f54af78cee4aec85cbfdccebd0ed0ef41b51fbf -->

# arrival

Finds how long a parameter stays in the register its convention passed it
in, from the machine code.

## How it works
A parameter arrives in a register and the function stores it into its
frame home. Until that store the register, not the home, holds the value,
which the debug location of the parameter must say. `stored` decodes the
function's bytes from the entry along the straight line and returns the
offset after the first `mov [cell], register` that writes the home.

It gives up (`None`) on the first branch, return or invalid code, on any
write to the register before the store (it no longer holds the argument),
and when nothing stores it. The home is a `Cell`: `Frame`, reached
through the frame register at a displacement, or `Stack`, where the code
keeps no frame register and the cell is `from_cfa` bytes from the
canonical frame address. For `Stack` the stack pointer's depth is tracked
through pushes, pops and constant `add`/`sub`, so `[esp+4]` after two
pushes is matched with the cell it names; any other write to the stack
pointer ends the search.

```text
 push ebp ; mov ebp,esp ; sub esp,8 ; mov [ebp-4],eax ; ...
 ^ entry                                                ^ eax is in the
   eax holds the argument                                 home from here
```

## Example
A function that stores `eax` and `edx` to its frame cells (`stored` returns
the offset after the store):

```text
 0  55            push ebp
 1  89 E5         mov  ebp,esp
 3  83 EC 08      sub  esp,8
 6  89 45 FC      mov  [ebp-4],eax      stored(cell ebp-4, eax) = Some(9)
 9  89 55 F8      mov  [ebp-8],edx      stored(cell ebp-8, edx) = Some(12)
12  C3            ret                   stored(cell ebp-8, eax) = None
```

From unit test
`the_first_store_of_the_arrival_register_into_the_home_ends_its_stay_in_the_register`
in
`arrival.rs`.

## Used by
`debuginfo.rs`, for 32-bit `-g` output: the parameter's location list is
the register up to the returned offset and the home after it.

## See also
`cfi.rs`, which reads the same code for the frame rule.

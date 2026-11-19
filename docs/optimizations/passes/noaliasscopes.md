> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/noaliasscopes.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/noaliasscopes.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/noaliasscopes.rs sha256:1a834e122edca7353d72ddaef061067dead49fac2c53823760357207fe8d186a -->

# noaliasscopes

`cloneAndAdaptNoAliasScopes`
([CloneFunction.cpp#L1213](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CloneFunction.cpp#L1213)):
a pass that copies blocks gives the copy its own scopes for those declared
inside the copied region.

## How it works
A `llvm.experimental.noalias.scope.decl` states a scope that starts there
and lives for one run of the code around it; a copy of that code is another
run, so its accesses must not share the scope with the original's. MIR
has no metadata call operand: the declaration is a call carrying its scope
list as `!noalias.scope.decl` ([`crate::inlinemetadata::DECL`](inlinemetadata.md)).
`declared_scopes` reads the lists of the declarations in the region
(`identifyNoAliasScopesToClone`); `cloned_and_adapted` makes a new scope
for each (the same domain, the name and `:ext`) and replaces the old one
in the `!alias.scope`, `!noalias` and declaration lists of the copies.

## Example
A loop declaring scope `!2` and unrolled twice: the copy's accesses and
declaration name a clone of `!2`.

## Provides
`declared_scopes`, `cloned_and_adapted`.

## Needs
[`crate::inlinemetadata`](inlinemetadata.md) for the attachment kinds.

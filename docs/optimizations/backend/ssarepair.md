> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/ssarepair.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/ssarepair.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/ssarepair.rs sha256:7e525218c7f301f5ddab83b9f1144b72d866675889164803acb114cf48dc4430 -->

# ssarepair

SSA after a spiller gave some values more than one definition: a reload
brings a value back, and every use after it must read the reload, or a phi
where two definitions meet. The classic construction: phis at the
iterated dominance frontier of the definitions, pruned where the value is
not live in, then renaming down the dominator tree (Cytron et al.; Braun
et al. 2013 reach the same result on the fly).

A definition is a reload when the instruction says so (`spill_reload` or
`rematerialized`) and defines one of `redefined`; the first definition of
each value keeps its name, and each reload is named afresh. A block that
holds the value in no register on entry (`held`) reloads it before every
use, so it needs no phi for it.

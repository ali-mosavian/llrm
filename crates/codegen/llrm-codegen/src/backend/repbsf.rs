//! `bsf` gets the `rep` prefix (the TZCNT encoding, which processors without
//! `tzcnt` run as the same `bsf`) unless the function is optimised for size or
//! the instruction's flags are read.
//!
//! LLVM's `X86MCInstLower` `BSF*` case sets `IP_HAS_REPEAT` when
//! `!hasOptSize() && EFLAGS is dead`
//! ([L516](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86MCInstLower.cpp#L516)).
//! `Profile::size` stands for `hasOptSize`.
//!
//! # Example
//! With the flags dead and the function built for speed:
//!
//! ```text
//!  before                after
//!  bsf eax, ecx          rep bsf eax, ecx     (tzcnt's encoding)
//!  cmp esi, 0            cmp esi, 0
//! ```
//!
//! Built for size, or with the flags read (`adc` next), the `bsf` is left as it
//! is. From unit test
//! `test_bsf_gets_the_rep_prefix_only_where_the_pin_gives_it`
//! in `peephole_tests.rs`, which runs the three cases.

use std::sync::Arc;

use crate::backend::cpu::Profile;
use crate::backend::peephole::{_flags_dead_after, _flags_live_out, id};
use crate::model::ir::Semantics;
use crate::model::lir::{Insn, LirBody};

/// `body` with the `bsf` whose flags are dead marked `repeat`.
pub fn marked(
    body: &LirBody,
    cpu: &Profile,
) -> LirBody {
    let is_bsf = |one: &Insn| one.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("bsf"));
    if cpu.size || !body.blocks.iter().any(|block| block.insns.iter().any(|one| is_bsf(one))) {
        return body.clone();
    }
    let flags_out = _flags_live_out(body);
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let dead_out = flags_out.entering.get(&block.at).is_some_and(|lanes| lanes.is_empty());
            let dead = _flags_dead_after(body.regs(), body.bits, block, dead_out);
            let insns = block
                .insns
                .iter()
                .map(|one| {
                    if is_bsf(one) && dead.contains(&id(one)) {
                        let what = Semantics { repeat: true, ..one.what.clone().expect("a bsf") };
                        Arc::new(Insn { what: Some(what), ..(**one).clone() })
                    } else {
                        Arc::clone(one)
                    }
                })
                .collect();
            block.with_insns(insns)
        })
        .collect();
    body.with_blocks(blocks)
}

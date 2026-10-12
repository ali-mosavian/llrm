//! The peephole's rule engine: the matchers generated from `peephole.peep`
//! and the walks, guards and makers they call.
//!
//! # How it works
//! A target's rules are text. The x86 family's `peephole.peep`
//! (`crates/target/llrm-x86`) holds the groups every x86 target has, and a
//! target adds its own beside it (`llrm-x86-m16` adds real mode's word pushes
//! and far loads). `build.rs` compiles each group into one decision automaton
//! and emits a [`Rules`] for the target with one field per group, `None` where
//! the target has none. `groups.list` names the groups the schedule runs and
//! the one form of each: `body` rewrites a whole body (`peephole`), `insns`
//! a run of instructions (instruction selection, `isel/combined`). The build
//! refuses a group the `.peep` has that is not listed, or lacks its form.
//!
//! A rule matches instructions as LIR spells them (destinations, then
//! sources), binds operands, tests guards and rewrites:
//!
//! ```text
//!   rule loaded_extension
//!     match   mov m:reg, src:mem ; ... ; x=(movsx|movzx) d:reg, m
//!     when    free(@0, widening), same_width(m, src), feeds_once(@0, @1)
//!     rewrite @0: x d, src {defines: ..., uses: ...} ; anchor @1 {...}
//! ```
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   peep["<b>peephole.peep</b>"]:::mid
//!   m["<b>Matcher</b><br/><small>head, tail, cross</small>"]:::mid
//!   walks["<b>walk.rs</b><br/><small>each, window, slide, gap</small>"]:::mid
//!   g["<b>guards.rs</b><br/><small>when</small>"]:::mid
//!   mk["<b>make.rs</b><br/><small>rewrite</small>"]:::mid
//!   peep -->|"build.rs"| m
//!   m --> walks
//!   walks --> g
//!   walks --> mk
//! ```
//!
//! *From peephole.peep to the rewrites*
//!
//! A [`walk::Matcher`] has a `head` that tests the first instruction and names
//! a state, and a `tail` that tests the rest from there and runs the rules.
//! The group's `walk` line picks how blocks are traversed: [`walk`] has the
//! four walks and the facts a rule may ask for.
//!
//! [`field`] has one bit per piece of instruction metadata (`clobbers`,
//! `requires`, `spill_store`, ...). A rule may rewrite an instruction only if
//! `guards::free` finds the bits its `meta` set names clear, so a rewrite
//! never drops information an earlier phase attached. [`Set`] is a set of
//! mnemonics named in the `.peep`, and for a family such as `far_loads` the
//! fixed register that selects each member.
//!
//! # Example
//! The rule above is the text of `loaded_extension` in `peephole.peep`. The
//! engine has no LIR of its own to show: the rewrites it produces are in the `#
//! Example` of `crate::backend::peephole`.
//! # Used by
//! `peephole` calls `rewritten` for each `body` group, in a fixed order among
//! the hand-written sub-passes. `isel/combined` calls `rewritten_insns` for the
//! `insns` groups.

pub mod guards;
pub mod make;
pub mod walk;

use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::model::lir::{Insn, LirBody};

/// A set of mnemonics from `peephole.peep`, and for a family (`far_loads`)
/// the fixed register that picks each member.
pub struct Set {
    pub names: &'static [&'static str],
    pub fixed: &'static [(RegId, &'static str)],
}

impl Set {
    pub fn contains(
        &self,
        name: &str,
    ) -> bool {
        self.names.contains(&name)
    }

    pub fn by_fixed(
        &self,
        register: RegId,
    ) -> Option<&'static str> {
        self.fixed.iter().find(|(one, _)| *one == register).map(|(_, name)| *name)
    }
}

/// The metadata `guards::free` can require empty, one bit each.
pub mod field {
    pub const CLOBBERS: u32 = 1 << 0;
    pub const CLOBBERS_HIGH: u32 = 1 << 1;
    pub const REQUIRES: u32 = 1 << 2;
    pub const DELIVERS: u32 = 1 << 3;
    pub const SPREAD: u32 = 1 << 4;
    pub const GROUP: u32 = 1 << 5;
    /// `symbol` is not `Some(true)`.
    pub const SYMBOL: u32 = 1 << 6;
    pub const FRAME_ADJUST: u32 = 1 << 7;
    pub const SPILL_RELOAD: u32 = 1 << 8;
    pub const SPILL_STORE: u32 = 1 << 9;
    pub const DEFINES: u32 = 1 << 10;
    pub const USES: u32 = 1 << 11;
    /// `covers` is one point: the instruction owns no source bytes.
    pub const POINT: u32 = 1 << 12;
    /// `covers` is no range: absent or one point.
    pub const UNOWNED: u32 = 1 << 13;
    /// `volatile` is false.
    pub const VOLATILE: u32 = 1 << 14;
}

/// A rewrite of a whole body by one rule group.
pub type BodyRule = fn(&LirBody, &walk::Facts) -> LirBody;
/// A rewrite of a run of instructions by one rule group.
pub type InsnRule = fn(&[Arc<Insn>], &walk::Facts) -> Vec<Arc<Insn>>;

/// `body` rewritten by `rule`, unchanged where the target has none.
pub fn rewritten(
    rule: Option<BodyRule>,
    body: &LirBody,
    facts: &walk::Facts,
) -> LirBody {
    rule.map_or_else(|| body.clone(), |rule| rule(body, facts))
}

/// `insns` rewritten by `rule`, unchanged where the target has none.
pub fn rewritten_insns(
    rule: Option<InsnRule>,
    insns: &[Arc<Insn>],
    facts: &walk::Facts,
) -> Vec<Arc<Insn>> {
    rule.map_or_else(|| insns.to_vec(), |rule| rule(insns, facts))
}

/// No mnemonics.
pub static NO_NAMES: Set = Set { names: &[], fixed: &[] };

include!(concat!(env!("OUT_DIR"), "/peep_rules.rs"));

#[cfg(test)]
mod tests;

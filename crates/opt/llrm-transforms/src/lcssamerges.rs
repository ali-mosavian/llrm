//! Close a loop with several exits: a phi at each exit, merged by phis where
//! the exits' paths join.
//!
//! # How it works
//! `lcssa` calls it for a loop whose exits lead to more than one block. Every
//! exit block must have only predecessors inside the loop; otherwise nothing
//! changes.
//!
//! For each value defined in the loop and read outside it, the blocks that
//! read it (a phi's reading counts at the incoming edge's block) are walked
//! backwards through predecessors until a block is reached where the value is
//! available: an exit whose predecessors are all dominated by the definition.
//! Reaching the loop, the entry or a block without predecessors abandons that
//! value.
//!
//! Within the blocks so found, a block is a join when the exits reaching it
//! are more than one and it has several predecessors. One phi is made for each
//! exit and each join. An exit's phi takes the value on each loop edge, a
//! join's takes its predecessors' phis. Every reader outside the loop then
//! reads the phi supplied by the nearest exit or join above it.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   loop["<b>loop</b>"]:::mid
//!   e1["<b>E1</b>"]:::mid
//!   e2["<b>E2</b>"]:::mid
//!   j["<b>J</b><br/><small>use v</small>"]:::mid
//!   loop -->|"exit"| e1
//!   loop -->|"exit"| e2
//!   e1 --> j
//!   e2 --> j
//! ```
//!
//! *Before: v is used past the join J*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   loop["<b>loop</b>"]:::mid
//!   e1["<b>E1</b><br/><small>v1 = phi [v, loop]</small>"]:::mid
//!   e2["<b>E2</b><br/><small>v2 = phi [v, loop]</small>"]:::mid
//!   j["<b>J</b><br/><small>v3 = phi [v1, E1], [v2, E2]<br/>use v3</small>"]:::mid
//!   loop -->|"exit"| e1
//!   loop -->|"exit"| e2
//!   e1 --> j
//!   e2 --> j
//! ```
//!
//! *After: a phi per exit, one at the join*
//!
//! Everything is resolved to a supplier before any phi is created, so a
//! refused merge leaves the function untouched. Phi names take `.lcssa`.
//!
//! # Example
//! A loop with two exits, `b3` from the header and `b4` from the latch, that
//! join in `b5`, where `%carried` is read. Before:
//!
//! ```mir
//! ; ... b0, then:
//! b1:
//!   %carried = phi i16 [ %seed, %b0 ], [ %stepped, %b2 ]
//!   %more = icmp ult i16 %carried, %n
//!   br i1 %more, label %b2, label %b3
//!
//! b2:
//!   %stepped = add i16 %carried, 1
//!   %stay = icmp ult i16 %stepped, %m
//!   br i1 %stay, label %b1, label %b4
//!
//! b3:
//!   br label %b5
//!
//! b4:
//!   br label %b5
//!
//! b5:
//!   %answer = add i16 %carried, 7
//!   ret i16 %answer
//! ```
//!
//! After:
//!
//! ```mir
//! ; ... b0 to b2 unchanged, then:
//! b3:
//!   %carried.lcssa = phi i16 [ %carried, %b1 ]
//!   br label %b5
//!
//! b4:
//!   %carried.lcssa1 = phi i16 [ %carried, %b2 ]
//!   br label %b5
//!
//! b5:
//!   %carried.lcssa2 = phi i16 [ %carried.lcssa, %b3 ], [ %carried.lcssa1, %b4 ]
//!   %answer = add i16 %carried.lcssa2, 7
//!   ret i16 %answer
//! ```
//!
//! From unit test `test_distinct_exits_merge_before_the_downstream_use` in
//! `lcssamerges_tests.rs`.
//!
//! # Runs
//! Called from [`crate::lcssa`] for each multi-exit loop, so wherever `lcssa`
//! runs.
//!
//! # Needs
//! Dominators and predecessors computed from the function on entry.
//!
//! # See also
//! LLVM's `formLCSSA` (LCSSA.cpp), which closes each exit block separately.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::{cfg, ssa};
use llrm_mir::module::{Function, Operand, ValueId};

use crate::edges;
use crate::lcssa::{arms, exit_phi, from_arms, operations, outside_reads, place_phi};

/// What the closing of a function's loops reads of its blocks, found when first
/// asked: closing adds phis and moves no edge, so it stands for every loop.
#[derive(Default)]
pub struct Flow {
    pub dominance: Option<cfg::Dominance>,
    graph: Option<(Vec<cfg::Block>, llrm_support::graph::Predecessors)>,
}

impl Flow {
    /// A flow that reads the dominators the caller holds.
    pub fn with_dominance(dominance: cfg::Dominance) -> Self {
        Self { dominance: Some(dominance), graph: None }
    }
}

/// Whether anything changed.
pub fn closed(
    function: &mut Function,
    loop_: &Loop,
    exiting: &[(i64, i64)],
    flow: &mut Flow,
) -> Result<bool, String> {
    let (graph, predecessors) = flow
        .graph
        .get_or_insert_with(
            || {
                let graph = cfg::graph(function);
                let predecessors = loops::predecessors(&graph);
                (graph, predecessors)
            },
        );
    let (graph, predecessors) = (&*graph, &*predecessors);
    let entry = function.entry().map(cfg::id);
    let dominance = &*flow.dominance.get_or_insert_with(|| cfg::Dominance::of(function));
    let exits = exiting.iter().map(|&(_, to)| to).collect::<BTreeSet<_>>();
    if exits.iter().any(|at| {
        predecessors
            .get(at)
            .is_none_or(|parents| parents.is_empty() || !parents.iter().all(|at| loop_.body.contains(at)))
    }) {
        return Ok(false);
    }
    let reachable = graph.iter().map(|block| block.at).collect::<BTreeSet<_>>();
    let mut sites: BTreeMap<ValueId, (i64, BTreeSet<i64>)> = BTreeMap::new();
    for (value, home, reads) in outside_reads(function, &loop_.body) {
        for read in reads {
            let site = match read.from {
                Some(from) if loop_.body.contains(&from) => continue,
                Some(from) => from,
                None => read.home,
            };
            if reachable.contains(&read.home) {
                sites.entry(value).or_insert_with(|| (home, BTreeSet::new())).1.insert(site);
            }
        }
    }
    let mut changed = false;
    for (&value, (home, value_sites)) in &sites {
        let available = exits
            .iter()
            .copied()
            .filter(|at| predecessors[at].iter().all(|&parent| dominance.dominates(function, *home, parent)))
            .collect::<BTreeSet<_>>();
        let mut needed = BTreeSet::new();
        let mut pending = value_sites.iter().copied().collect::<Vec<_>>();
        let mut broke = false;
        while let Some(at) = pending.pop() {
            if needed.contains(&at) {
                continue;
            }
            if loop_.body.contains(&at) || Some(at) == entry || predecessors.get(&at).is_none_or(<[i64]>::is_empty) {
                broke = true;
                break;
            }
            needed.insert(at);
            if !available.contains(&at) {
                pending.extend(predecessors[&at].iter().copied());
            }
        }
        if !broke {
            let reached = available.intersection(&needed).copied().collect();
            changed |= _merged(function, value, &needed, &reached, &predecessors)?;
        }
    }
    Ok(changed)
}

/// `value` read through a phi at each of `exits` and at each join of their
/// paths within `needed`; whether it was.
pub fn _merged(
    function: &mut Function,
    value: ValueId,
    needed: &BTreeSet<i64>,
    exits: &BTreeSet<i64>,
    predecessors: &llrm_support::graph::Predecessors,
) -> Result<bool, String> {
    // The phis that reach each block. A join stands for the exits behind it: a
    // block below one that sees nothing else needs no phi (SSAUpdater makes a
    // phi only where the values differ, as in `formLCSSAForInstructionsImpl`:
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/LCSSA.cpp#L169-L262
    let mut joins = exits.clone();
    let reaching = loop {
        let mut reaching = needed
            .iter()
            .map(|&at| (at, if joins.contains(&at) { BTreeSet::from([at]) } else { BTreeSet::new() }))
            .collect::<BTreeMap<_, _>>();
        loop {
            let changed = needed
                .iter()
                .map(|&at| {
                    let now = if joins.contains(&at) {
                        reaching[&at].clone()
                    } else {
                        predecessors[&at].iter().flat_map(|parent| reaching[parent].iter().copied()).collect()
                    };
                    (at, now)
                })
                .collect::<BTreeMap<_, _>>();
            if changed == reaching {
                break;
            }
            reaching = changed;
        }
        if reaching.values().any(BTreeSet::is_empty) {
            return Ok(false);
        }
        let more = needed
            .iter()
            .copied()
            .filter(|at| !joins.contains(at) && reaching[at].len() > 1 && predecessors[at].len() > 1)
            .collect::<Vec<_>>();
        // One at a time, the one nothing else waiting lies above: a join above
        // may leave those below it with one supplier.
        let above = |from: i64| {
            let mut seen = BTreeSet::new();
            let mut pending = predecessors[&from].to_vec();
            while let Some(at) = pending.pop() {
                if needed.contains(&at) && seen.insert(at) {
                    pending.extend(predecessors[&at].iter().copied());
                }
            }
            seen
        };
        let Some(&first) = more.iter().find(|&&at| !above(at).iter().any(|up| more.contains(up))).or(more.first())
        else {
            break reaching;
        };
        joins.insert(first);
    };
    // Which join's phi each needed block reads.
    let mut supplier = joins.iter().map(|&at| (at, at)).collect::<BTreeMap<_, _>>();
    let mut pending = needed.difference(&joins).copied().collect::<BTreeSet<_>>();
    while !pending.is_empty() {
        let mut changed = BTreeSet::new();
        for &at in &pending {
            if reaching[&at].len() == 1 {
                let only = *reaching[&at].first().expect("one");
                supplier.insert(at, supplier[&only]);
            } else {
                let parents = &predecessors[&at];
                if parents.len() > 1 {
                    return Err("too many values to unpack (expected 1)".to_owned());
                }
                let Some(parent) = parents.first() else {
                    return Err("not enough values to unpack (expected 1, got 0)".to_owned());
                };
                let Some(&found) = supplier.get(parent) else {
                    continue;
                };
                supplier.insert(at, found);
            }
            changed.insert(at);
        }
        if changed.is_empty() {
            return Ok(false);
        }
        pending = pending.difference(&changed).copied().collect();
    }
    let phis = joins.iter().map(|&at| (at, exit_phi(function, value))).collect::<BTreeMap<_, _>>();
    let result = |function: &Function, at: i64| {
        Operand::Value(function.instruction(phis[&supplier[&at]]).result.expect("a phi's value"))
    };
    let replacements = supplier.keys().map(|&at| (at, result(function, at))).collect::<BTreeMap<_, _>>();
    for (&at, &phi) in &phis {
        let incoming = function
            .edge_predecessors(cfg::block(at))
            .into_iter()
            .map(cfg::id)
            .map(|parent| {
                (if exits.contains(&at) { Operand::Value(value) } else { replacements[&parent] }, cfg::block(parent))
            })
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
    }
    let swap = |at: i64| BTreeMap::from([(value, replacements[&at])]);
    // A phi reads `value` through a replaced parent only in a successor of one.
    let reading = replacements
        .keys()
        .flat_map(|&at| cfg::successors_of(function, at))
        .chain(replacements.keys().copied())
        .collect::<BTreeSet<_>>();
    for block in function.layout().to_vec() {
        if !reading.contains(&cfg::id(block)) {
            continue;
        }
        for phi in edges::phis(function, block) {
            let incoming = arms(function, phi)
                .into_iter()
                .map(|(incoming, parent)| match replacements.get(&cfg::id(parent)) {
                    Some(&replacement) if incoming == Operand::Value(value) => (replacement, parent),
                    _ => (incoming, parent),
                })
                .collect::<Vec<_>>();
            if from_arms(&incoming) != function.instruction(phi).operands {
                function.set_operands(phi, from_arms(&incoming));
            }
        }
        if replacements.contains_key(&cfg::id(block)) {
            for inst in operations(function, block) {
                let operands = ssa::substituted(function.instruction(inst), &swap(cfg::id(block)))
                    .map_err(|error| error.to_string())?;
                if operands != function.instruction(inst).operands {
                    function.set_operands(inst, operands);
                }
            }
        }
    }
    for (&at, &phi) in &phis {
        place_phi(function, cfg::block(at), phi)?;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "lcssamerges_tests.rs"]
mod tests;

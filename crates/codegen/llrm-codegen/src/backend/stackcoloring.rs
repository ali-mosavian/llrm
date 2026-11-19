//! StackColoring
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/StackColoring.cpp#L1)),
//! shared by every target: the static allocas whose lifetimes never overlap
//! share one frame object.
//!
//! # Example
//! Two `[64 x i32]` allocas, each live between its own `llvm.lifetime.start`
//! and `.end`, were a 512-byte frame; they are one 256-byte object (jpeg's
//! frame went from 400 to 336 bytes, the pin's). Without markers each alloca
//! keeps its own.
//!
//! # Used by
//! - [`crate::backend::selectiondag::dag_function`], which groups the allocas
//!   before the DAG is built (`share_frame`).
//! - `llrm-wasm`, which applies the grouping to its machine code after
//!   selection (`color_frame`).

/// DEVIATE SC1 (owner target-wasm): the pin's StackColoring runs on MIR after selection
/// and merges the frame objects its LIFETIME_START/END pseudos bound; this
/// groups the allocas on the IR, with the old selector's rule, before the
/// objects are made. Same frames on the shapes measured ((4,4) -> (4,2)); exit:
/// the MIR-level pass over the markers the DAG builder keeps.
///
/// StackColoring over the IR's lifetime markers (`llvm.lifetime.start` and
/// `.end`, which the builder keeps as chain nodes): the frame object of each
/// static alloca in `sizes` (value, bytes, alignment), locals never live
/// together sharing one, the largest first. Without markers, or after a
/// second return from `setjmp`, each has its own.
pub fn shared_allocas(
    module: &llrm_mir::module::Module,
    function: &llrm_mir::module::Function,
    sizes: &[(llrm_mir::module::ValueId, i64, i64)],
) -> Vec<(llrm_mir::module::ValueId, i32)> {
    use llrm_mir::intrinsics::Intrinsic;
    let own = || sizes.iter().enumerate().map(|(at, (value, _, _))| (*value, at as i32)).collect::<Vec<_>>();
    if llrm_mir::memory::calls_returns_twice(module, function) {
        return own();
    }
    let layout: Vec<llrm_mir::module::BlockId> = function.layout().to_vec();
    let positions: crate::support::hash::IndexMap<_, i64> = layout
        .iter()
        .flat_map(|&block| function.iter_block(block))
        .enumerate()
        .map(|(at, inst)| (inst, at as i64))
        .collect();
    let live = crate::backend::lifetimes::intervals(function, &layout, &positions, |inst| {
        let global = llrm_mir::memory::callee(&module.context, function, inst)?;
        let starts = match module.global(global).intrinsic()? {
            Intrinsic::LifetimeStart => true,
            Intrinsic::LifetimeEnd => false,
            _ => return None,
        };
        let llrm_mir::module::Operand::Value(object) = *function.instruction(inst).operands.get(1)? else {
            return None;
        };
        Some(crate::backend::lifetimes::Marker { object, starts })
    });
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&at| (-sizes[at].1, at));
    let mut colors: Vec<crate::backend::slots::Color> = Vec::new();
    let mut group_of = vec![0i32; sizes.len()];
    for at in order {
        let (value, size, _) = &sizes[at];
        let interval = live.get(value);
        let size = (*size + *size % 2) as u32;
        let color = interval.and_then(|interval| crate::backend::slots::choose(&colors, interval, size, &[]));
        let color = color.unwrap_or_else(|| {
            colors.push((colors.len() as i64, size, Vec::new()));
            colors.len() - 1
        });
        colors[color]
            .2
            .push(
                match interval {
                    Some(interval) => interval.clone(),
                    // Live throughout: it fits no other.
                    None => crate::analysis::intervals::Interval::new(
                        0,
                        vec![crate::analysis::intervals::Segment { start: i64::MIN, end: i64::MAX }],
                    ),
                },
            );
        group_of[at] = color as i32;
    }
    sizes.iter().zip(group_of).map(|((value, _, _), group)| (*value, group)).collect()
}

/// What StackColoring decided: the frame object each static alloca moves to,
/// and the objects that remain.
pub struct Coloring {
    /// New frame index by old, in the order of `sizes`.
    pub colors: Vec<i32>,
    /// The (size, alignment) of each remaining object: the largest of its
    /// group.
    pub objects: Vec<(i64, i64)>,
}

/// The coloring of the static allocas `sizes` (value, bytes, alignment, in
/// frame index order), applied by the target to its machine code after
/// selection: the DAG then still tells the allocas apart.
pub fn color_frame(
    module: &llrm_mir::module::Module,
    function: &llrm_mir::module::Function,
    sizes: &[(llrm_mir::module::ValueId, i64, i64)],
) -> Coloring {
    let mut colors: Vec<i32> = shared_allocas(module, function, sizes).into_iter().map(|(_, group)| group).collect();
    let count = colors.iter().map(|group| *group + 1).max().unwrap_or(0) as usize;
    // The pin keeps the surviving slots in their own order and drops the
    // merged ones, so a group sits where its largest member did.
    let mut first = vec![usize::MAX; count];
    for (at, &group) in colors.iter().enumerate() {
        let largest = &mut first[group as usize];
        if *largest == usize::MAX || sizes[at].1 > sizes[*largest].1 {
            *largest = at;
        }
    }
    let mut by_first: Vec<usize> = (0..count).collect();
    by_first.sort_by_key(|&group| first[group]);
    let mut renumbered = vec![0i32; count];
    for (index, group) in by_first.into_iter().enumerate() {
        renumbered[group] = index as i32;
    }
    for group in &mut colors {
        *group = renumbered[*group as usize];
    }
    let mut objects = vec![(0, 1); count];
    for ((_, size, align), &group) in sizes.iter().zip(&colors) {
        let object = &mut objects[group as usize];
        *object = (object.0.max(*size), object.1.max(*align));
    }
    Coloring { colors, objects }
}

/// The frame objects (size, alignment) of the static allocas `sizes` after
/// sharing, numbered as `info` now numbers them (before selection, SC1): the
/// groups in the order they were made, largest first.
pub fn share_frame(
    module: &llrm_mir::module::Module,
    function: &llrm_mir::module::Function,
    info: &mut crate::backend::selectiondag::function_lowering::FunctionLoweringInfo,
    sizes: &[(llrm_mir::module::ValueId, i64, i64)],
) -> Vec<(i64, i64)> {
    let groups = shared_allocas(module, function, sizes);
    let count = groups.iter().map(|(_, group)| *group + 1).max().unwrap_or(0);
    info.share_allocas(groups.iter().cloned(), count);
    let mut objects = vec![(0, 1); count as usize];
    for ((_, size, align), (_, group)) in sizes.iter().zip(&groups) {
        let object = &mut objects[*group as usize];
        *object = (object.0.max(*size), object.1.max(*align));
    }
    objects
}

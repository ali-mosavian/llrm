//! `AllSwitchPaths`: the paths of a loop that set a switch's state to a
//! constant, found backwards from the `phi` that is the state.

use std::cell::Cell;
use std::collections::VecDeque;

use llrm_mir::context::Context;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::hash::HashMap;
use llrm_mir::loopinfo::{Loop, LoopInfo};
use llrm_mir::module::{BlockId, Function, InstId};

use super::{
    DfaJumpThreadingOptions, constant_int, default_dest, incoming_per_block, phi_of, successor_edges, switch_cases,
};

thread_local! {
    static STEPS: Cell<usize> = const { Cell::new(0) };
}

/// The blocks and phi inputs the path search has looked at so far: the work
/// of [`find`], for a test of its slope.
pub fn path_steps() -> usize {
    STEPS.with(Cell::get)
}

/// `PathType`: a path of blocks.
pub(super) type PathType = VecDeque<BlockId>;

/// `ThreadingPath`
/// ([L409](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L409)):
/// a path in the control flow of a loop that can be threaded by cloning the
/// necessary blocks and replacing conditional branches with unconditional
/// ones: the blocks, the exit state (the bits of the constant), and the
/// determinator, the block that determines the next state.
#[derive(Clone, Debug)]
pub(super) struct ThreadingPath {
    pub(super) path: PathType,
    pub(super) exit_value: u128,
    pub(super) determinator: BlockId,
}

impl ThreadingPath {
    fn new(
        determinator: BlockId,
        exit_value: u128,
    ) -> Self {
        Self { path: PathType::new(), exit_value, determinator }
    }

    /// `appendExcludingFirst`.
    fn append_excluding_first(
        &mut self,
        other: &PathType,
    ) {
        self.path.extend(other.iter().skip(1));
    }
}

/// What `AllSwitchPaths` found around one switch.
pub(super) struct SwitchPaths {
    pub(super) switch: InstId,
    pub(super) switch_block: BlockId,
    pub(super) paths: Vec<ThreadingPath>,
}

/// `AllSwitchPaths::run`: `findTPaths` then `unifyTPaths`.
pub(super) fn find(
    function: &Function,
    context: &Context,
    loops: &LoopInfo,
    options: &DfaJumpThreadingOptions,
    switch: InstId,
    outermost: Loop<'_>,
) -> SwitchPaths {
    let switch_block = function.parent(switch).expect("a placed switch");
    let mut all = AllSwitchPaths {
        function,
        context,
        loops,
        options,
        switch,
        switch_block,
        outermost,
        num_visited: 0,
        paths: Vec::new(),
        case_val_to_dest: None,
    };
    all.find_t_paths();
    all.unify_t_paths();
    SwitchPaths { switch, switch_block, paths: all.paths }
}

/// Value: the `phi` that defines a switch state; key: the block of that `phi`.
type StateDefMap = IdMap<BlockId, InstId>;

struct AllSwitchPaths<'a> {
    function: &'a Function,
    context: &'a Context,
    loops: &'a LoopInfo,
    options: &'a DfaJumpThreadingOptions,
    switch: InstId,
    switch_block: BlockId,
    /// `SwitchOuterLoop`.
    outermost: Loop<'a>,
    num_visited: u32,
    paths: Vec<ThreadingPath>,
    case_val_to_dest: Option<HashMap<u128, BlockId>>,
}

impl AllSwitchPaths<'_> {
    fn step(&self) {
        STEPS.with(|steps| steps.set(steps.get() + 1));
    }

    /// The `phi` the switch reads.
    fn switch_phi(&self) -> InstId {
        phi_of(self.function, self.function.instruction(self.switch).operands[0]).expect("a candidate reads a phi")
    }

    /// `getPathsFromStateDefMap`
    /// ([L610](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L610)).
    fn paths_from_state_def_map(
        &mut self,
        state_def: &StateDefMap,
        phi: InstId,
        visited: &mut IdSet<BlockId>,
        paths_limit: usize,
    ) -> Vec<ThreadingPath> {
        let mut res: Vec<ThreadingPath> = Vec::new();
        let phi_bb = self.function.parent(phi).expect("a placed phi");
        visited.insert(phi_bb);

        let mut unique_blocks = IdSet::<BlockId>::new();
        for (incoming_bb, incoming_value) in incoming_per_block(self.function, phi) {
            if res.len() >= paths_limit {
                break;
            }
            self.step();
            if !unique_blocks.insert(incoming_bb) {
                continue;
            }
            if !self.outermost.contains(incoming_bb) {
                continue;
            }

            // We found the determinator. This is the start of our path.
            if let Some(constant) = constant_int(self.context, incoming_value) {
                // SwitchBlock is the determinator, unsupported unless its also
                // the def.
                if phi_bb == self.switch_block
                    && self.switch_block != self.function.parent(self.switch_phi()).expect("a placed phi")
                {
                    continue;
                }
                let mut new_path = ThreadingPath::new(phi_bb, constant);
                // Don't add SwitchBlock at the start, this is handled later.
                if incoming_bb != self.switch_block {
                    // Don't add a cycle to the path.
                    if visited.contains(&incoming_bb) {
                        continue;
                    }
                    new_path.path.push_back(incoming_bb);
                }
                new_path.path.push_back(phi_bb);
                res.push(new_path);
                continue;
            }
            // Don't get into a cycle.
            if visited.contains(&incoming_bb) || incoming_bb == self.switch_block {
                continue;
            }
            // Recurse up the PHI chain.
            let Some(incoming_phi) = phi_of(self.function, incoming_value) else { continue };
            let incoming_phi_def_bb = self.function.parent(incoming_phi).expect("a placed phi");
            if !state_def.contains_key(&incoming_phi_def_bb) {
                continue;
            }

            // Direct predecessor, just add to the path.
            if incoming_phi_def_bb == incoming_bb {
                let pred_paths =
                    self.paths_from_state_def_map(state_def, incoming_phi, visited, paths_limit - res.len());
                for mut path in pred_paths {
                    path.path.push_back(phi_bb);
                    res.push(path);
                }
                continue;
            }
            // Not a direct predecessor, find intermediate paths to append to
            // the existing path.
            if visited.contains(&incoming_phi_def_bb) {
                continue;
            }

            let inter_path_limit = paths_limit - res.len();
            let intermediate = self.paths(incoming_phi_def_bb, incoming_bb, visited, 1, inter_path_limit);
            if intermediate.is_empty() {
                continue;
            }

            let pred_path_limit = inter_path_limit / intermediate.len();
            let pred_paths = self.paths_from_state_def_map(state_def, incoming_phi, visited, pred_path_limit);
            for path in &pred_paths {
                for inter in &intermediate {
                    let mut new_path = path.clone();
                    new_path.append_excluding_first(inter);
                    new_path.path.push_back(phi_bb);
                    res.push(new_path);
                }
            }
        }
        visited.remove(&phi_bb);
        res
    }

    /// `paths`
    /// ([L700](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L700)):
    /// the paths from `bb` to `to_bb` inside the outermost loop of the switch.
    fn paths(
        &mut self,
        bb: BlockId,
        to_bb: BlockId,
        visited: &mut IdSet<BlockId>,
        path_depth: u32,
        paths_limit: usize,
    ) -> Vec<PathType> {
        let mut res: Vec<PathType> = Vec::new();

        // Stop exploring paths after visiting MaxPathLength blocks.
        if path_depth > self.options.max_path_length {
            return res;
        }

        visited.insert(bb);
        self.num_visited += 1;
        self.step();
        if self.num_visited > self.options.max_num_visited_paths {
            return res;
        }

        // Stop if we have reached the BB out of loop, since its successors have
        // no impact on the DFA.
        if !self.outermost.contains(bb) {
            return res;
        }

        // Some blocks have multiple edges to the same successor, and this set
        // is used to prevent a duplicate path from being generated.
        let mut successors = IdSet::<BlockId>::new();
        for succ in successor_edges(self.function, bb) {
            if res.len() >= paths_limit {
                break;
            }
            if !successors.insert(succ) {
                continue;
            }

            // Found a cycle through the final block.
            if succ == to_bb {
                res.push(PathType::from([bb, to_bb]));
                continue;
            }

            // We have encountered a cycle, do not get caught in it.
            if visited.contains(&succ) {
                continue;
            }

            let curr_loop = self.loops.get_loop_for(bb);
            // Unlikely to be beneficial.
            if Some(succ) == curr_loop.map(|one| one.get_header()) {
                continue;
            }
            // Skip for now, revisit this condition later to see the impact on
            // coverage and compile time.
            if self.loops.get_loop_for(succ) != curr_loop {
                continue;
            }
            let succ_paths = self.paths(succ, to_bb, visited, path_depth + 1, paths_limit - res.len());
            for mut path in succ_paths {
                path.push_front(bb);
                res.push(path);
            }
        }
        // This block could now be visited again from a different predecessor.
        // Note that this will result in exponential runtime, which the caps
        // bound.
        visited.remove(&bb);
        res
    }

    /// `getStateDefMap`
    /// ([L768](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L768)):
    /// walk the use-def chain and collect all the state-defining blocks and
    /// the `phi` in each that defines the state.
    fn state_def_map(&self) -> StateDefMap {
        let mut res = StateDefMap::new();
        let mut multiple_def_bbs = IdSet::<BlockId>::new();
        let first_def = self.switch_phi();

        let mut stack = vec![first_def];
        let mut seen_values = IdSet::<InstId>::new();

        while let Some(cur_phi) = stack.pop() {
            let cur_def_block = self.function.parent(cur_phi).expect("a placed phi");
            if res.contains_key(&cur_def_block) {
                multiple_def_bbs.insert(cur_def_block);
            } else {
                res.insert(cur_def_block, cur_phi);
            }
            seen_values.insert(cur_phi);

            for (incoming_bb, value) in incoming_per_block(self.function, cur_phi) {
                self.step();
                let Some(incoming_phi) = phi_of(self.function, value) else { continue };
                let is_outside_loops = !self.outermost.contains(incoming_bb);
                if seen_values.contains(&incoming_phi) || is_outside_loops {
                    continue;
                }
                stack.push(incoming_phi);
            }
        }

        // NOTE: If multiple phi definitions exist in a block, we cannot thread
        // the paths with such block by simple cloning: it is impossible to
        // diverge then->det into then->det.0 and then->det.1 by simple path
        // cloning.
        for bb in multiple_def_bbs.iter() {
            res.remove(&bb);
        }
        res
    }

    /// `findTPaths`
    /// ([L816](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L816)):
    /// find all threadable paths.
    fn find_t_paths(&mut self) {
        let state_def = self.state_def_map();
        if state_def.is_empty() {
            return;
        }

        let switch_phi = self.switch_phi();
        let switch_phi_def_bb = self.function.parent(switch_phi).expect("a placed phi");
        let mut visited = IdSet::<BlockId>::new();
        // Get paths from the determinator BBs to SwitchPhiDefBB.
        let max_num_paths = self.options.max_num_paths as usize;
        let paths_to_phi_def = self.paths_from_state_def_map(&state_def, switch_phi, &mut visited, max_num_paths);
        if switch_phi_def_bb == self.switch_block || paths_to_phi_def.is_empty() {
            self.paths = paths_to_phi_def;
            return;
        }

        let paths_limit = max_num_paths / paths_to_phi_def.len();
        // Find and append paths from SwitchPhiDefBB to SwitchBlock.
        let paths_to_switch_bb = self.paths(switch_phi_def_bb, self.switch_block, &mut visited, 1, paths_limit);
        if paths_to_switch_bb.is_empty() {
            return;
        }

        let mut temp_list = Vec::new();
        for path in &paths_to_phi_def {
            let mut path_set = IdSet::<BlockId>::new();
            for &bb in &path.path {
                path_set.insert(bb);
            }
            for path_to_sw in &paths_to_switch_bb {
                if path_to_sw.iter().skip(1).any(|bb| path_set.contains(bb)) {
                    continue;
                }
                let mut path_copy = path.clone();
                path_copy.append_excluding_first(path_to_sw);
                temp_list.push(path_copy);
            }
        }
        self.paths = temp_list;
    }

    /// `getNextCaseSuccessor`
    /// ([L864](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L864)):
    /// the successor of a particular case value, from a map built once.
    fn next_case_successor(
        &mut self,
        next_state: u128,
    ) -> BlockId {
        if self.case_val_to_dest.is_none() {
            let map = switch_cases(self.function, self.context, self.switch).into_iter().collect();
            self.case_val_to_dest = Some(map);
        }
        let map = self.case_val_to_dest.as_ref().expect("built above");
        map.get(&next_state).copied().unwrap_or_else(|| default_dest(self.function, self.switch))
    }

    /// `unifyTPaths`
    /// ([L880](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L880)):
    /// two states are equivalent if they have the same switch destination;
    /// unify the states in different threading paths if they are.
    fn unify_t_paths(&mut self) {
        let mut dest_to_state = IdMap::<BlockId, u128>::new();
        for index in 0..self.paths.len() {
            let next_state = self.paths[index].exit_value;
            let dest = self.next_case_successor(next_state);
            match dest_to_state.get(&dest) {
                None => {
                    dest_to_state.insert(dest, next_state);
                }
                Some(&state) => self.paths[index].exit_value = state,
            }
        }
    }
}

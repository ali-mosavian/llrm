//! Trace metrics: how long a path through a function takes, by the data
//! dependences and the resources of its instructions (`MachineTraceMetrics`
//! of the pin, `TS_MinInstrCount` ensemble only).
//!
//! # How it works
//! A *trace* is the path through a block that has the fewest instructions
//! above it (up to the top of its loop) and below it. For every instruction
//! of the trace the pass knows its *depth* (cycles from the trace's start to
//! the moment its inputs are ready) and *height* (cycles from its result to
//! the trace's end); the trace's *critical path* is the largest depth plus
//! height. The *resource length* is the most any one resource is busy over the
//! trace. Both follow `MachineTraceMetrics.cpp` step for step, over the
//! [`Func`] a caller builds from its own body.
//!
//! # Example
//! ```text
//! b0: %1 = load      (4)      trace of b2 = b0 b1 b2
//! b1: %2 = add %1    (1)      depth of the add = 4, height = 1 + what follows
//! b2: %3 = mul %2    (3)      critical path = 4 + 1 + 3
//! ```
//!
//! # Not translated
//! - TM1: the cache is dropped when the body changes (the pin invalidates the
//!   blocks the conversion touched and what depends on them). A trace is the
//!   same unless a cached trace of an earlier query chose a predecessor whose
//!   depth was not yet known.
//! - TM2: an operand's latency is the def's write latency; the use's
//!   `ReadAdvance` is not applied (arm64 SB6) and the flags' write is one
//!   cycle.
//! - TM3: the `Local` ensemble (no caller).
//!
//! # Used by
//! [`crate::ifcvt`].

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

/// A machine register unit (the flags, a register by number).
pub type Unit = u32;

/// One instruction as the trace metrics read it.
#[derive(Clone, Debug, Default)]
pub struct Ins {
    /// `isTransient`: a phi or a copy, or a meta instruction. Costs nothing and
    /// is not counted.
    pub transient: bool,
    pub call: bool,
    /// For a phi: the value from each predecessor block.
    pub phi: Option<Vec<(usize, u32)>>,
    /// The values it defines (virtual registers), and reads.
    pub defs: Vec<u32>,
    pub uses: Vec<u32>,
    /// The machine register units it defines (live, not dead) and reads.
    pub unit_defs: Vec<Unit>,
    pub unit_uses: Vec<Unit>,
    /// The latency of what it defines.
    pub latency: u32,
    /// `(resource, ReleaseAtCycle)` of each processor resource it writes.
    pub writes: Vec<(usize, u32)>,
}

#[derive(Clone, Debug, Default)]
pub struct Blk {
    pub pred: Vec<usize>,
    pub succ: Vec<usize>,
    pub ins: Vec<Ins>,
}

/// What the machine model says (`TargetSchedModel`).
#[derive(Clone, Debug)]
pub struct Model {
    pub issue_width: u32,
    /// `getLatencyFactor`: the unit cycles are scaled by.
    pub latency_factor: u32,
    /// Each resource's `getResourceFactor`; resource 0 is the invalid one.
    pub factors: Vec<u32>,
}

/// The loops of the function, as the trace selection asks.
#[derive(Clone, Debug, Default)]
pub struct Loops {
    /// Each block's innermost loop, by header.
    pub of: Vec<Option<usize>>,
    /// Loop `i`: its header, its blocks, and its parent loop.
    pub headers: Vec<usize>,
    pub blocks: Vec<HashSet<usize>>,
    pub parent: Vec<Option<usize>>,
}

impl Loops {
    fn contains_loop(
        &self,
        outer: usize,
        inner: Option<usize>,
    ) -> bool {
        let mut at = inner;
        while let Some(one) = at {
            if one == outer {
                return true;
            }
            at = self.parent[one];
        }
        false
    }
}

/// The body: blocks, where each value is defined, the loops.
pub struct Func {
    pub blocks: Vec<Blk>,
    /// The one definition of each virtual register: `(block, instruction)`.
    pub def: HashMap<u32, (usize, usize)>,
    pub loops: Loops,
    pub model: Model,
}

const INVALID: usize = usize::MAX;

#[derive(Clone, Debug)]
struct LiveIn {
    /// A value, or a machine register unit.
    reg: Option<u32>,
    unit: Option<Unit>,
    height: u32,
}

#[derive(Clone, Debug)]
struct TraceBlock {
    pred: Option<usize>,
    succ: Option<usize>,
    instr_depth: u32,
    instr_height: u32,
    head: usize,
    tail: usize,
    valid_instr_depths: bool,
    valid_instr_heights: bool,
    critical_path: u32,
    live_ins: Vec<LiveIn>,
}

impl TraceBlock {
    fn new() -> Self {
        TraceBlock {
            pred: None,
            succ: None,
            instr_depth: 0,
            instr_height: 0,
            head: INVALID,
            tail: INVALID,
            valid_instr_depths: false,
            valid_instr_heights: false,
            critical_path: 0,
            live_ins: Vec::new(),
        }
    }

    fn valid_depth(&self) -> bool {
        self.head != INVALID
    }

    fn valid_height(&self) -> bool {
        self.tail != INVALID
    }

    /// `isUsefulDominator`: this block's instruction depths are comparable
    /// with `other`'s.
    fn is_useful_dominator(
        &self,
        other: &TraceBlock,
    ) -> bool {
        if !self.valid_depth() || !other.valid_depth() {
            return false;
        }
        if self.head != other.head {
            return false;
        }
        self.valid_instr_depths && self.instr_depth <= other.instr_depth
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct InstrCycles {
    depth: u32,
    height: u32,
}

/// What a block takes of each resource, and how many instructions it has.
#[derive(Clone, Debug)]
struct Fixed {
    instr_count: u32,
    /// Scaled by the resource factor.
    release: Vec<u32>,
}

/// The `MinInstr` ensemble over a [`Func`].
pub struct Traces<'a> {
    f: &'a Func,
    fixed: Vec<Option<Fixed>>,
    tb: Vec<TraceBlock>,
    depths: Vec<Vec<u32>>,
    heights: Vec<Vec<u32>>,
    cycles: HashMap<(usize, usize), InstrCycles>,
}

#[derive(Clone, Copy, Debug)]
struct LiveUnit {
    cycle: u32,
    at: Option<(usize, usize)>,
}

impl<'a> Traces<'a> {
    pub fn new(f: &'a Func) -> Self {
        let n = f.blocks.len();
        let kinds = f.model.factors.len();
        Traces {
            f,
            fixed: vec![None; n],
            tb: vec![TraceBlock::new(); n],
            depths: vec![vec![0; kinds]; n],
            heights: vec![vec![0; kinds]; n],
            cycles: HashMap::default(),
        }
    }

    fn kinds(&self) -> usize {
        self.f.model.factors.len()
    }

    /// `getResources`: the block's instruction count and resource use.
    fn resources(
        &mut self,
        block: usize,
    ) -> u32 {
        if self.fixed[block].is_none() {
            let mut count = 0;
            let mut release = vec![0u32; self.kinds()];
            for ins in &self.f.blocks[block].ins {
                if ins.transient {
                    continue;
                }
                count += 1;
                for &(resource, cycles) in &ins.writes {
                    release[resource] += cycles;
                }
            }
            for (k, one) in release.iter_mut().enumerate() {
                *one *= self.f.model.factors[k];
            }
            self.fixed[block] = Some(Fixed { instr_count: count, release });
        }
        self.fixed[block].as_ref().map_or(0, |fixed| fixed.instr_count)
    }

    fn release(
        &mut self,
        block: usize,
    ) -> Vec<u32> {
        self.resources(block);
        self.fixed[block].as_ref().map(|fixed| fixed.release.clone()).unwrap_or_default()
    }

    fn loop_of(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.f.loops.of[block]
    }

    /// `isExitingLoop(From, To)`: leaving the loop `from` for `to`.
    fn is_exiting(
        &self,
        from: Option<usize>,
        to: Option<usize>,
    ) -> bool {
        from.is_some_and(|from| !self.f.loops.contains_loop(from, to))
    }

    fn pick_pred(
        &mut self,
        block: usize,
    ) -> Option<usize> {
        if self.f.blocks[block].pred.is_empty() {
            return None;
        }
        let cur = self.loop_of(block);
        if cur.is_some_and(|l| self.f.loops.headers[l] == block) {
            return None;
        }
        let count = self.resources(block);
        let mut best: Option<usize> = None;
        let mut best_depth = 0;
        for &pred in &self.f.blocks[block].pred {
            if !self.tb[pred].valid_depth() {
                continue;
            }
            let depth = self.tb[pred].instr_depth + count;
            if best.is_none() || depth < best_depth {
                best = Some(pred);
                best_depth = depth;
            }
        }
        best
    }

    fn pick_succ(
        &self,
        block: usize,
    ) -> Option<usize> {
        if self.f.blocks[block].succ.is_empty() {
            return None;
        }
        let cur = self.loop_of(block);
        let mut best: Option<usize> = None;
        let mut best_height = 0;
        for &succ in &self.f.blocks[block].succ {
            if cur.is_some_and(|l| self.f.loops.headers[l] == succ) {
                continue;
            }
            if self.is_exiting(cur, self.loop_of(succ)) {
                continue;
            }
            if !self.tb[succ].valid_height() {
                continue;
            }
            let height = self.tb[succ].instr_height;
            if best.is_none() || height < best_height {
                best = Some(succ);
                best_height = height;
            }
        }
        best
    }

    /// The `LoopBoundsPostOrderTraversal` from `start`: blocks whose trace is
    /// not yet known, in the order their dependencies come first.
    fn bounded_post_order(
        &self,
        start: usize,
        downward: bool,
    ) -> Vec<usize> {
        let mut visited: HashSet<usize> = HashSet::default();
        let mut out = Vec::new();
        let insert_edge = |from: Option<usize>, to: usize, visited: &mut HashSet<usize>| -> bool {
            let known = if downward { self.tb[to].valid_height() } else { self.tb[to].valid_depth() };
            if known {
                return false;
            }
            if let Some(from) = from
                && let Some(from_loop) = self.loop_of(from)
            {
                let key = if downward { to } else { from };
                if key == self.f.loops.headers[from_loop] {
                    return false;
                }
                if self.is_exiting(Some(from_loop), self.loop_of(to)) {
                    return false;
                }
            }
            visited.insert(to)
        };
        if !insert_edge(None, start, &mut visited) {
            return out;
        }
        let edges = |block: usize| -> &Vec<usize> {
            if downward { &self.f.blocks[block].succ } else { &self.f.blocks[block].pred }
        };
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some((node, child)) = stack.pop() {
            let list = edges(node);
            let mut next = child;
            let mut pushed = false;
            while next < list.len() {
                let to = list[next];
                next += 1;
                if insert_edge(Some(node), to, &mut visited) {
                    stack.push((node, next));
                    stack.push((to, 0));
                    pushed = true;
                    break;
                }
            }
            if !pushed {
                out.push(node);
            }
        }
        out
    }

    fn compute_trace(
        &mut self,
        block: usize,
    ) {
        for b in self.bounded_post_order(block, false) {
            self.tb[b].pred = self.pick_pred(b);
            self.compute_depth_resources(b);
        }
        for b in self.bounded_post_order(block, true) {
            self.tb[b].succ = self.pick_succ(b);
            self.compute_height_resources(b);
        }
    }

    fn compute_depth_resources(
        &mut self,
        block: usize,
    ) {
        let kinds = self.kinds();
        let Some(pred) = self.tb[block].pred else {
            self.tb[block].instr_depth = 0;
            self.tb[block].head = block;
            self.depths[block] = vec![0; kinds];
            return;
        };
        let pred_count = self.resources(pred);
        self.tb[block].instr_depth = self.tb[pred].instr_depth + pred_count;
        self.tb[block].head = self.tb[pred].head;
        let release = self.release(pred);
        self.depths[block] = (0..kinds).map(|k| self.depths[pred][k] + release[k]).collect();
    }

    fn compute_height_resources(
        &mut self,
        block: usize,
    ) {
        let kinds = self.kinds();
        self.tb[block].instr_height = self.resources(block);
        let release = self.release(block);
        let Some(succ) = self.tb[block].succ else {
            self.tb[block].tail = block;
            self.heights[block] = release;
            return;
        };
        self.tb[block].instr_height += self.tb[succ].instr_height;
        self.tb[block].tail = self.tb[succ].tail;
        self.heights[block] = (0..kinds).map(|k| self.heights[succ][k] + release[k]).collect();
    }

    /// The def of virtual register `reg`, `(block, instruction)`.
    fn def_of(
        &self,
        reg: u32,
    ) -> Option<(usize, usize)> {
        self.f.def.get(&reg).copied()
    }

    fn latency_of(
        &self,
        at: (usize, usize),
    ) -> u32 {
        let ins = &self.f.blocks[at.0].ins[at.1];
        if ins.transient { 0 } else { ins.latency }
    }

    /// `getDataDeps`/`getPHIDeps`: the instructions `ins` reads from.
    fn deps(
        &self,
        block: usize,
        ins: &Ins,
        pred: Option<usize>,
    ) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        if let Some(phi) = &ins.phi {
            let Some(pred) = pred else { return out };
            if let Some(&(_, value)) = phi.iter().find(|(from, _)| *from == pred)
                && let Some(def) = self.def_of(value)
            {
                out.push(def);
            }
            return out;
        }
        let _ = block;
        for &value in &ins.uses {
            if let Some(def) = self.def_of(value) {
                out.push(def);
            }
        }
        out
    }

    fn update_depth(
        &mut self,
        block: usize,
        at: usize,
        units: &mut HashMap<Unit, LiveUnit>,
    ) {
        let ins = self.f.blocks[block].ins[at].clone();
        let mut deps = self.deps(block, &ins, self.tb[block].pred);
        if ins.phi.is_none() && (!ins.unit_defs.is_empty() || !ins.unit_uses.is_empty()) {
            // `updatePhysDepsDownwards`.
            for unit in &ins.unit_uses {
                if let Some(live) = units.get(unit)
                    && let Some(at) = live.at
                {
                    deps.push(at);
                }
            }
            for unit in &ins.unit_defs {
                units.insert(*unit, LiveUnit { cycle: 0, at: Some((block, at)) });
            }
        }
        let mut cycle = 0;
        for dep in deps {
            let dep_tb = &self.tb[dep.0];
            if !dep_tb.is_useful_dominator(&self.tb[block]) {
                continue;
            }
            let mut dep_cycle = self.cycles.get(&dep).copied().unwrap_or_default().depth;
            if !self.f.blocks[dep.0].ins[dep.1].transient {
                dep_cycle += self.latency_of(dep);
            }
            cycle = cycle.max(dep_cycle);
        }
        let entry = self.cycles.entry((block, at)).or_default();
        entry.depth = cycle;
        let height = entry.height;
        if self.tb[block].valid_instr_heights {
            self.tb[block].critical_path = self.tb[block].critical_path.max(cycle + height);
        }
    }

    fn cross_block_critical_path(
        &self,
        block: usize,
    ) -> u32 {
        let mut max = 0;
        for live in &self.tb[block].live_ins {
            let Some(reg) = live.reg else { continue };
            let Some(def) = self.def_of(reg) else { continue };
            if !self.tb[def.0].is_useful_dominator(&self.tb[block]) {
                continue;
            }
            let len = live.height + self.cycles.get(&def).copied().unwrap_or_default().depth;
            max = max.max(len);
        }
        max
    }

    fn compute_instr_depths(
        &mut self,
        block: usize,
    ) {
        let mut stack = Vec::new();
        let mut at = Some(block);
        while let Some(b) = at {
            if self.tb[b].valid_instr_depths {
                break;
            }
            stack.push(b);
            at = self.tb[b].pred;
        }
        let mut units: HashMap<Unit, LiveUnit> = HashMap::default();
        while let Some(b) = stack.pop() {
            self.tb[b].valid_instr_depths = true;
            self.tb[b].critical_path = 0;
            if self.tb[b].valid_instr_heights {
                self.tb[b].critical_path = self.cross_block_critical_path(b);
            }
            for i in 0..self.f.blocks[b].ins.len() {
                self.update_depth(b, i, &mut units);
            }
        }
    }

    /// `pushDepHeight`: `true` when the def had no height yet.
    fn push_dep_height(
        &self,
        def: (usize, usize),
        use_height: u32,
        heights: &mut HashMap<(usize, usize), u32>,
    ) -> bool {
        let height = use_height + self.latency_of(def);
        match heights.get_mut(&def) {
            None => {
                heights.insert(def, height);
                true
            }
            Some(old) => {
                if *old < height {
                    *old = height;
                }
                false
            }
        }
    }

    /// `addLiveIns`: the value is live into every block of `trace` below its
    /// defining block.
    fn add_live_ins(
        &mut self,
        def: (usize, usize),
        value: u32,
        trace: &[usize],
    ) {
        for &b in trace.iter().rev() {
            if b == def.0 {
                return;
            }
            self.tb[b].live_ins.push(LiveIn { reg: Some(value), unit: None, height: 0 });
        }
    }

    fn compute_instr_heights(
        &mut self,
        block: usize,
    ) {
        let mut stack = Vec::new();
        let mut at = Some(block);
        while let Some(b) = at {
            if self.tb[b].valid_instr_heights {
                break;
            }
            stack.push(b);
            self.tb[b].live_ins.clear();
            at = self.tb[b].succ;
        }
        let mut heights: HashMap<(usize, usize), u32> = HashMap::default();
        let mut units: HashMap<Unit, LiveUnit> = HashMap::default();
        if let Some(below) = at {
            let lives = self.tb[below].live_ins.clone();
            for live in lives {
                if let Some(reg) = live.reg {
                    if let Some(def) = self.def_of(reg) {
                        let entry = heights.entry(def).or_insert(0);
                        if *entry < live.height {
                            *entry = live.height;
                        }
                    }
                } else if let Some(unit) = live.unit {
                    units.insert(unit, LiveUnit { cycle: live.height, at: None });
                }
            }
        }
        while let Some(b) = stack.last().copied() {
            self.tb[b].valid_instr_heights = true;
            self.tb[b].critical_path = 0;
            let mut succ = self.tb[b].succ;
            if succ.is_none()
                && let Some(l) = self.loop_of(b)
            {
                let header = self.f.loops.headers[l];
                if self.f.blocks[b].succ.contains(&header) {
                    succ = Some(header);
                }
            }
            if let Some(succ) = succ {
                let phis: Vec<(usize, Ins)> = self.f.blocks[succ]
                    .ins
                    .iter()
                    .enumerate()
                    .take_while(|(_, ins)| ins.phi.is_some())
                    .map(|(i, ins)| (i, ins.clone()))
                    .collect();
                for (i, phi) in phis {
                    let deps = self.deps(succ, &phi, Some(b));
                    let Some(&dep) = deps.first() else { continue };
                    let height =
                        if self.tb[b].succ.is_some() { self.cycles.get(&(succ, i)).copied().unwrap_or_default().height } else { 0 };
                    if self.push_dep_height(dep, height, &mut heights) {
                        // The value the phi reads.
                        let value = phi.phi.as_ref().and_then(|p| p.iter().find(|(f, _)| *f == b)).map(|(_, v)| *v);
                        if let Some(value) = value {
                            self.add_live_ins(dep, value, &stack);
                        }
                    }
                }
            }
            for i in (0..self.f.blocks[b].ins.len()).rev() {
                let ins = self.f.blocks[b].ins[i].clone();
                let mut cycle = 0;
                if let Some(height) = heights.remove(&(b, i)) {
                    cycle = height;
                }
                let deps: Vec<(usize, usize, u32)> = if ins.phi.is_none() {
                    ins.uses.iter().filter_map(|&v| self.def_of(v).map(|d| (d.0, d.1, v))).collect()
                } else {
                    Vec::new()
                };
                if ins.phi.is_none() && (!ins.unit_defs.is_empty() || !ins.unit_uses.is_empty()) {
                    // `updatePhysDepsUpwards`.
                    for unit in &ins.unit_defs {
                        if let Some(live) = units.remove(unit) {
                            let mut dep = live.cycle;
                            if !ins.transient {
                                dep += ins.latency;
                            }
                            cycle = cycle.max(dep);
                        }
                    }
                    for unit in &ins.unit_uses {
                        let live = units.entry(*unit).or_insert(LiveUnit { cycle: 0, at: None });
                        if live.cycle <= cycle && live.at != Some((b, i)) {
                            live.cycle = cycle;
                            live.at = Some((b, i));
                        }
                    }
                }
                for (db, di, value) in deps {
                    if self.push_dep_height((db, di), cycle, &mut heights) {
                        self.add_live_ins((db, di), value, &stack);
                    }
                }
                let entry = self.cycles.entry((b, i)).or_default();
                entry.height = cycle;
                let depth = entry.depth;
                if self.tb[b].valid_instr_depths {
                    self.tb[b].critical_path = self.tb[b].critical_path.max(cycle + depth);
                }
            }
            let lives = self.tb[b].live_ins.clone();
            let mut updated = Vec::new();
            for mut live in lives {
                if let Some(reg) = live.reg
                    && let Some(def) = self.def_of(reg)
                {
                    live.height = heights.get(&def).copied().unwrap_or(0);
                }
                updated.push(live);
            }
            self.tb[b].live_ins = updated;
            for (unit, live) in units.iter() {
                self.tb[b].live_ins.push(LiveIn { reg: None, unit: Some(*unit), height: live.cycle });
            }
            if self.tb[b].valid_instr_depths {
                let cross = self.cross_block_critical_path(b);
                self.tb[b].critical_path = self.tb[b].critical_path.max(cross);
            }
            stack.pop();
        }
    }

    /// `getTrace`: the trace through `block`, computed as far as it was not.
    pub fn trace(
        &mut self,
        block: usize,
    ) -> Trace {
        if !self.tb[block].valid_depth() || !self.tb[block].valid_height() {
            self.compute_trace(block);
        }
        if !self.tb[block].valid_instr_depths {
            self.compute_instr_depths(block);
        }
        if !self.tb[block].valid_instr_heights {
            self.compute_instr_heights(block);
        }
        Trace { block }
    }

    fn get_cycles(
        &self,
        scaled: u32,
    ) -> u32 {
        scaled.div_ceil(self.f.model.latency_factor.max(1))
    }
}

/// A trace, named by its center block.
#[derive(Clone, Copy, Debug)]
pub struct Trace {
    block: usize,
}

impl Trace {
    pub fn critical_path(
        &self,
        t: &Traces,
    ) -> u32 {
        t.tb[self.block].critical_path
    }

    pub fn instr_depth(
        &self,
        t: &Traces,
        at: usize,
    ) -> u32 {
        t.cycles.get(&(self.block, at)).copied().unwrap_or_default().depth
    }

    /// `getInstrSlack`.
    pub fn instr_slack(
        &self,
        t: &Traces,
        at: usize,
    ) -> u32 {
        let c = t.cycles.get(&(self.block, at)).copied().unwrap_or_default();
        self.critical_path(t).wrapping_sub(c.depth + c.height)
    }

    /// `getPHIDepth`: the depth at which the phi's value from this block is
    /// ready.
    pub fn phi_depth(
        &self,
        t: &Traces,
        phi_block: usize,
        phi: usize,
    ) -> u32 {
        let ins = &t.f.blocks[phi_block].ins[phi];
        let deps = t.deps(self.block, ins, Some(self.block));
        let Some(&dep) = deps.first() else { return 0 };
        let mut cycle = t.cycles.get(&dep).copied().unwrap_or_default().depth;
        if !t.f.blocks[dep.0].ins[dep.1].transient {
            cycle += t.latency_of(dep);
        }
        cycle
    }

    /// `getResourceLength(ExtraBlocks)`.
    pub fn resource_length(
        &self,
        t: &mut Traces,
        extra: &[usize],
    ) -> u32 {
        let kinds = t.kinds();
        let mut max = 0;
        let extra_release: Vec<Vec<u32>> = extra.iter().map(|&b| t.release(b)).collect();
        for k in 0..kinds {
            let mut cycles = t.depths[self.block][k] + t.heights[self.block][k];
            for release in &extra_release {
                cycles += release[k];
            }
            max = max.max(cycles);
        }
        let max = t.get_cycles(max);
        let mut instrs = t.tb[self.block].instr_depth + t.tb[self.block].instr_height;
        for &b in extra {
            instrs += t.resources(b);
        }
        let width = t.f.model.issue_width;
        if width != 0 {
            instrs /= width;
        }
        instrs.max(max)
    }

    /// The trace's instruction depth of the center block's first instruction
    /// is not needed; the block's own number is.
    pub fn block(&self) -> usize {
        self.block
    }
}

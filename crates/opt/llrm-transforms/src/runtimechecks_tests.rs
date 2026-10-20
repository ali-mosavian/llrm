//! `addRuntimeChecks` and `addDiffRuntimeChecks` against the pinned LLVM:
//! the text of the check block `opt
//! -passes='loop-simplify,lcssa,loop-versioning'` (and `loop-vectorize` for the
//! diff form) writes for the same loop.

use llrm_analysis::runtimecheck::{AccessOrder, DepCandidates, RuntimePointerChecking, VectorizerParams};
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expander::{Env, Expander};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::memory::Callees;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;
use llrm_mir::valuetracking::AssumptionCache;

use super::Rewriter;

pub(crate) const DATALAYOUT: &str = "target datalayout = \"e-m:e-p:64:64-i64:64-n8:16:32:64\"\n\n";

/// `a[i] = b[i]` with a preheader.
/// `a[i*n + j] = b[(i*n + j) * 2] + 1` in a nest: the inner loop's bounds are
/// recurrences of the outer one.
pub(crate) const NEST: &str = "define void @f(ptr %a, ptr %b, i64 %n, i64 %m) {
entry:
  %g = icmp sgt i64 %m, 0
  %h = icmp sgt i64 %n, 0
  %gh = and i1 %g, %h
  br i1 %gh, label %outer.ph, label %exit
outer.ph:
  br label %outer
outer:
  %i = phi i64 [ 0, %outer.ph ], [ %i.next, %outer.latch ]
  %row = mul nuw nsw i64 %i, %n
  br label %loop
loop:
  %j = phi i64 [ 0, %outer ], [ %j.next, %loop ]
  %k = add nuw nsw i64 %row, %j
  %k2 = shl nuw nsw i64 %k, 1
  %pb = getelementptr inbounds i32, ptr %b, i64 %k2
  %v = load i32, ptr %pb, align 4
  %w = add i32 %v, 1
  %pa = getelementptr inbounds i32, ptr %a, i64 %k
  store i32 %w, ptr %pa, align 4
  %j.next = add nuw nsw i64 %j, 1
  %c = icmp eq i64 %j.next, %n
  br i1 %c, label %outer.latch, label %loop
outer.latch:
  %i.next = add nuw nsw i64 %i, 1
  %d = icmp eq i64 %i.next, %m
  br i1 %d, label %exit, label %outer
exit:
  ret void
}
";

pub(crate) const COPY: &str = "define void @f(ptr %a, ptr %b, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %ph, label %exit
ph:
  br label %loop
loop:
  %i = phi i64 [ 0, %ph ], [ %i.next, %loop ]
  %pb = getelementptr inbounds i32, ptr %b, i64 %i
  %v = load i32, ptr %pb, align 4
  %pa = getelementptr inbounds i32, ptr %a, i64 %i
  store i32 %v, ptr %pa, align 4
  %i.next = add nuw nsw i64 %i, 1
  %c = icmp eq i64 %i.next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
";

/// What `MemoryDepChecker` records of the accesses: their order in the loop.
struct Accesses {
    header: BlockId,
    list: Vec<(Operand, bool, u32, InstId)>,
}

impl AccessOrder for Accesses {
    fn order_for_access(
        &self,
        ptr: Operand,
        is_write: bool,
    ) -> &[u32] {
        match self.list.iter().position(|one| one.0 == ptr && one.1 == is_write) {
            Some(at) => std::slice::from_ref(&self.list[at].2),
            None => &[],
        }
    }

    fn instructions_for_access(
        &self,
        ptr: Operand,
        is_write: bool,
    ) -> Vec<InstId> {
        self.list.iter().filter(|one| one.0 == ptr && one.1 == is_write).map(|one| one.3).collect()
    }

    fn innermost_loop(&self) -> BlockId {
        self.header
    }
}

/// A function's loop `loop` with its memory checks worked out (what
/// `LoopAccessInfo` will hand over), and what a rewrite of it needs.
pub(crate) struct Analysed {
    pub module: llrm_mir::module::Module,
    pub context: Context,
    pub function: Function,
    pub globals: Vec<GlobalValue>,
    pub metadata: Vec<MetadataNode>,
    pub layout: DataLayout,
    pub tree: DomTree,
    pub loops: LoopInfo,
    pub assumptions: AssumptionCache,
    pub se: ScalarEvolution,
    pub rtc: RuntimePointerChecking,
    pub header: BlockId,
    /// `getMemoryInstructions`.
    pub memory: Vec<InstId>,
    pub declared: Declared,
    pub callees: Callees,
}

/// `text`'s `@f` with the checks of its loop `loop`; the pointers of its
/// stores, then of its loads, each in a dependency set of its own.
pub(crate) fn analysed(text: &str) -> Analysed {
    analysed_freezing(text, false)
}

/// [`analysed`] with every pointer to be frozen after expansion or not.
pub(crate) fn analysed_freezing(
    text: &str,
    needs_freeze: bool,
) -> Analysed {
    let module = llrm_mir::parse::module(&format!("{DATALAYOUT}{text}")).expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("@f");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("a layout");
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let header = *function
        .layout()
        .iter()
        .find(|&&one| function.block(one).name.as_deref() == Some("loop"))
        .expect("a block named loop");
    let mut se = ScalarEvolution::new();
    let mut rtc = RuntimePointerChecking::new(VectorizerParams::default());
    let mut memory = Vec::new();
    {
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout: &layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let btc = se.get_backedge_taken_count(&cx, header, ExitCountKind::Exact);
        let max_btc = se.get_backedge_taken_count(&cx, header, ExitCountKind::SymbolicMaximum);
        let mut accesses = Vec::new();
        for (position, inst) in function.iter_block(header).enumerate() {
            let instruction = function.instruction(inst);
            match instruction.opcode {
                Opcode::Load { .. } => accesses.push((instruction.operands[0], false, position as u32, inst)),
                Opcode::Store { .. } => accesses.push((instruction.operands[1], true, position as u32, inst)),
                _ => {}
            }
        }
        memory.extend(accesses.iter().map(|one| one.3));
        let ordered: Vec<_> = accesses.iter().filter(|one| one.1).chain(accesses.iter().filter(|one| !one.1)).collect();
        for (set, &&(ptr, write, _, inst)) in ordered.iter().enumerate() {
            let ptr_expr = se.get_scev(&cx, ptr);
            let ty = if write {
                function.operand_type(&module.context, function.instruction(inst).operands[0]).expect("a stored type")
            } else {
                function.instruction(inst).ty
            };
            assert!(rtc.insert(
                &mut se,
                &cx,
                header,
                ptr,
                ptr_expr,
                ty,
                write,
                set as u32 + 1,
                0,
                btc,
                max_btc,
                needs_freeze
            ));
        }
        let order = Accesses { header, list: accesses };
        rtc.generate_checks(&mut se, &cx, &order, &DepCandidates::default());
    }
    Analysed {
        context: module.context.clone(),
        function: function.clone(),
        globals: module.globals.clone(),
        metadata: module.metadata.clone(),
        layout,
        tree,
        loops,
        assumptions,
        se,
        rtc,
        header,
        memory,
        declared: Declared::of(&module),
        callees: Callees::default(),
        module,
    }
}

impl Analysed {
    /// The module with the rewritten function and the metadata made, which
    /// must verify.
    pub fn finished(&mut self) -> llrm_mir::module::Module {
        let mut module = self.module.clone();
        module.context = self.context.clone();
        *module.function_mut("f").expect("@f").1 = self.function.clone();
        self.declared.place(&mut module).expect("the declarations");
        module
    }

    /// The block named `name`.
    pub fn block(
        &self,
        name: &str,
    ) -> BlockId {
        *self.function.layout().iter().find(|&&one| self.function.block(one).name.as_deref() == Some(name)).expect(name)
    }

    /// `body` over a [`Rewriter`] of this function and its checks.
    pub fn rewrite<R>(
        &mut self,
        body: impl FnOnce(&mut Rewriter, &RuntimePointerChecking) -> R,
    ) -> R {
        let env = Env {
            globals: &self.globals,
            metadata: &self.metadata,
            layout: &self.layout,
            loops: &self.loops,
            tree: &self.tree,
            assumptions: &self.assumptions,
        };
        let mut rewriter = Rewriter {
            context: &mut self.context,
            declared: &mut self.declared,
            function: &mut self.function,
            se: &mut self.se,
            env,
            callees: &self.callees,
        };
        body(&mut rewriter, &self.rtc)
    }

    /// The instructions of `block`, one per line.
    pub fn lines(
        &self,
        block: BlockId,
    ) -> Vec<String> {
        let text = llrm_mir::print::body(&self.context, &self.function);
        let name = self.function.block(block).name.clone().expect("a named block");
        text.lines()
            .skip_while(|line| line.trim() != format!("{name}:"))
            .skip(1)
            .take_while(|line| !line.trim().ends_with(':') && !line.trim().is_empty())
            .map(|line| line.trim().to_owned())
            .collect()
    }
}

/// The pin, for `a[i] = b[i]`: `%0 = shl %n, 2` once, the two bounds from it,
/// and one conflict test. The pin names the bounds `%scevgep` and
/// `%scevgep1`; the expander names nothing.
#[test]
fn a_copy_between_two_arrays_is_one_conflict_test_of_two_bounds() {
    let mut a = analysed(COPY);
    let ph = a.block("ph");
    let (header, loc) = (a.header, a.function.terminator(ph).expect("a jump"));
    let checks = a.rtc.checks.clone();
    let found = a.rewrite(|rw, rtc| {
        let mut exp = Expander::new("induction");
        rw.add_runtime_checks(&mut exp, loc, header, rtc, &checks, false)
    });
    assert!(found.is_some());
    let lines = a.lines(ph);
    let pin = [
        "%0 = shl i64 %n, 2",
        "%1 = getelementptr i8, ptr %a, i64 %0",
        "%2 = getelementptr i8, ptr %b, i64 %0",
        "%bound0 = icmp ult ptr %a, %2",
        "%bound1 = icmp ult ptr %b, %1",
        "%found.conflict = and i1 %bound0, %bound1",
    ];
    assert_eq!(lines[..lines.len() - 1], pin, "{lines:#?}");
}

/// The pin (`loop-vectorize` at width 4), for the nest: with hoisting the
/// bounds cover the outer loop's trips and are computed in its preheader; the
/// two strides may be negative, so each is tested too.
#[test]
fn a_hoisted_check_covers_the_outer_loop_and_tests_the_strides() {
    let mut a = analysed(NEST);
    let (header, outer) = (a.header, a.block("outer"));
    let loc = a.function.terminator(outer).expect("a jump");
    let checks = a.rtc.checks.clone();
    a.rewrite(|rw, rtc| {
        let mut exp = Expander::new("induction");
        rw.add_runtime_checks(&mut exp, loc, header, rtc, &checks, true)
    })
    .expect("a check");
    assert_eq!(
        a.lines(a.block("outer.ph")),
        [
            "%0 = mul i64 %m, %n",
            "%1 = shl i64 %0, 2",
            "%2 = getelementptr i8, ptr %a, i64 %1",
            "%3 = shl i64 %n, 2",
            "%4 = shl i64 %0, 3",
            "%5 = add i64 %4, -4",
            "%6 = getelementptr i8, ptr %b, i64 %5",
            "%7 = shl i64 %n, 3",
            "br label %outer",
        ]
    );
    assert_eq!(
        a.lines(outer)[2..],
        [
            "%bound0 = icmp ult ptr %a, %6",
            "%bound1 = icmp ult ptr %b, %2",
            "%found.conflict = and i1 %bound0, %bound1",
            "%stride.check = icmp slt i64 %3, 0",
            "%8 = or i1 %found.conflict, %stride.check",
            "%stride.check1 = icmp slt i64 %7, 0",
            "%9 = or i1 %8, %stride.check1",
            "br label %loop",
        ]
    );
}

/// Without hoisting the bounds are those of one trip of the outer loop: they
/// stay in the outer loop's body, and no stride is tested.
#[test]
fn without_hoisting_the_bounds_stay_in_the_outer_loop() {
    let mut a = analysed(NEST);
    let (header, outer) = (a.header, a.block("outer"));
    let loc = a.function.terminator(outer).expect("a jump");
    let checks = a.rtc.checks.clone();
    a.rewrite(|rw, rtc| {
        let mut exp = Expander::new("induction");
        rw.add_runtime_checks(&mut exp, loc, header, rtc, &checks, false)
    })
    .expect("a check");
    let body = a.lines(outer).join("\n");
    assert!(body.contains("%bound0") && !body.contains("stride.check"), "{body}");
    assert!(!a.lines(a.block("outer.ph")).iter().any(|line| line.contains("getelementptr")));
}

/// `a[i*n + j] = b[i*n + j] + 1`: one pair, one stride.
const NEST_DIFF: &str = "define void @f(ptr %a, ptr %b, i64 %n, i64 %m) {
entry:
  %g = icmp sgt i64 %m, 0
  %h = icmp sgt i64 %n, 0
  %gh = and i1 %g, %h
  br i1 %gh, label %outer.ph, label %exit
outer.ph:
  br label %outer
outer:
  %i = phi i64 [ 0, %outer.ph ], [ %i.next, %outer.latch ]
  %row = mul nuw nsw i64 %i, %n
  br label %loop
loop:
  %j = phi i64 [ 0, %outer ], [ %j.next, %loop ]
  %k = add nuw nsw i64 %row, %j
  %pb = getelementptr inbounds i32, ptr %b, i64 %k
  %v = load i32, ptr %pb, align 4
  %w = add i32 %v, 1
  %pa = getelementptr inbounds i32, ptr %a, i64 %k
  store i32 %w, ptr %pa, align 4
  %j.next = add nuw nsw i64 %j, 1
  %c = icmp eq i64 %j.next, %n
  br i1 %c, label %outer.latch, label %loop
outer.latch:
  %i.next = add nuw nsw i64 %i, 1
  %d = icmp eq i64 %i.next, %m
  br i1 %d, label %exit, label %outer
exit:
  ret void
}
";

/// The pin (`loop-vectorize`, VF 4, IC 1): the difference of the two
/// pointers is invariant and computed once outside the outer loop; the test
/// is `(diff - 1) <u 15`.
#[test]
fn a_diff_check_tests_the_pointer_difference_against_the_span_less_one() {
    let mut a = analysed(NEST_DIFF);
    let outer = a.block("outer");
    let loc = a.function.terminator(outer).expect("a jump");
    let diffs = a.rtc.diff_checks().expect("diff checks").to_vec();
    assert_eq!(diffs.len(), 1);
    a.rewrite(|rw, _| {
        let mut exp = Expander::new("induction");
        rw.add_diff_runtime_checks(&mut exp, loc, &diffs, 4, 1)
    })
    .expect("a check");
    // The pin writes `ptrtoaddr`; MIR has `ptrtoint` (DEVIATE 9).
    assert_eq!(a.lines(a.block("entry"))[..2], ["%0 = ptrtoint ptr %b to i64", "%1 = ptrtoint ptr %a to i64"]);
    assert_eq!(a.lines(a.block("outer.ph")), ["%2 = sub i64 %1, %0", "br label %outer"]);
    assert_eq!(a.lines(outer)[2..], ["%3 = sub i64 %2, 1", "%diff.check = icmp ult i64 %3, 15", "br label %loop"]);
}

/// Two checks of the same pointers are one compare: the second would be a
/// second `diff.check` or-ed to the first.
#[test]
fn a_compare_of_the_same_operands_is_made_once() {
    let mut a = analysed(NEST_DIFF);
    let outer = a.block("outer");
    let loc = a.function.terminator(outer).expect("a jump");
    let diff = a.rtc.diff_checks().expect("diff checks")[0];
    let found = a.rewrite(|rw, _| {
        let mut exp = Expander::new("induction");
        rw.add_diff_runtime_checks(&mut exp, loc, &[diff, diff], 4, 1)
    });
    assert!(found.is_some());
    let body = a.lines(outer);
    assert_eq!(body.iter().filter(|line| line.contains("diff.check")).count(), 1, "{body:#?}");
    assert!(!body.iter().any(|line| line.contains("conflict.rdx")), "{body:#?}");
}

/// A pointer that may be poison outside the loop is frozen after its bounds
/// are expanded, so the compares do not branch on poison.
#[test]
fn bounds_of_a_pointer_that_may_be_poison_are_frozen() {
    let mut a = analysed_freezing(COPY, true);
    let ph = a.block("ph");
    let (header, loc) = (a.header, a.function.terminator(ph).expect("a jump"));
    let checks = a.rtc.checks.clone();
    a.rewrite(|rw, rtc| {
        let mut exp = Expander::new("induction");
        rw.add_runtime_checks(&mut exp, loc, header, rtc, &checks, false)
    })
    .expect("a check");
    let lines = a.lines(ph);
    assert_eq!(lines.iter().filter(|line| line.contains(" = freeze ptr ")).count(), 4, "{lines:#?}");
    let compare = lines.iter().find(|line| line.contains("%bound0")).expect("bound0");
    assert!(compare.contains(".fr"), "{compare}");
}

/// `eraseDeadInstructions` removes what the expander made and nothing reads,
/// but the root.
#[test]
fn the_expansion_nothing_reads_is_erased_but_for_the_root() {
    for keep in [false, true] {
        let mut a = analysed(COPY);
        let ph = a.block("ph");
        let loc = a.function.terminator(ph).expect("a jump");
        let end = a.rtc.pointers[0].end;
        a.rewrite(|rw, _| {
            let mut exp = Expander::new("induction");
            exp.set_insert_point(loc);
            let value = rw.expand(&mut exp, end, None);
            rw.erase_dead(&mut exp, keep.then_some(value));
        });
        let lines = a.lines(ph);
        assert_eq!(lines.len(), if keep { 3 } else { 1 }, "{lines:#?}");
    }
}

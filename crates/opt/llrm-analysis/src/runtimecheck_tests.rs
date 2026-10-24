//! `RuntimePointerChecking` against the pinned LLVM's
//! `opt -passes='print<access-info>'` on the same loops (the text of its
//! "Run-time memory checks" and "Grouped accesses").

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::AssumptionCache;

use super::{AccessOrder, DepCandidates, RuntimePointerChecking, VectorizerParams};
use crate::scev::backedge::ExitCountKind;
use crate::scev::print::Labels;
use crate::scev::{Cx, ScalarEvolution};

const DATALAYOUT: &str = "target datalayout = \"e-m:e-p:64:64-i64:64-n8:16:32:64\"\n\n";

/// `a[i] = b[i]`, the pin's `rtc1`.
const COPY: &str = "define void @f(ptr %a, ptr %b, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
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

/// `c[i] = a[i] + a[i + 1]`, the pin's `rtc2`.
const WINDOW: &str = "define void @f(ptr %a, ptr %c, i64 %n) {
entry:
  %g = icmp sgt i64 %n, 0
  br i1 %g, label %loop, label %exit
loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %pa = getelementptr inbounds i32, ptr %a, i64 %i
  %v = load i32, ptr %pa, align 4
  %j = add nuw nsw i64 %i, 1
  %pa1 = getelementptr inbounds i32, ptr %a, i64 %j
  %w = load i32, ptr %pa1, align 4
  %s = add i32 %v, %w
  %pc = getelementptr inbounds i32, ptr %c, i64 %i
  store i32 %s, ptr %pc, align 4
  %i.next = add nuw nsw i64 %i, 1
  %e = icmp eq i64 %i.next, %n
  br i1 %e, label %exit, label %loop
exit:
  ret void
}
";

/// What `MemoryDepChecker` records of the accesses: their order in the loop.
struct Accesses {
    header: BlockId,
    /// (pointer, is write, position, instruction).
    list: Vec<(Operand, bool, u32, InstId)>,
}

impl AccessOrder for Accesses {
    fn order_for_access(
        &self,
        ptr: Operand,
        is_write: bool,
    ) -> &[u32] {
        // One position per access in these loops: none for the other kind.
        let found = self.list.iter().position(|one| one.0 == ptr && one.1 == is_write);
        match found {
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

/// The checks of `text`'s `@f` loop `loop`: the pointers of its stores, then
/// of its loads, each its own dependency set unless `classes` unites them; the
/// pin's text, and the diff checks as `(src, sink, size)`.
fn analysed(
    text: &str,
    classes: &[(&str, &str)],
) -> (String, Vec<(String, String, u32)>) {
    let module = llrm_mir::parse::module(&format!("{DATALAYOUT}{text}")).expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("@f");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a layout")).expect("a layout");
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
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
    let header =
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("loop")).expect("loop");
    let mut se = ScalarEvolution::new();
    let btc = se.get_backedge_taken_count(&cx, header, ExitCountKind::Exact);
    let max_btc = se.get_backedge_taken_count(&cx, header, ExitCountKind::SymbolicMaximum);

    // The accesses in program order, then stores before loads.
    let mut accesses = Vec::new();
    for (position, inst) in function.iter_block(header).enumerate() {
        let instruction = function.instruction(inst);
        match instruction.opcode {
            Opcode::Load { .. } => accesses.push((instruction.operands[0], false, position as u32, inst)),
            Opcode::Store { .. } => accesses.push((instruction.operands[1], true, position as u32, inst)),
            _ => {}
        }
    }
    let named = |name: &str| {
        let found = function
            .walk()
            .filter_map(|(_, inst)| function.instruction(inst).result)
            .find(|&value| function.value(value).name.as_deref() == Some(name))
            .expect(name);
        Operand::Value(found)
    };

    let mut rtc = RuntimePointerChecking::new(VectorizerParams::default());
    let mut dep_cands = DepCandidates::default();
    for (a, b) in classes {
        dep_cands.union_sets(&(named(a), false), &(named(b), false));
    }
    let mut ordered: Vec<_> = accesses.iter().filter(|one| one.1).chain(accesses.iter().filter(|one| !one.1)).collect();
    let mut dep_set = 0;
    for (ptr, write, _, inst) in ordered.drain(..) {
        let ptr_expr = se.get_scev(&cx, *ptr);
        let ty = crate::loopaccess::load_store_type(&cx, *inst).expect("a load or store");
        let id = classes
            .iter()
            .position(|(a, b)| *ptr == named(a) || *ptr == named(b))
            .map_or_else(
                || {
                    dep_set += 100;
                    dep_set
                },
                |class| class as u32 + 1,
            );
        assert!(rtc.insert(&mut se, &cx, header, *ptr, ptr_expr, ty, *write, id, 0, btc, max_btc, false));
    }
    let order = Accesses { header, list: accesses };
    rtc.generate_checks(&mut se, &cx, &order, &dep_cands);

    let labels = Labels::of(function);
    let printer = crate::laa_testing::PinText::of(&module, function);
    let text = rtc.print(&se, &cx, &labels, &printer, 0);
    let diffs = rtc
        .diff_checks()
        .unwrap_or_default()
        .iter()
        .map(|one| (se.print(&cx, &labels, one.src_start), se.print(&cx, &labels, one.sink_start), one.access_size))
        .collect();
    (text, diffs)
}

/// The pin, for `a[i] = b[i]`: one check of the written array's group against
/// the read one, each bounded `[p, p + 4n)`.
#[test]
fn a_copy_between_two_arrays_is_one_check_of_two_groups() {
    let (text, diffs) = analysed(COPY, &[]);
    assert_eq!(
        text,
        "Run-time memory checks:
Check 0:
  Comparing group GRP0:
    %pa = getelementptr inbounds i32, ptr %a, i64 %i
  Against group GRP1:
    %pb = getelementptr inbounds i32, ptr %b, i64 %i
Grouped accesses:
  Group GRP0:
    (Low: %a High: ((4 * %n) + %a))
      Member: {%a,+,4}<nuw><%loop>
  Group GRP1:
    (Low: %b High: ((4 * %n) + %b))
      Member: {%b,+,4}<nuw><%loop>
"
    );
    // The load comes first in the body: it is the source of the pair.
    assert_eq!(diffs, [("(ptrtoaddr ptr %b to i64)".to_owned(), "(ptrtoaddr ptr %a to i64)".to_owned(), 4)]);
}

/// The pin, for `c[i] = a[i] + a[i + 1]`: the two reads of `a` differ by a
/// constant, so they share a group bounded `[a, a + 4 + 4n)`.
#[test]
fn reads_a_constant_apart_share_a_group() {
    let (text, diffs) = analysed(WINDOW, &[("pa1", "pa")]);
    assert_eq!(
        text,
        "Run-time memory checks:
Check 0:
  Comparing group GRP0:
    %pc = getelementptr inbounds i32, ptr %c, i64 %i
  Against group GRP1:
    %pa1 = getelementptr inbounds i32, ptr %a, i64 %j
    %pa = getelementptr inbounds i32, ptr %a, i64 %i
Grouped accesses:
  Group GRP0:
    (Low: %c High: ((4 * %n) + %c))
      Member: {%c,+,4}<nuw><%loop>
  Group GRP1:
    (Low: %a High: (4 + (4 * %n) + %a))
      Member: {(4 + %a),+,4}<nuw><%loop>
      Member: {%a,+,4}<nuw><%loop>
"
    );
    // A group of two pointers has no difference check.
    assert!(diffs.is_empty());
}

/// `c[i] = a[i] + a[i + 1] + ... + a[i + n - 1]`: `n` reads a constant apart.
fn window(n: usize) -> String {
    let mut text = String::from(
        "define void @f(ptr %a, ptr %c, i64 %n) {\nentry:\n  %g = icmp sgt i64 %n, 0\n  br i1 %g, label %loop, label %exit\nloop:\n  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]\n",
    );
    for k in 0..n {
        text += &format!(
            "  %k{k} = add nuw nsw i64 %i, {k}\n  %pa{k} = getelementptr inbounds i32, ptr %a, i64 %k{k}\n  %v{k} = load i32, ptr %pa{k}, align 4\n"
        );
    }
    text += "  %pc = getelementptr inbounds i32, ptr %c, i64 %i\n";
    text += &format!("  store i32 %v{}, ptr %pc, align 4\n", n - 1);
    text + "  %i.next = add nuw nsw i64 %i, 1\n  %c1 = icmp eq i64 %i.next, %n\n  br i1 %c1, label %exit, label %loop\nexit:\n  ret void\n}\n"
}

/// The bounds `groupChecks` compared for `n` reads a constant apart.
fn comparisons_for(n: usize) -> usize {
    let before = super::group_comparisons();
    // The reads are one dependency class, as `AccessAnalysis` makes them.
    let names: Vec<String> = (0..n).map(|k| format!("pa{k}")).collect();
    let classes: Vec<(&str, &str)> = names.iter().skip(1).map(|name| ("pa0", name.as_str())).collect();
    let (checks, _) = analysed(&window(n), &classes);
    assert!(checks.contains("Group GRP1") && !checks.contains("Group GRP2"), "{checks}");
    super::group_comparisons() - before
}

/// The reads a constant apart share one group, and the grouping is linear in
/// them: four times the reads make about four times the comparisons, not
/// sixteen (a slope test at 1x and 4x, counting comparisons).
#[test]
fn the_comparisons_grow_with_the_reads_not_with_their_square() {
    let (small, large) = (comparisons_for(25), comparisons_for(100));
    assert!(small > 0 && large < small * 5, "25 reads: {small} comparisons, 100 reads: {large}");
}

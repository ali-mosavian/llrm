use super::*;
use crate::hash::HashMap;
use crate::module::Operand;

/// A graph given by its successor lists, for edits a `Function` cannot make
/// without a pass.
#[derive(Clone)]
struct Cfg {
    succs: Vec<Vec<u32>>,
    blocks: Vec<BlockId>,
}

impl Cfg {
    fn new(succs: &[&[u32]]) -> Self {
        Self {
            succs: succs.iter().map(|one| one.to_vec()).collect(),
            blocks: (0..succs.len() as u32).map(BlockId).collect(),
        }
    }

    fn has(
        &self,
        from: u32,
        to: u32,
    ) -> bool {
        self.succs[from as usize].contains(&to)
    }

    fn insert(
        &mut self,
        from: u32,
        to: u32,
    ) {
        self.succs[from as usize].push(to);
    }

    fn delete(
        &mut self,
        from: u32,
        to: u32,
    ) {
        self.succs[from as usize].retain(|one| *one != to);
    }

    /// The update that flips the edge, and the graph after it.
    fn flipped(
        &self,
        from: u32,
        to: u32,
    ) -> (Update, Self) {
        let mut after = self.clone();
        if self.has(from, to) {
            after.delete(from, to);
            (Update::delete(BlockId(from), BlockId(to)), after)
        } else {
            after.insert(from, to);
            (Update::insert(BlockId(from), BlockId(to)), after)
        }
    }
}

impl DomGraph for Cfg {
    fn entry(&self) -> Option<BlockId> {
        Some(BlockId(0))
    }

    fn max_number(&self) -> usize {
        self.succs.len()
    }

    fn nodes(&self) -> &[BlockId] {
        &self.blocks
    }

    fn successors(
        &self,
        block: BlockId,
    ) -> BlockList {
        self.succs[block.0 as usize].iter().map(|one| BlockId(*one)).collect()
    }

    fn predecessors(
        &self,
        block: BlockId,
    ) -> BlockList {
        // Newest use first: the order of an LLVM block's use list, which the
        // harness builds in block order.
        (0..self.succs.len() as u32).rev().filter(|from| self.has(*from, block.0)).map(BlockId).collect()
    }
}

/// Eight blocks: a loop with an exit, a diamond, a switch-like fan, an
/// unreachable block (7) that reaches the loop and an infinite loop (5, 6).
fn graph() -> Cfg {
    Cfg::new(&[&[1, 2], &[3], &[3, 4], &[1, 5], &[3, 6], &[6], &[5], &[2]])
}

fn assert_verifies<const POST: bool>(
    tree: &DomTreeBase<POST>,
    cfg: &Cfg,
    what: &str,
) {
    if let Err(error) = tree.verify(cfg, VerificationLevel::Full) {
        panic!("{what}: {error}\n{}", tree.print());
    }
}

fn fresh<const POST: bool>(cfg: &Cfg) -> DomTreeBase<POST> {
    let mut tree = DomTreeBase::<POST>::new();
    tree.recalculate(cfg);
    tree
}

/// `a` dominates `b` where `b` cannot be reached from the entry once `a` is
/// gone.
fn by_definition(
    cfg: &Cfg,
    a: u32,
    b: u32,
) -> bool {
    if a == 0 {
        return true;
    }
    let mut seen = vec![false; cfg.succs.len()];
    let mut work = vec![0_u32];
    seen[0] = true;
    while let Some(at) = work.pop() {
        for &next in &cfg.succs[at as usize] {
            if next != a && !seen[next as usize] {
                seen[next as usize] = true;
                work.push(next);
            }
        }
    }
    !seen[b as usize]
}

#[test]
fn a_built_tree_dominates_what_the_definition_says_for_every_pair() {
    let cfg = graph();
    let tree = fresh::<false>(&cfg);
    assert_verifies(&tree, &cfg, "built");
    let reachable = |b: u32| tree.contains(BlockId(b));
    for a in 0..8 {
        for b in 0..8 {
            if reachable(a) && reachable(b) {
                assert_eq!(tree.dominates(BlockId(a), BlockId(b)), by_definition(&cfg, a, b), "{a} over {b}");
            }
        }
    }
    // Everything dominates an unreachable block, and it dominates nothing.
    assert!(!reachable(7) && tree.dominates(BlockId(3), BlockId(7)) && !tree.dominates(BlockId(7), BlockId(3)));
}

#[test]
fn a_post_dominator_tree_roots_each_infinite_loop_once() {
    let cfg = graph();
    let tree = fresh::<true>(&cfg);
    assert_verifies(&tree, &cfg, "built");
    // 5 and 6 are one infinite loop: one root; 7 reaches 2, so it has an exit.
    assert_eq!(tree.roots().len(), 1, "{}", tree.print());
    assert!(tree.contains(BlockId(7)), "a block that reaches the loop is in the tree");
}

/// The cases `tools/llvmport/domtree_dump.cpp` runs: every single edge flip of
/// `graph()`, then every pair, and a triple for some pairs, each as a batch.
fn cases() -> Vec<Vec<(u32, u32)>> {
    let edges: Vec<(u32, u32)> = (0..8).flat_map(|from| (0..8).map(move |to| (from, to))).collect();
    let mut cases: Vec<Vec<(u32, u32)>> = edges.iter().map(|edge| vec![*edge]).collect();
    for i in 0..edges.len() {
        for j in i + 1..edges.len() {
            cases.push(vec![edges[i], edges[j]]);
            if (i + j) % 7 == 0 {
                let c = edges[(i * 5 + j * 3) % edges.len()];
                if c != edges[i] && c != edges[j] {
                    cases.push(vec![edges[i], edges[j], c]);
                }
            }
        }
    }
    cases
}

/// The graph after the flips, the updates, and the key the golden file lists
/// them by.
fn applied(batch: &[(u32, u32)]) -> (Cfg, Vec<Update>, String) {
    let (mut after, mut updates, mut key) = (graph(), Vec::new(), String::new());
    for &(from, to) in batch {
        let (update, next) = after.flipped(from, to);
        key.push_str(&format!("{from}{}{to},", if update.kind == UpdateKind::Insert { '+' } else { '-' }));
        updates.push(update);
        after = next;
    }
    (after, updates, key)
}

/// The tree as `domtree_dump.cpp` prints it: per block its immediate dominator,
/// level and children, then the roots.
fn dump<const POST: bool>(
    tree: &DomTreeBase<POST>,
    sorted: bool,
) -> String {
    dump_of(tree, sorted, 8)
}

/// `dump` of a graph of nine blocks.
fn dump9<const POST: bool>(
    tree: &DomTreeBase<POST>,
    sorted: bool,
) -> String {
    dump_of(tree, sorted, 9)
}

fn dump_of<const POST: bool>(
    tree: &DomTreeBase<POST>,
    sorted: bool,
    blocks: u32,
) -> String {
    let list = |mut items: Vec<u32>| {
        if sorted {
            items.sort_unstable();
        }
        items.iter().map(|one| format!("{one},")).collect::<String>()
    };
    let line = |name: String, block: Bb| match block.map(|one| tree.contains(one)) {
        Some(false) => format!("{name}:x\n"),
        _ => {
            let idom =
                block.and_then(|one| tree.immediate_dominator(one)).map_or("-".to_string(), |one| one.0.to_string());
            let level = block.map_or(Some(0), |one| tree.level(one)).expect("in the tree");
            format!("{name}:{idom}:{level}:{}\n", list(tree.children(block).iter().map(|one| one.0).collect()))
        }
    };
    let mut out = String::new();
    if POST {
        out.push_str(&line("V".into(), None));
    }
    for block in 0..blocks {
        out.push_str(&line(block.to_string(), Some(BlockId(block))));
    }
    out.push_str(&format!("roots:{}\n", list(tree.roots().iter().map(|one| one.0).collect())));
    out
}

/// FNV-1a of the dump, as the C++ side computes it.
fn hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// `domtree_golden.txt` as `(kind, key) -> [updated, updated sorted, fresh
/// sorted, lazy, permissive]`, and the split lines as `("split <kind>",
/// "<from>><to> <strategy>") -> [hash]`.
fn golden() -> (String, HashMap<(String, String), Vec<String>>) {
    let text = include_str!("domtree_golden.txt");
    let mut lines = text.lines();
    let version = lines.next().expect("a version line").to_string();
    let rows = lines
        .map(|line| {
            let f: Vec<&str> = line.split(' ').collect();
            if f[0] == "split" {
                ((format!("split {}", f[1]), format!("{} {}", f[2], f[3])), vec![f[4].to_string()])
            } else {
                ((f[0].to_string(), f[1].to_string()), f[2..].iter().map(|one| one.to_string()).collect())
            }
        })
        .collect();
    (version, rows)
}

/// The instrument's premise: the golden file is the pinned LLVM's, has a line
/// per case and tree, and holds the divergence that makes it worth having:
/// LLVM's updated post-dominator tree differs from a fresh one in some
/// cases and its dominator tree in none.
#[test]
fn the_golden_file_is_the_pinned_llvms_and_lists_every_case() {
    let (version, rows) = golden();
    assert_eq!(
        version, "# LLVM 24.0.0git d1106deb71cc81016406a1917d0508d2df296266",
        "golden file not from the pinned binary"
    );
    assert_eq!(rows.keys().filter(|(kind, _)| !kind.starts_with("split")).count(), 2 * cases().len());
    let diverges = |kind: &str| rows.iter().filter(|((k, _), r)| k == kind && r[1] != r[2]).count();
    assert_eq!((diverges("dom"), diverges("post") > 0), (0, true));
}

/// Every update of every case leaves the tree LLVM leaves, children in LLVM's
/// order, for the dominator and the post-dominator tree, by `applyUpdates`.
#[test]
fn updated_trees_are_the_trees_llvm_leaves_for_every_case() {
    let (_, rows) = golden();
    let cfg = graph();
    for batch in cases() {
        let (after, updates, key) = applied(&batch);
        let (mut dom, mut post) = (fresh::<false>(&cfg), fresh::<true>(&cfg));
        dom.apply_updates(&after, &updates);
        post.apply_updates(&after, &updates);
        let want = &rows[&("dom".to_string(), key.clone())];
        assert_eq!(hash(&dump(&dom, false)), want[0], "dom {key}\n{}", dump(&dom, false));
        assert_eq!(hash(&dump(&dom, true)), want[2], "dom {key} against a fresh tree");
        assert_verifies(&dom, &after, &format!("dom {key}"));
        let want = &rows[&("post".to_string(), key.clone())];
        assert_eq!(hash(&dump(&post, false)), want[0], "post {key}\n{}", dump(&post, false));
        assert_eq!(hash(&dump(&post, true)), want[1], "post {key} sorted");
    }
}

/// The single-edge entry points leave the same tree as the batch of one.
#[test]
fn insert_edge_and_delete_edge_leave_the_tree_llvm_leaves() {
    let (_, rows) = golden();
    let cfg = graph();
    for batch in cases().into_iter().filter(|batch| batch.len() == 1) {
        let (after, updates, key) = applied(&batch);
        let (mut dom, mut post) = (fresh::<false>(&cfg), fresh::<true>(&cfg));
        for tree_update in &updates {
            let (from, to) = (tree_update.from, tree_update.to);
            if tree_update.kind == UpdateKind::Insert {
                dom.insert_edge(&after, from, to);
                post.insert_edge(&after, from, to);
            } else {
                dom.delete_edge(&after, from, to);
                post.delete_edge(&after, from, to);
            }
        }
        assert_eq!(hash(&dump(&dom, false)), rows[&("dom".to_string(), key.clone())][0], "dom {key}");
        assert_eq!(hash(&dump(&post, false)), rows[&("post".to_string(), key.clone())][0], "post {key}");
    }
}

/// libstdc++'s `std::priority_queue` over `{level, id}` ordered by level only,
/// pushing ids 0..16 at these levels and popping once after the 10th and the
/// 13th push: the order is LLVM's `InsertionInfo::Bucket`, which decides the
/// order of the children of the node an insertion moves nodes under.
#[test]
fn nodes_of_one_level_leave_the_queue_in_the_order_the_standard_heap_gives() {
    let levels = [3_u32, 5, 5, 2, 5, 3, 4, 5, 1, 4, 4, 5, 3, 2, 5, 4];
    let mut queue = LevelQueue::new();
    let mut popped = Vec::new();
    for (id, level) in levels.iter().enumerate() {
        queue.push(*level, id as Node);
        if id == 9 || id == 12 {
            popped.push(queue.pop());
        }
    }
    while !queue.is_empty() {
        popped.push(queue.pop());
    }
    assert_eq!(popped, [1, 2, 4, 14, 11, 7, 15, 6, 9, 10, 0, 12, 5, 13, 3, 8]);
}

/// A new edge from an unreachable block to a reachable one joined the
/// unreachable block's subtree to the tree under the wrong dominator.
#[test]
fn an_edge_out_of_an_unreachable_block_into_the_tree_is_ignored() {
    let cfg = graph();
    let mut tree = fresh::<false>(&cfg);
    let before = tree.print();
    let (_, after) = cfg.flipped(7, 3);
    tree.insert_edge(&after, BlockId(7), BlockId(3));
    assert_eq!(tree.print(), before);
    assert_verifies(&tree, &after, "ignored");
}

#[test]
fn a_function_is_a_graph_of_its_blocks() {
    let text = "define void @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br label %join\nb:\n  br label %join\njoin:\n  ret void\n}\n";
    let module = crate::parse::module(text).expect("parses");
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("a body");
    let layout = function.layout().to_vec();
    let (mut dom, mut post) = (DomTree::new(), PostDomTree::new());
    dom.recalculate(function);
    post.recalculate(function);
    dom.verify(function, VerificationLevel::Full).expect("dom");
    post.verify(function, VerificationLevel::Full).expect("post");
    assert_eq!(dom.immediate_dominator(layout[3]), Some(layout[0]), "the join's dominator is the entry");
    assert_eq!(post.immediate_dominator(layout[0]), Some(layout[3]), "the entry's post-dominator is the join");
}

fn function(text: &str) -> Function {
    let module = crate::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    module
        .functions()
        .find(|(_, _, function)| !function.is_declaration())
        .map(|(_, _, function)| function.clone())
        .expect("a body")
}

/// Asking was a walk up the tree through a hash map, as deep as the loops nest:
/// 45% of a compile of 60 sequential loops, and 301 s of one of 200 (#394). It
/// is a comparison of two numbers.
#[test]
fn asking_whether_a_block_dominates_does_not_walk_the_tree() {
    let depth = 20_000;
    let body: String = (0..depth).map(|at| format!("b{at}:\n  br label %b{}\n", at + 1)).collect();
    let function = function(&format!("define void @f() {{\nentry:\n  br label %b0\n{body}b{depth}:\n  ret void\n}}\n"));
    let tree = DomTree::of(&function);
    let last = *function.layout().last().expect("blocks");
    let start = std::time::Instant::now();
    let asked = function.layout().iter().filter(|&&at| tree.dominates(at, last)).count();
    assert_eq!(asked, depth + 2);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(1),
        "{:?} for {depth} questions of depth {depth}",
        start.elapsed()
    );
}

const DIAMOND_LOOP: &str = "define void @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br label %join\nb:\n  br i1 %c, label %join, label %loop\nloop:\n  br i1 %c, label %loop, label %out\njoin:\n  br label %out\nout:\n  ret void\n}\n";

/// The tree after an edit the log tells of, against a tree of the edited
/// function built afresh. Each edit is one the passes make: a branch to another
/// block, a block put between two, a branch cut and a branch made.
#[test]
fn a_tree_updated_from_the_log_is_the_tree_of_the_function_afresh() {
    // (block whose terminator is rewritten, operand index, new target block
    // name) over DIAMOND_LOOP.
    let edits: [(&str, usize, &str); 5] =
        [("a", 0, "out"), ("b", 1, "out"), ("b", 2, "join"), ("loop", 1, "join"), ("join", 0, "loop")];
    for (block, slot, target) in edits {
        let mut function = function(DIAMOND_LOOP);
        let by_name = |function: &Function, name: &str| {
            *function.layout().iter().find(|one| function.block(**one).name.as_deref() == Some(name)).expect("a block")
        };
        let before = DomTree::of(&function);
        let mark = function.mark();
        let (at, to) = (by_name(&function, block), by_name(&function, target));
        let last = function.terminator(at).expect("terminated");
        let mut operands = function.instruction(last).operands.clone();
        operands[slot] = Operand::Block(to);
        function.set_operands(last, operands);
        let changes = function.changes_since(mark).expect("on record").to_vec();
        let updated =
            before.updated(&function, &changes).unwrap_or_else(|| panic!("{block}[{slot}] -> {target}: not updated"));
        let fresh = DomTree::of(&function);
        assert_eq!(updated, fresh, "{block}[{slot}] -> {target}");
        updated
            .verify(&function, VerificationLevel::Full)
            .unwrap_or_else(|error| panic!("{block}[{slot}] -> {target}: {error}"));
    }
}

use crate::domtree_updater::{DomTreeUpdater, UpdateStrategy};

/// The same updates through a Lazy updater, one at a time, then the tree asked
/// for; and through `applyUpdatesPermissive` with the first update repeated:
/// the trees the pinned LLVM's `DomTreeUpdater` leaves.
#[test]
fn a_lazy_updater_leaves_the_tree_llvm_leaves_for_every_case() {
    let (_, rows) = golden();
    let cfg = graph();
    for batch in cases() {
        let (after, updates, key) = applied(&batch);
        let mut repeated = updates.clone();
        repeated.push(updates[0]);
        for (permissive, column) in [(false, 3), (true, 4)] {
            let (mut dom, mut post) = (
                DomTreeUpdater::new(Some(fresh::<false>(&cfg)), None, UpdateStrategy::Lazy),
                DomTreeUpdater::new(None, Some(fresh::<true>(&cfg)), UpdateStrategy::Lazy),
            );
            for updater in [&mut dom, &mut post] {
                if permissive {
                    updater.apply_updates_permissive(&after, &repeated);
                } else {
                    for update in &updates {
                        updater.apply_updates(&after, std::slice::from_ref(update));
                    }
                }
            }
            let (want_dom, want_post) =
                (&rows[&("dom".to_string(), key.clone())], &rows[&("post".to_string(), key.clone())]);
            assert_eq!(
                hash(&dump(dom.get_dom_tree(&after), false)),
                want_dom[column],
                "dom {key} permissive {permissive}"
            );
            assert_eq!(
                hash(&dump(post.get_post_dom_tree(&after), false)),
                want_post[column],
                "post {key} permissive {permissive}"
            );
            dom.into_trees(&after);
            post.into_trees(&after);
        }
    }
}

/// A critical edge split with block 8, made after the trees were: Eager and
/// Lazy leave the pinned LLVM's tree.
#[test]
fn splitting_a_critical_edge_leaves_the_tree_llvm_leaves() {
    let (_, rows) = golden();
    let cfg = graph();
    let mut seen = 0;
    for from in 0..8_u32 {
        for &to in &cfg.succs[from as usize] {
            let mut after = cfg.clone();
            for target in after.succs[from as usize].iter_mut() {
                if *target == to {
                    *target = 8;
                    break;
                }
            }
            after.succs.push(vec![to]);
            after.blocks.push(BlockId(8));
            for lazy in [false, true] {
                let strategy = if lazy { UpdateStrategy::Lazy } else { UpdateStrategy::Eager };
                let name = format!("{from}>{to} {}", if lazy { "lazy" } else { "eager" });
                if from != 7 {
                    let mut dom = DomTreeUpdater::new(Some(fresh::<false>(&cfg)), None, strategy);
                    dom.split_critical_edge(&after, BlockId(from), BlockId(to), BlockId(8));
                    let want = &rows[&("split dom".to_string(), name.clone())];
                    assert_eq!(hash(&dump9(dom.get_dom_tree(&after), false)), want[0], "dom split {name}");
                    dom.into_trees(&after);
                    seen += 1;
                }
                let mut post = DomTreeUpdater::new(None, Some(fresh::<true>(&cfg)), strategy);
                post.split_critical_edge(&after, BlockId(from), BlockId(to), BlockId(8));
                let want = &rows[&("split post".to_string(), name.clone())];
                assert_eq!(hash(&dump9(post.get_post_dom_tree(&after), false)), want[0], "post split {name}");
                post.into_trees(&after);
                seen += 1;
            }
        }
    }
    assert_eq!(
        seen,
        rows.keys().filter(|(kind, _)| kind.starts_with("split")).count(),
        "premise: every split line was compared"
    );
}

/// Blocks waiting to be deleted leave the trees only when no update is pending:
/// a flush event, as LLVM's.
#[test]
fn a_block_deleted_under_a_lazy_updater_leaves_the_trees_at_the_flush() {
    let mut cfg = graph();
    cfg.delete(0, 2);
    cfg.delete(2, 3);
    cfg.delete(2, 4);
    cfg.delete(7, 2);
    cfg.succs[2].clear();
    let mut updater = DomTreeUpdater::new(Some(fresh::<false>(&graph())), None, UpdateStrategy::Lazy);
    updater.apply_updates(
        &cfg,
        &[
            Update::delete(BlockId(0), BlockId(2)),
            Update::delete(BlockId(2), BlockId(3)),
            Update::delete(BlockId(2), BlockId(4)),
        ],
    );
    updater.delete_bb(BlockId(2));
    assert!(updater.is_bb_pending_deletion(BlockId(2)) && updater.has_pending_dom_tree_updates());
    let tree = updater.get_dom_tree(&cfg);
    assert!(!tree.contains(BlockId(2)), "block 2 is out of the tree after the flush");
    assert!(!updater.has_pending_deleted_bb());
    updater.into_trees(&cfg);
}

fn builds() -> (u64, u64) {
    use crate::counts::{Event, get};
    (get(Event::Dominators), get(Event::DominatorsReused))
}

/// 394 trees were built for d_faces, 322 of them for a CFG built before: 5 M Ir
/// of SemiNCA for answers already held.
#[test]
fn a_tree_of_an_unchanged_cfg_is_copied_not_built() {
    let function = function(
        "define i32 @f(i1 %c, i32 %x) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br label %m\nb:\n  br label %m\nm:\n  %y = add i32 %x, 1\n  ret i32 %y\n}\n",
    );
    let (built, reused) = builds();
    let first = DomTree::of(&function);
    let second = DomTree::of(&function);
    let (built_now, reused_now) = builds();
    assert_eq!((built_now - built, reused_now - reused), (1, 1), "one build, one copy");
    assert!(first == second);
    second.verify(&function, VerificationLevel::Full).expect("the copy is a tree of the function");
}

/// What the copy must never do: hand the tree of the CFG as it was.
#[test]
fn a_tree_after_an_edit_of_the_cfg_is_built_again_and_follows_it() {
    let mut function = function(
        "define void @f(i1 %c) {\nentry:\n  br i1 %c, label %a, label %b\na:\n  br label %m\nb:\n  br label %m\nm:\n  br label %n\nn:\n  ret void\n}\n",
    );
    let before = DomTree::of(&function);
    let layout = function.layout().to_vec();
    let entry_branch = function.terminator(layout[0]).expect("a branch");
    function.set_operand(entry_branch, 2, Operand::Block(layout[1]));
    let (built, _) = builds();
    let after = DomTree::of(&function);
    assert_eq!(builds().0 - built, 1, "built again");
    assert!(after == DomTree::built(&function), "the tree of the CFG now");
    assert!(before != after, "b is no longer reached from the entry");
}

/// An edit that leaves the blocks and edges alone leaves the tree.
#[test]
fn a_tree_after_an_edit_that_keeps_the_cfg_is_copied() {
    let mut function = function(
        "define i32 @f(i32 %x) {\nentry:\n  br label %a\na:\n  br label %b\nb:\n  br label %c\nc:\n  %y = add i32 %x, 1\n  ret i32 %y\n}\n",
    );
    DomTree::of(&function);
    let layout = function.layout().to_vec();
    let add = function.iter_block(layout[3]).next().expect("the add");
    let operands = function.instruction(add).operands.clone();
    function.set_operand(add, 0, operands[1]);
    function.set_operand(add, 1, operands[0]);
    let (built, reused) = builds();
    let tree = DomTree::of(&function);
    assert_eq!((builds().0 - built, builds().1 - reused), (0, 1));
    tree.verify(&function, VerificationLevel::Full).expect("still a tree of the function");
}

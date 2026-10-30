//! The iterated dominance frontier of every defining set of 8 blocks, on the
//! graph of `tools/llvmport/idf_dump.cpp` and each single-edge flip of it,
//! against the lists the pinned LLVM's calculators give.

use super::*;
use crate::blocklist::BlockList;
use crate::domtree::DomGraph;

/// A graph given by its successor lists.
struct Cfg {
    succs: Vec<Vec<u32>>,
    blocks: Vec<BlockId>,
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
        // Newest use first, as an LLVM block's use list the harness builds in
        // block order.
        (0..self.succs.len() as u32)
            .rev()
            .filter(|from| self.succs[*from as usize].contains(&block.0))
            .map(BlockId)
            .collect()
    }
}

const BASE: [&[u32]; 8] = [&[1, 2], &[3], &[3, 4], &[1, 5], &[3, 6], &[6], &[5], &[2]];

/// `BASE` with the edge `key` ("base", or "3+5" / "3-1": from, flip, to)
/// flipped.
fn graph(key: &str) -> Cfg {
    let mut succs: Vec<Vec<u32>> = BASE.iter().map(|one| one.to_vec()).collect();
    if key != "base" {
        let at = key.find(['+', '-']).expect("a flip");
        let (from, to) = (key[..at].parse::<usize>().expect("a block"), key[at + 1..].parse::<u32>().expect("a block"));
        if &key[at..=at] == "+" {
            succs[from].push(to);
        } else {
            succs[from].retain(|one| *one != to);
        }
    }
    Cfg { blocks: (0..8).map(BlockId).collect(), succs }
}

fn set(mask: u32) -> IdSet<BlockId> {
    (0..8).filter(|b| mask >> b & 1 == 1).map(BlockId).collect()
}

/// The lists for every defining mask of one direction and live-in mask, as
/// the dump writes them: `a,b,;` each.
fn lists(
    cfg: &Cfg,
    post: bool,
    live: u32,
) -> Vec<String> {
    let live_in = set(live);
    let live_in = (live != 0).then_some(&live_in);
    (0..256)
        .map(|mask| {
            let defining = set(mask);
            let found = if post {
                let mut tree = PostDomTree::new();
                tree.recalculate(cfg);
                reverse(&tree, cfg, &defining, live_in)
            } else {
                let mut tree = DomTree::new();
                tree.recalculate(cfg);
                forward(&tree, cfg, &defining, live_in)
            };
            found.iter().map(|b| format!("{},", b.0)).collect()
        })
        .collect()
}

/// FNV-1a 64, as the dump's `mix`.
fn hash(text: &str) -> String {
    let mut h = 0xcbf29ce484222325_u64;
    for byte in text.bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// The instrument's premise: the golden file is the pinned LLVM's, and its
/// lists are neither all empty nor all alike.
#[test]
fn the_golden_file_is_the_pinned_llvms_and_has_frontiers_to_find() {
    let text = include_str!("idf_golden.txt");
    assert_eq!(text.lines().next(), Some("# LLVM 24.0.0git d1106deb71cc81016406a1917d0508d2df296266"));
    let lists: std::collections::BTreeSet<&str> =
        text.lines().filter(|l| l.starts_with("list")).filter_map(|l| l.split(" : ").nth(1)).collect();
    assert!(lists.len() > 20 && lists.iter().any(|l| !l.is_empty()), "{lists:?}");
    assert_eq!(text.lines().filter(|l| l.starts_with("hash")).count(), 2 * 65 * 6);
}

/// Every list of the base graph, forward and reverse, with and without a
/// live-in set, in LLVM's order.
#[test]
fn the_frontier_of_every_defining_set_is_the_list_llvm_gives() {
    let text = include_str!("idf_golden.txt");
    let cfg = graph("base");
    let mut compared = 0;
    for (dir, post) in [("fwd", false), ("rev", true)] {
        for (name, live) in [("-", 0), ("live", 0x55)] {
            let got = lists(&cfg, post, live);
            for line in
                text.lines().filter(|l| l.starts_with(&format!("list {dir} ")) && l.contains(&format!(" {name} :")))
            {
                let mask = usize::from_str_radix(&line.split(' ').nth(2).expect("a mask"), 16).expect("hex");
                let want = line.split(" : ").nth(1).expect("a list");
                assert_eq!(got[mask], want, "{line}");
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 4 * 256);
}

/// Every graph, direction and live-in mode against LLVM's hash of the 256
/// lists.
#[test]
fn the_frontiers_of_every_flipped_graph_are_the_lists_llvm_gives() {
    let text = include_str!("idf_golden.txt");
    let mut seen = 0;
    for line in text.lines().filter(|l| l.starts_with("hash")) {
        let f: Vec<&str> = line.split(' ').collect();
        let (post, key, live) = (f[1] == "rev", f[2], u32::from_str_radix(f[3], 16).expect("hex"));
        let all: String = lists(&graph(key), post, live).iter().map(|l| format!("{l};")).collect();
        assert_eq!(hash(&all), f[4], "{line}");
        seen += 1;
    }
    assert_eq!(seen, 2 * 65 * 6);
}

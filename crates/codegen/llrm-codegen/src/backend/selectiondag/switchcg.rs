//! SwitchCG: how a `switch` is split into clusters before any code is made
//! (pin `SwitchLoweringUtils.cpp` and the planning half of `visitSwitch`).
//!
//! # How it works
//! The cases become single-value clusters (`range`), sorted and merged where
//! adjacent values share a destination (`sortAndRangeify`). A cluster that
//! carries two thirds of the probability is peeled off first
//! (`peelDominantCaseCluster`). `find_jump_tables` then covers the clusters
//! with jump tables where the range is dense enough (`isSuitableForJumpTable`,
//! at least 4 entries), by the pin's dynamic program that minimizes the number
//! of partitions; `find_bit_test_clusters` does the same for runs that fit a
//! machine word and go to at most three destinations (`buildBitTests`).
//! Probabilities are the pin's 31-bit fractions ([`BranchProbability`]).
//!
//! # Runs
//! Called by the builder's `switch` at -O1 and above, with the IR switch's
//! edge probabilities from `llrm_analysis::bpi`.
//!
//! # See also
//! LLVM's `findJumpTables`
//! ([SwitchLoweringUtils.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SwitchLoweringUtils.cpp#L47)),
//! `findBitTestClusters`
//! ([L222](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SwitchLoweringUtils.cpp#L222)),
//! `peelDominantCaseCluster`
//! ([SelectionDAGBuilder.cpp:12823](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L12823)).

use std::collections::BTreeMap;

/// `BranchProbability`: a fraction of `2^31`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BranchProbability(pub u32);

const D: u32 = 1 << 31;

impl BranchProbability {
    pub const ZERO: BranchProbability = BranchProbability(0);

    /// `BranchProbability(Numerator, Denominator)`, rounded to nearest.
    pub fn new(
        numerator: u32,
        denominator: u32,
    ) -> BranchProbability {
        assert!(denominator > 0 && numerator <= denominator);
        if denominator == D {
            return BranchProbability(numerator);
        }
        BranchProbability(
            ((u64::from(numerator) * u64::from(D) + u64::from(denominator / 2)) / u64::from(denominator)) as u32,
        )
    }

    pub fn compl(self) -> BranchProbability {
        BranchProbability(D - self.0)
    }

    pub fn is_one(self) -> bool {
        self.0 == D
    }

    /// `operator/(uint32_t)`.
    pub fn div(
        self,
        by: u32,
    ) -> BranchProbability {
        BranchProbability(self.0 / by)
    }

    /// `scale(Num)`: `Num * self`, rounded down.
    pub fn scale(
        self,
        num: u64,
    ) -> u64 {
        (u128::from(num) * u128::from(self.0) / u128::from(D)) as u64
    }

    pub fn saturating_add(
        self,
        other: BranchProbability,
    ) -> BranchProbability {
        BranchProbability(self.0.saturating_add(other.0).min(D))
    }

    pub fn saturating_sub(
        self,
        other: BranchProbability,
    ) -> BranchProbability {
        BranchProbability(self.0.saturating_sub(other.0))
    }
}

/// What a cluster is lowered as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterKind {
    Range,
    /// Index into [`SwitchPlan::jump_tables`].
    JumpTable(usize),
    /// Index into [`SwitchPlan::bit_tests`].
    BitTests(usize),
}

/// `CaseCluster`: the case values `low..=high` (signed at the condition's
/// width) and where they go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseCluster {
    pub kind: ClusterKind,
    pub low: i128,
    pub high: i128,
    pub dest: u32,
    pub prob: BranchProbability,
}

/// `JumpTableHeader` and `JumpTable`: the table's entries, one per value
/// from `low` to `high`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JumpTableCase {
    pub low: i128,
    pub high: i128,
    pub table: Vec<u32>,
    /// Each destination of the table and the probability of its cases
    /// (`JTProbs`), in first-seen order.
    pub dests: Vec<(u32, BranchProbability)>,
}

/// `BitTestCase`: one destination of a bit-test cluster and the mask of
/// the values (from `low_bound`) that go there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitTestCase {
    pub mask: u64,
    pub dest: u32,
    pub prob: BranchProbability,
}

/// `BitTestBlock`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitTestBlock {
    pub low_bound: i128,
    pub range: i128,
    pub contiguous: bool,
    pub cases: Vec<BitTestCase>,
    pub prob: BranchProbability,
}

/// The target facts the planner asks.
#[derive(Clone, Copy, Debug)]
pub struct SwitchTarget {
    /// The pointer width: the bit-test word and the index size.
    pub pointer_bits: u32,
    /// `getMinimumJumpTableEntries`.
    pub min_jump_table_entries: usize,
    /// `areJTsAllowed`.
    pub jump_tables: bool,
    /// `shouldOptimizeForSize`.
    pub opt_size: bool,
    /// `hasMinSize`: no peeling, no balanced tree.
    pub min_size: bool,
    /// Not -O0.
    pub optimize: bool,
}

impl SwitchTarget {
    /// x86 at -O2 with the pin's defaults.
    pub fn x86(pointer_bits: u32) -> SwitchTarget {
        SwitchTarget {
            pointer_bits,
            min_jump_table_entries: 4,
            jump_tables: true,
            opt_size: false,
            min_size: false,
            optimize: true,
        }
    }
}

/// The clusters of one switch after peeling and the jump-table and bit-test
/// passes, and what those clusters refer to.
#[derive(Clone, Debug, Default)]
pub struct SwitchPlan {
    pub clusters: Vec<CaseCluster>,
    pub jump_tables: Vec<JumpTableCase>,
    pub bit_tests: Vec<BitTestBlock>,
    /// The cluster peeled off first and its probability.
    pub peeled: Option<(CaseCluster, BranchProbability)>,
}

/// The pin's `Case clusters:` line.
pub fn describe(clusters: &[CaseCluster]) -> String {
    let mut text = String::new();
    for cluster in clusters {
        match cluster.kind {
            ClusterKind::JumpTable(_) => text.push_str("JT:"),
            ClusterKind::BitTests(_) => text.push_str("BT:"),
            ClusterKind::Range => {}
        }
        text.push_str(&cluster.low.to_string());
        if cluster.low != cluster.high {
            text.push_str(&format!("-{}", cluster.high));
        }
        text.push(' ');
    }
    text
}

/// `visitSwitch` up to the work list: the cases `(value, destination)` of a
/// switch whose default goes to `default`.
pub fn plan(
    cases: &[(i128, u32)],
    default: u32,
    target: &SwitchTarget,
) -> SwitchPlan {
    let prob = BranchProbability::new(1, cases.len() as u32 + 1);
    let with: Vec<(i128, u32, BranchProbability)> = cases.iter().map(|&(value, dest)| (value, dest, prob)).collect();
    plan_with(&with, default, target)
}

/// [`plan`] with each case's own probability (the IR switch's edge).
pub fn plan_with(
    cases: &[(i128, u32, BranchProbability)],
    default: u32,
    target: &SwitchTarget,
) -> SwitchPlan {
    let mut clusters: Vec<CaseCluster> = cases
        .iter()
        .map(|&(value, dest, prob)| CaseCluster { kind: ClusterKind::Range, low: value, high: value, dest, prob })
        .collect();
    sort_and_rangeify(&mut clusters);
    let mut plan = SwitchPlan::default();
    plan.peeled = peel_dominant_case_cluster(&mut clusters, target);
    if clusters.is_empty() {
        return plan;
    }
    find_jump_tables(&mut clusters, default, target, &mut plan.jump_tables);
    find_bit_test_clusters(&mut clusters, target, &mut plan.bit_tests);
    plan.clusters = clusters;
    plan
}

/// `sortAndRangeify`.
pub fn sort_and_rangeify(clusters: &mut Vec<CaseCluster>) {
    clusters.sort_by_key(|cluster| cluster.low);
    let mut merged: Vec<CaseCluster> = Vec::with_capacity(clusters.len());
    for cluster in clusters.drain(..) {
        match merged.last_mut() {
            Some(last) if last.dest == cluster.dest && cluster.low - last.high == 1 => {
                last.high = cluster.low;
                last.prob = last.prob.saturating_add(cluster.prob);
            }
            _ => merged.push(cluster),
        }
    }
    *clusters = merged;
}

const SWITCH_PEEL_THRESHOLD: u32 = 66;

/// `scaleCaseProbality`.
pub fn scale_case_probability(
    case: BranchProbability,
    peeled: BranchProbability,
) -> BranchProbability {
    if peeled.is_one() {
        return BranchProbability::ZERO;
    }
    let switch = peeled.compl();
    let numerator = case.0;
    let denominator = switch.scale(u64::from(D)) as u32;
    BranchProbability::new(numerator, numerator.max(denominator))
}

/// `peelDominantCaseCluster`: the cluster with the top probability at or
/// above 66%, taken out of `clusters`; the others are scaled to what is left.
fn peel_dominant_case_cluster(
    clusters: &mut Vec<CaseCluster>,
    target: &SwitchTarget,
) -> Option<(CaseCluster, BranchProbability)> {
    if clusters.len() < 2 || !target.optimize || target.min_size {
        return None;
    }
    let mut top = BranchProbability::new(SWITCH_PEEL_THRESHOLD, 100);
    let mut peeled_at = None;
    for (index, cluster) in clusters.iter().enumerate() {
        if cluster.prob < top {
            continue;
        }
        top = cluster.prob;
        peeled_at = Some(index);
    }
    let peeled = clusters.remove(peeled_at?);
    for cluster in clusters.iter_mut() {
        cluster.prob = scale_case_probability(cluster.prob, top);
    }
    Some((peeled, top))
}

fn limited(value: i128) -> u64 {
    u64::try_from(value.max(0)).unwrap_or(u64::MAX)
}

/// `getJumpTableRange`.
fn jump_table_range(
    clusters: &[CaseCluster],
    first: usize,
    last: usize,
) -> u64 {
    let span = limited(clusters[last].high - clusters[first].low).min((u64::MAX - 1) / 100);
    span + 1
}

/// `isSuitableForJumpTable`.
fn suitable_for_jump_table(
    num_cases: u64,
    range: u64,
    target: &SwitchTarget,
) -> bool {
    let min_density = if target.opt_size { 40 } else { 10 };
    num_cases * 100 >= range * min_density
}

/// `isSuitableForBitTests`.
fn suitable_for_bit_tests(
    dests: &BTreeMap<u32, u32>,
    low: i128,
    high: i128,
    target: &SwitchTarget,
) -> bool {
    if !range_fits_in_word(low, high, target) {
        return false;
    }
    let num_dests = dests.len();
    let num_cmps: u32 = dests.values().sum();
    let max_entry = dests.values().copied().max().unwrap_or(0);
    if max_entry < 2 {
        return false;
    }
    (num_dests == 1 && num_cmps >= 3) || (num_dests == 2 && num_cmps >= 5) || (num_dests == 3 && num_cmps >= 6)
}

/// `rangeFitsInWord`.
fn range_fits_in_word(
    low: i128,
    high: i128,
    target: &SwitchTarget,
) -> bool {
    let range = limited(high - low).min(u64::MAX - 1) + 1;
    range <= u64::from(target.pointer_bits)
}

/// `findJumpTables`.
pub fn find_jump_tables(
    clusters: &mut Vec<CaseCluster>,
    default: u32,
    target: &SwitchTarget,
    tables: &mut Vec<JumpTableCase>,
) {
    if !target.jump_tables {
        return;
    }
    let min_entries = target.min_jump_table_entries;
    let small = min_entries / 2;
    let n = clusters.len();
    if n < 2 || n < min_entries {
        return;
    }
    let mut total = vec![0u64; n];
    for i in 0..n {
        total[i] = limited(clusters[i].high - clusters[i].low) + 1 + if i == 0 { 0 } else { total[i - 1] };
    }
    let cases = |first: usize, last: usize| total[last] - if first == 0 { 0 } else { total[first - 1] };
    let range = jump_table_range(clusters, 0, n - 1);
    if suitable_for_jump_table(cases(0, n - 1), range, target) {
        if let Some(cluster) = build_jump_table(clusters, 0, n - 1, default, target, tables) {
            *clusters = vec![cluster];
            return;
        }
    }
    if !target.optimize {
        return;
    }
    const NO_TABLE: u32 = 0;
    let _ = NO_TABLE;
    const TABLE: u32 = 1;
    const FEW_CASES: u32 = 1;
    const SINGLE_CASE: u32 = 2;
    let mut min_partitions = vec![0usize; n];
    let mut last_element = vec![0usize; n];
    let mut score = vec![0u32; n];
    min_partitions[n - 1] = 1;
    last_element[n - 1] = n - 1;
    score[n - 1] = SINGLE_CASE;
    for i in (0..n - 1).rev() {
        min_partitions[i] = min_partitions[i + 1] + 1;
        last_element[i] = i;
        score[i] = score[i + 1] + SINGLE_CASE;
        for j in ((i + 1)..n).rev() {
            let range = jump_table_range(clusters, i, j);
            if !suitable_for_jump_table(cases(i, j), range, target) {
                continue;
            }
            let partitions = 1 + if j == n - 1 { 0 } else { min_partitions[j + 1] };
            let mut s = if j == n - 1 { 0 } else { score[j + 1] };
            let entries = j - i + 1;
            if entries == 1 {
                s += SINGLE_CASE;
            } else if entries <= small {
                s += FEW_CASES;
            } else if entries >= min_entries {
                s += TABLE;
            }
            if partitions < min_partitions[i] || (partitions == min_partitions[i] && s > score[i]) {
                min_partitions[i] = partitions;
                last_element[i] = j;
                score[i] = s;
            }
        }
    }
    let mut out = Vec::new();
    let mut first = 0;
    while first < n {
        let last = last_element[first];
        let built = if last - first + 1 >= min_entries {
            build_jump_table(clusters, first, last, default, target, tables)
        } else {
            None
        };
        match built {
            Some(cluster) => out.push(cluster),
            None => out.extend_from_slice(&clusters[first..=last]),
        }
        first = last + 1;
    }
    *clusters = out;
}

/// `buildJumpTable`: `None` where the run suits bit tests better.
fn build_jump_table(
    clusters: &[CaseCluster],
    first: usize,
    last: usize,
    default: u32,
    target: &SwitchTarget,
    tables: &mut Vec<JumpTableCase>,
) -> Option<CaseCluster> {
    let mut prob = BranchProbability::ZERO;
    let mut table = Vec::new();
    let mut dests: BTreeMap<u32, u32> = BTreeMap::new();
    let mut dest_probs: Vec<(u32, BranchProbability)> = Vec::new();
    for i in first..=last {
        let cluster = &clusters[i];
        prob = prob.saturating_add(cluster.prob);
        match dest_probs.iter_mut().find(|one| one.0 == cluster.dest) {
            Some(one) => one.1 = one.1.saturating_add(cluster.prob),
            None => dest_probs.push((cluster.dest, cluster.prob)),
        }
        *dests.entry(cluster.dest).or_default() += if cluster.low == cluster.high { 1 } else { 2 };
        if i != first {
            let gap = limited(cluster.low - clusters[i - 1].high) - 1;
            table.extend(std::iter::repeat_n(default, gap as usize));
        }
        let size = limited(cluster.high - cluster.low) + 1;
        table.extend(std::iter::repeat_n(cluster.dest, size as usize));
    }
    if suitable_for_bit_tests(&dests, clusters[first].low, clusters[last].high, target) {
        return None;
    }
    tables.push(JumpTableCase { low: clusters[first].low, high: clusters[last].high, table, dests: dest_probs });
    Some(CaseCluster {
        kind: ClusterKind::JumpTable(tables.len() - 1),
        low: clusters[first].low,
        high: clusters[last].high,
        dest: 0,
        prob,
    })
}

/// `findBitTestClusters`.
pub fn find_bit_test_clusters(
    clusters: &mut Vec<CaseCluster>,
    target: &SwitchTarget,
    tests: &mut Vec<BitTestBlock>,
) {
    if !target.optimize {
        return;
    }
    let bit_width = target.pointer_bits as usize;
    let n = clusters.len();
    let mut min_partitions = vec![0usize; n];
    let mut last_element = vec![0usize; n];
    min_partitions[n - 1] = 1;
    last_element[n - 1] = n - 1;
    for i in (0..n - 1).rev() {
        min_partitions[i] = min_partitions[i + 1] + 1;
        last_element[i] = i;
        for j in ((i + 1)..=(n - 1).min(i + bit_width - 1)).rev() {
            if !range_fits_in_word(clusters[i].low, clusters[j].high, target) {
                continue;
            }
            let mut ranges_only = true;
            let mut dests = std::collections::BTreeSet::new();
            for cluster in &clusters[i..=j] {
                if cluster.kind != ClusterKind::Range {
                    ranges_only = false;
                    break;
                }
                dests.insert(cluster.dest);
            }
            if !ranges_only || dests.len() > 3 {
                break;
            }
            let partitions = 1 + if j == n - 1 { 0 } else { min_partitions[j + 1] };
            if partitions < min_partitions[i] {
                min_partitions[i] = partitions;
                last_element[i] = j;
            }
        }
    }
    let mut out = Vec::new();
    let mut first = 0;
    while first < n {
        let last = last_element[first];
        match build_bit_tests(clusters, first, last, target, tests) {
            Some(cluster) => out.push(cluster),
            None => out.extend_from_slice(&clusters[first..=last]),
        }
        first = last + 1;
    }
    *clusters = out;
}

/// `buildBitTests`.
fn build_bit_tests(
    clusters: &[CaseCluster],
    first: usize,
    last: usize,
    target: &SwitchTarget,
    tests: &mut Vec<BitTestBlock>,
) -> Option<CaseCluster> {
    if first == last {
        return None;
    }
    let mut dests: BTreeMap<u32, u32> = BTreeMap::new();
    for cluster in &clusters[first..=last] {
        *dests.entry(cluster.dest).or_default() += if cluster.low == cluster.high { 1 } else { 2 };
    }
    let (low, high) = (clusters[first].low, clusters[last].high);
    if !suitable_for_bit_tests(&dests, low, high, target) {
        return None;
    }
    let bit_width = i128::from(target.pointer_bits);
    let mut contiguous =
        clusters[first + 1..=last].iter().zip(&clusters[first..]).all(|(next, previous)| next.low == previous.high + 1);
    let (low_bound, range);
    if low > 0 && high < bit_width {
        low_bound = 0;
        range = high;
        contiguous = false;
    } else {
        low_bound = low;
        range = high - low;
    }
    struct CaseBits {
        mask: u64,
        dest: u32,
        bits: u32,
        extra: BranchProbability,
    }
    let mut cbv: Vec<CaseBits> = Vec::new();
    let mut total = BranchProbability::ZERO;
    for cluster in &clusters[first..=last] {
        let at = match cbv.iter().position(|bits| bits.dest == cluster.dest) {
            Some(at) => at,
            None => {
                cbv.push(CaseBits { mask: 0, dest: cluster.dest, bits: 0, extra: BranchProbability::ZERO });
                cbv.len() - 1
            }
        };
        let lo = (cluster.low - low_bound) as u64;
        let hi = (cluster.high - low_bound) as u64;
        cbv[at].mask |= (u64::MAX >> (63 - (hi - lo))) << lo;
        cbv[at].bits += (hi - lo + 1) as u32;
        cbv[at].extra = cbv[at].extra.saturating_add(cluster.prob);
        total = total.saturating_add(cluster.prob);
    }
    cbv.sort_by(|a, b| b.extra.cmp(&a.extra).then(b.bits.cmp(&a.bits)).then(a.mask.cmp(&b.mask)));
    let cases = cbv.iter().map(|bits| BitTestCase { mask: bits.mask, dest: bits.dest, prob: bits.extra }).collect();
    tests.push(BitTestBlock { low_bound, range, contiguous, cases, prob: total });
    Some(CaseCluster { kind: ClusterKind::BitTests(tests.len() - 1), low, high, dest: 0, prob: total })
}

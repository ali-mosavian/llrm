//! LLVM's switch clusters: which cases are one range, a jump table or a bit
//! test, and where the tree of what is left splits.
//!
//! A translation of `SwitchLoweringUtils.cpp` at LLVM `d1106deb71cc`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SwitchLoweringUtils.cpp)),
//! as pure decisions over a cluster vector: nothing here edits a function.
//! The emission is a separate step that reads the clusters.
//!
//! # How it works
//! `sort_and_rangeify` makes one cluster of the neighbouring cases that share a
//! target. `peel_dominant_case_cluster` takes out a cluster that holds two
//! cases in three (`peelDominantCaseCluster`). `find_jump_tables` partitions
//! the clusters into the fewest dense runs (Kannan and Proebsting), a run of at
//! least `min_jump_table_entries` becoming a table unless bit tests suit it
//! better. `find_bit_test_clusters` does the same for runs that fit a word with
//! at most three targets. `split_work_item` picks the pivot of the balanced
//! tree over what is left.
//!
//! Values are signed, as LLVM's `slt`: a case is its value sign-extended from
//! the switch's width.
//!
//! # Example
//! Cases 0..=5 alternating between targets `a`, `b` and `c` are six
//! clusters; the whole range is suitable for a table, but three targets with
//! six comparisons suit bit tests (`isSuitableForBitTests`), so the table is
//! refused and the run is one bit-test cluster.
//!
//! # Deviations
//! - DEVIATE S1: a case's `BranchProbability` is `1 / (cases + 1)`, what
//!   `BranchProbabilityInfo` gives each edge of a switch with no weights and no
//!   unreachable target; MIR carries no edge probabilities.
//! - DEVIATE S2: `buildJumpTable` asks `isSuitableForBitTests` before it fills
//!   the table, not after. The answer does not read the table.
//! - DEVIATE S3: the jump table's successor probabilities (`JTProbs`,
//!   `normalizeSuccProbs`) are not kept; MIR has no successor probabilities.
//!   The cluster keeps its summed probability.
//! - `findJumpTables` is quadratic in the clusters, as the pin is. It is exact,
//!   so the decisions match; the switch it runs behind is off.

use llrm_mir::module::BlockId;

/// `BranchProbability`: a numerator over 2^31, saturating.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Probability(u32);

impl Probability {
    const DENOMINATOR: u32 = 1 << 31;

    pub const ZERO: Probability = Probability(0);

    /// `BranchProbability(numerator, denominator)`.
    pub fn new(
        numerator: u32,
        denominator: u32,
    ) -> Probability {
        assert!(denominator > 0 && numerator <= denominator);
        if denominator == Self::DENOMINATOR {
            return Probability(numerator);
        }
        let scaled = (numerator as u64 * Self::DENOMINATOR as u64 + denominator as u64 / 2) / denominator as u64;
        Probability(scaled as u32)
    }

    /// `getCompl`.
    fn complement(self) -> Probability {
        Probability(Self::DENOMINATOR - self.0)
    }

    pub fn plus(
        self,
        other: Probability,
    ) -> Probability {
        Probability(self.0.saturating_add(other.0).min(Self::DENOMINATOR))
    }

    /// `operator/(uint32_t)`: the numerator divided, truncating.
    pub fn divided(
        self,
        by: u32,
    ) -> Probability {
        Probability(self.0 / by)
    }
}

/// What a cluster is lowered as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `CC_Range`: `low..=high` to `target`.
    Range,
    /// `CC_JumpTable`: the target of each value from `low`, the default's
    /// block in the holes.
    JumpTable { table: Vec<BlockId> },
    /// `CC_BitTests`: the bit tests of `low..=high`.
    BitTests(BitTests),
}

/// One run of bit tests (`BitTestBlock` and its `BitTestInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitTests {
    /// The value the bit index is taken from: zero where the cases are all
    /// below the word, else the lowest case.
    pub low_bound: i128,
    /// The largest bit index.
    pub range: i128,
    /// No value in the range goes to the default.
    pub contiguous: bool,
    /// A mask and a target each, the likeliest first.
    pub cases: Vec<BitTestCase>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitTestCase {
    pub mask: u64,
    pub target: BlockId,
    pub probability: Probability,
}

/// `CaseCluster`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseCluster {
    pub low: i128,
    pub high: i128,
    pub target: BlockId,
    pub probability: Probability,
    pub kind: Kind,
}

impl CaseCluster {
    /// `CaseCluster::range`.
    pub fn range(
        low: i128,
        high: i128,
        target: BlockId,
        probability: Probability,
    ) -> CaseCluster {
        CaseCluster { low, high, target, probability, kind: Kind::Range }
    }

    pub fn is_range(&self) -> bool {
        self.kind == Kind::Range
    }
}

/// What `findJumpTables` and `findBitTestClusters` ask of the target and the
/// function: `TargetLowering`'s hooks and the size of the code.
#[derive(Clone, Copy, Debug)]
pub struct SwitchTarget {
    /// `areJTsAllowed`.
    pub jump_tables: bool,
    /// `getMinimumJumpTableEntries`; 4 on x86.
    pub min_jump_table_entries: u32,
    /// `getMinimumJumpTableDensity(false)` and `(true)`: percent.
    pub density: u32,
    pub size_density: u32,
    /// `getMaximumJumpTableSize`.
    pub max_jump_table_size: u64,
    /// `getMinimumBitTestCmps`.
    pub min_bit_test_cmps: u32,
    /// The pointer's width: `rangeFitsInWord` and the bit tests' word.
    pub word_bits: u32,
    /// `isOperationLegal(SHL, pointer)`.
    pub shift_legal: bool,
    /// `TM->getOptLevel() != None`.
    pub optimizing: bool,
    /// `shouldOptimizeForSize` of the switch's block.
    pub optimize_for_size: bool,
    /// The function has `minsize`.
    pub minimize_size: bool,
}

impl SwitchTarget {
    /// The defaults of `TargetLoweringBase` and X86 at -O2, for a word of
    /// `word_bits`.
    pub fn x86(word_bits: u32) -> SwitchTarget {
        SwitchTarget {
            jump_tables: true,
            min_jump_table_entries: 4,
            density: 10,
            size_density: 40,
            max_jump_table_size: u32::MAX as u64,
            min_bit_test_cmps: 2,
            word_bits,
            shift_legal: true,
            optimizing: true,
            optimize_for_size: false,
            minimize_size: false,
        }
    }

    /// `rangeFitsInWord`.
    fn range_fits_in_word(
        &self,
        low: i128,
        high: i128,
    ) -> bool {
        limited(high - low, u64::MAX - 1) < self.word_bits as u64
    }

    /// `isSuitableForJumpTable`.
    fn suitable_for_jump_table(
        &self,
        cases: u64,
        range: u64,
    ) -> bool {
        let density = if self.optimize_for_size { self.size_density } else { self.density };
        (self.optimize_for_size || range <= self.max_jump_table_size) && cases * 100 >= range * density as u64
    }

    /// `isSuitableForBitTests`.
    fn suitable_for_bit_tests(
        &self,
        destinations: &Destinations,
        low: i128,
        high: i128,
    ) -> bool {
        if !self.range_fits_in_word(low, high) {
            return false;
        }
        let targets = destinations.0.len();
        let comparisons: u32 = destinations.0.iter().map(|&(_, n)| n).sum();
        let most = destinations.0.iter().map(|&(_, n)| n).max().unwrap_or(0);
        if most < self.min_bit_test_cmps {
            return false;
        }
        matches!((targets, comparisons), (1, 3..) | (2, 5..) | (3, 6..))
    }
}

/// `DestMap`: the comparisons each target takes, a handful of targets.
#[derive(Default)]
struct Destinations(Vec<(BlockId, u32)>);

impl Destinations {
    fn add(
        &mut self,
        target: BlockId,
        comparisons: u32,
    ) {
        match self.0.iter_mut().find(|(to, _)| *to == target) {
            Some((_, total)) => *total += comparisons,
            None => self.0.push((target, comparisons)),
        }
    }
}

/// `APInt::getLimitedValue`: the value, or `limit` where it is larger.
fn limited(
    value: i128,
    limit: u64,
) -> u64 {
    u64::try_from(value).map_or(limit, |value| value.min(limit))
}

/// `sortAndRangeify`: the single-case clusters in value order, neighbours that
/// share a target made one.
pub fn sort_and_rangeify(clusters: &mut Vec<CaseCluster>) {
    assert!(clusters.iter().all(|cluster| cluster.low == cluster.high), "input clusters must be single-case");
    clusters.sort_by_key(|cluster| cluster.low);
    let mut merged: Vec<CaseCluster> = Vec::with_capacity(clusters.len());
    for cluster in clusters.drain(..) {
        match merged.last_mut() {
            Some(last) if last.target == cluster.target && cluster.low - last.high == 1 => {
                last.high = cluster.low;
                last.probability = last.probability.plus(cluster.probability);
            }
            _ => merged.push(cluster),
        }
    }
    *clusters = merged;
}

/// `-switch-peel-threshold`: percent.
const PEEL_THRESHOLD: u32 = 66;

/// `scaleCaseProbality`: a probability among what is left once a cluster of
/// `peeled` has been taken out.
pub fn scale_case_probability(
    probability: Probability,
    peeled: Probability,
) -> Probability {
    if peeled.0 == Probability::DENOMINATOR {
        return Probability::ZERO;
    }
    let numerator = probability.0;
    let denominator = peeled.complement().0;
    Probability::new(numerator, numerator.max(denominator))
}

/// `peelDominantCaseCluster`: the likeliest cluster, taken out of `clusters`
/// where its probability is at least 66%, with that probability; the others'
/// are scaled to what is left. Only where the target optimizes for speed.
pub fn peel_dominant_case_cluster(
    clusters: &mut Vec<CaseCluster>,
    target: &SwitchTarget,
) -> Option<(CaseCluster, Probability)> {
    if clusters.len() < 2 || !target.optimizing || target.minimize_size {
        return None;
    }
    let mut top = Probability::new(PEEL_THRESHOLD, 100);
    let mut peeled = None;
    for (index, cluster) in clusters.iter().enumerate() {
        if cluster.probability >= top {
            top = cluster.probability;
            peeled = Some(index);
        }
    }
    let cluster = clusters.remove(peeled?);
    for rest in clusters.iter_mut() {
        rest.probability = scale_case_probability(rest.probability, top);
    }
    Some((cluster, top))
}

/// `getJumpTableRange`: the values from the first cluster's low to the last
/// one's high.
fn jump_table_range(
    clusters: &[CaseCluster],
    first: usize,
    last: usize,
) -> u64 {
    limited(clusters[last].high - clusters[first].low, (u64::MAX - 1) / 100) + 1
}

/// `findJumpTables`.
pub fn find_jump_tables(
    clusters: &mut Vec<CaseCluster>,
    target: &SwitchTarget,
    default: BlockId,
) {
    assert!(!clusters.is_empty() && clusters.iter().all(CaseCluster::is_range));
    assert!(clusters.windows(2).all(|pair| pair[0].high < pair[1].low));
    if !target.jump_tables {
        return;
    }
    let minimum = target.min_jump_table_entries as usize;
    let small = minimum / 2;
    let n = clusters.len();
    if n < 2 || n < minimum {
        return;
    }

    // `total_cases[i]`: the cases of clusters 0..=i.
    let mut total_cases = vec![0u64; n];
    for (i, cluster) in clusters.iter().enumerate() {
        total_cases[i] =
            limited(cluster.high - cluster.low, u64::MAX) + 1 + if i == 0 { 0 } else { total_cases[i - 1] };
    }
    let cases_in = |first: usize, last: usize| total_cases[last] - if first == 0 { 0 } else { total_cases[first - 1] };

    // The cheap case: the whole range is one table.
    if target.suitable_for_jump_table(cases_in(0, n - 1), jump_table_range(clusters, 0, n - 1)) {
        if let Some(table) = build_jump_table(clusters, 0, n - 1, target, default) {
            clusters.clear();
            clusters.push(table);
            return;
        }
    }
    if !target.optimizing {
        return;
    }

    // The partitions with the fewest parts, breaking a tie for the likelier
    // tables (arXiv 1910.02351): scores of a single case, a few cases and a
    // table.
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
        for j in (i + 1..n).rev() {
            if !target.suitable_for_jump_table(cases_in(i, j), jump_table_range(clusters, i, j)) {
                continue;
            }
            let partitions = 1 + if j == n - 1 { 0 } else { min_partitions[j + 1] };
            let mut candidate = if j == n - 1 { 0 } else { score[j + 1] };
            let entries = j - i + 1;
            if entries == 1 {
                candidate += SINGLE_CASE;
            } else if entries <= small {
                candidate += FEW_CASES;
            } else if entries >= minimum {
                candidate += TABLE;
            }
            if partitions < min_partitions[i] || (partitions == min_partitions[i] && candidate > score[i]) {
                min_partitions[i] = partitions;
                last_element[i] = j;
                score[i] = candidate;
            }
        }
    }

    let mut out = Vec::with_capacity(n);
    let mut first = 0;
    while first < n {
        let last = last_element[first];
        let table =
            if last - first + 1 >= minimum { build_jump_table(clusters, first, last, target, default) } else { None };
        match table {
            Some(table) => out.push(table),
            None => out.extend_from_slice(&clusters[first..=last]),
        }
        first = last + 1;
    }
    *clusters = out;
}

/// `buildJumpTable`: the table over `first..=last`, none where bit tests suit
/// the run better.
fn build_jump_table(
    clusters: &[CaseCluster],
    first: usize,
    last: usize,
    target: &SwitchTarget,
    default: BlockId,
) -> Option<CaseCluster> {
    let mut destinations = Destinations::default();
    for cluster in &clusters[first..=last] {
        destinations.add(cluster.target, if cluster.low == cluster.high { 1 } else { 2 });
    }
    // DEVIATE S2: asked before the table is filled.
    if target.suitable_for_bit_tests(&destinations, clusters[first].low, clusters[last].high) {
        return None;
    }
    let mut table = Vec::new();
    let mut probability = Probability::ZERO;
    for (i, cluster) in clusters[first..=last].iter().enumerate() {
        if i != 0 {
            let gap = limited(cluster.low - clusters[first + i - 1].high, u64::MAX) - 1;
            table.extend((0..gap).map(|_| default));
        }
        let size = limited(cluster.high - cluster.low, u64::MAX) + 1;
        table.extend((0..size).map(|_| cluster.target));
        probability = probability.plus(cluster.probability);
    }
    Some(CaseCluster {
        low: clusters[first].low,
        high: clusters[last].high,
        target: default,
        probability,
        kind: Kind::JumpTable { table },
    })
}

/// `findBitTestClusters`.
pub fn find_bit_test_clusters(
    clusters: &mut Vec<CaseCluster>,
    target: &SwitchTarget,
) {
    assert!(!clusters.is_empty());
    assert!(clusters.windows(2).all(|pair| pair[0].high < pair[1].low));
    if !target.optimizing || !target.shift_legal {
        return;
    }
    let width = target.word_bits as usize;
    let n = clusters.len();
    let mut min_partitions = vec![0usize; n];
    let mut last_element = vec![0usize; n];
    min_partitions[n - 1] = 1;
    last_element[n - 1] = n - 1;
    for i in (0..n - 1).rev() {
        min_partitions[i] = min_partitions[i + 1] + 1;
        last_element[i] = i;
        // The search is limited to a word's width.
        for j in (i + 1..=(n - 1).min(i + width - 1)).rev() {
            if !target.range_fits_in_word(clusters[i].low, clusters[j].high) {
                continue;
            }
            let run = &clusters[i..=j];
            let mut targets: Vec<BlockId> = Vec::new();
            let mut ranges_only = true;
            for cluster in run {
                if !cluster.is_range() {
                    ranges_only = false;
                    break;
                }
                if !targets.contains(&cluster.target) {
                    targets.push(cluster.target);
                }
            }
            if !ranges_only || targets.len() > 3 {
                break;
            }
            let partitions = 1 + if j == n - 1 { 0 } else { min_partitions[j + 1] };
            if partitions < min_partitions[i] {
                min_partitions[i] = partitions;
                last_element[i] = j;
            }
        }
    }

    let mut out = Vec::with_capacity(n);
    let mut first = 0;
    while first < n {
        let last = last_element[first];
        match build_bit_tests(clusters, first, last, target) {
            Some(tests) => out.push(tests),
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
) -> Option<CaseCluster> {
    if first == last {
        return None;
    }
    let run = &clusters[first..=last];
    let mut destinations = Destinations::default();
    for cluster in run {
        assert!(cluster.is_range());
        destinations.add(cluster.target, if cluster.low == cluster.high { 1 } else { 2 });
    }
    let (low, high) = (run[0].low, run[run.len() - 1].high);
    assert!(low < high);
    if !target.suitable_for_bit_tests(&destinations, low, high) {
        return None;
    }

    let mut contiguous = run.windows(2).all(|pair| pair[1].low == pair[0].high + 1);
    let (low_bound, range) = if low > 0 && high < target.word_bits as i128 {
        // The cases fit the word as they are: no subtraction of the lowest.
        contiguous = false;
        (0, high)
    } else {
        (low, high - low)
    };

    // One mask per target, in the order the targets first appear.
    struct Bits {
        target: BlockId,
        mask: u64,
        bits: u32,
        probability: Probability,
    }
    let mut masks: Vec<Bits> = Vec::new();
    let mut total = Probability::ZERO;
    for cluster in run {
        let at = match masks.iter().position(|bits| bits.target == cluster.target) {
            Some(at) => at,
            None => {
                masks.push(Bits { target: cluster.target, mask: 0, bits: 0, probability: Probability::ZERO });
                masks.len() - 1
            }
        };
        let (from, to) = ((cluster.low - low_bound) as u64, (cluster.high - low_bound) as u64);
        assert!(to >= from && to < 64, "invalid bit case");
        masks[at].mask |= (u64::MAX >> (63 - (to - from))) << from;
        masks[at].bits += (to - from + 1) as u32;
        masks[at].probability = masks[at].probability.plus(cluster.probability);
        total = total.plus(cluster.probability);
    }
    // The likeliest first, then the most bits, then the smaller mask.
    masks.sort_by(|a, b| b.probability.cmp(&a.probability).then(b.bits.cmp(&a.bits)).then(a.mask.cmp(&b.mask)));
    let cases = masks
        .into_iter()
        .map(|bits| BitTestCase { mask: bits.mask, target: bits.target, probability: bits.probability })
        .collect();
    Some(CaseCluster {
        low,
        high,
        target: run[0].target,
        probability: total,
        kind: Kind::BitTests(BitTests { low_bound, range, contiguous, cases }),
    })
}

/// A part of the tree still to be lowered (`SwitchWorkListItem`), by the
/// indices of its first and last cluster.
#[derive(Clone, Copy, Debug)]
pub struct WorkItem {
    pub first: usize,
    pub last: usize,
    pub default_probability: Probability,
}

/// `SplitWorkItemInfo`: the last cluster to the left and the first to the
/// right of the pivot, and the probability each side carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Split {
    pub last_left: usize,
    pub first_right: usize,
    pub left_probability: Probability,
    pub right_probability: Probability,
}

/// `caseClusterRank`: how many of `clusters[first..=last]` rank before
/// `cluster`: likelier, or the same probability and a lower value.
fn case_cluster_rank(
    clusters: &[CaseCluster],
    cluster: &CaseCluster,
    first: usize,
    last: usize,
) -> usize {
    clusters[first..=last]
        .iter()
        .filter(|other| {
            if other.probability != cluster.probability {
                return other.probability > cluster.probability;
            }
            other.low < cluster.low
        })
        .count()
}

/// `computeSplitWorkItemInfo`.
pub fn split_work_item(
    clusters: &[CaseCluster],
    item: &WorkItem,
) -> Split {
    let mut last_left = item.first;
    let mut first_right = item.last;
    let half = item.default_probability.divided(2);
    let mut left = clusters[last_left].probability.plus(half);
    let mut right = clusters[first_right].probability.plus(half);

    // Move the two ends towards each other, taking the side with the less
    // probability; on a tie, alternate, so zero-probability nodes spread.
    let mut turn = 0u32;
    while last_left + 1 < first_right {
        if left < right || (left == right && turn & 1 == 1) {
            last_left += 1;
            left = left.plus(clusters[last_left].probability);
        } else {
            first_right -= 1;
            right = right.plus(clusters[first_right].probability);
        }
        turn += 1;
    }

    // A leaf holds up to three clusters, which the pivot does not take into
    // account: move one across where that does not demote it.
    loop {
        let on_left = last_left - item.first + 1;
        let on_right = item.last - first_right + 1;
        if on_left.min(on_right) < 3 && on_left.max(on_right) > 3 {
            if on_left < on_right {
                let cluster = &clusters[first_right];
                let right_rank = case_cluster_rank(clusters, cluster, first_right, item.last);
                let left_rank = case_cluster_rank(clusters, cluster, item.first, last_left);
                if left_rank <= right_rank {
                    last_left += 1;
                    first_right += 1;
                    continue;
                }
            } else {
                let cluster = &clusters[last_left];
                let left_rank = case_cluster_rank(clusters, cluster, item.first, last_left);
                let right_rank = case_cluster_rank(clusters, cluster, first_right, item.last);
                if right_rank <= left_rank {
                    last_left -= 1;
                    first_right -= 1;
                    continue;
                }
            }
        }
        break;
    }
    assert!(last_left + 1 == first_right && last_left >= item.first && first_right <= item.last);
    Split { last_left, first_right, left_probability: left, right_probability: right }
}

#[cfg(test)]
#[path = "switchcg_tests.rs"]
mod tests;

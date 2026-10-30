//! The clusters of `switchcg` against the pinned llc (`switchcg_golden.txt`,
//! from `tools/switchcg_oracle.py`) and against clusters worked by hand.

use llrm_mir::module::BlockId;

use super::*;

const DEFAULT: BlockId = BlockId(100);

fn target(name: char) -> BlockId {
    if name == 'z' { DEFAULT } else { BlockId(name as u32 - 'a' as u32) }
}

fn name(block: BlockId) -> char {
    if block == DEFAULT { 'z' } else { (b'a' + block.0 as u8) as char }
}

fn clusters_of(spec: &str) -> Vec<CaseCluster> {
    let pairs: Vec<(i128, char)> = spec
        .split_whitespace()
        .map(|pair| {
            let (value, target) = pair.split_once(':').unwrap();
            (value.parse().unwrap(), target.chars().next().unwrap())
        })
        .collect();
    // DEVIATE S1: `visitSwitch` without branch probabilities.
    let probability = Probability::new(1, pairs.len() as u32 + 1);
    pairs.iter().map(|&(value, to)| CaseCluster::range(value, value, target(to), probability)).collect()
}

fn lowered(spec: &str) -> Vec<CaseCluster> {
    let mut clusters = clusters_of(spec);
    sort_and_rangeify(&mut clusters);
    let target = SwitchTarget::x86(32);
    peel_dominant_case_cluster(&mut clusters, &target);
    if clusters.is_empty() {
        return clusters;
    }
    find_jump_tables(&mut clusters, &target, DEFAULT);
    find_bit_test_clusters(&mut clusters, &target);
    clusters
}

/// llc's tokens: a table's targets, and each mask `visitBitTestCase` tests with
/// `bt` (a single bit, or all but one bit of the range, is a compare; the last
/// target of a contiguous range is not tested).
fn tokens(clusters: &[CaseCluster]) -> Vec<String> {
    let mut tokens = Vec::new();
    for cluster in clusters {
        match &cluster.kind {
            Kind::Range => {}
            Kind::JumpTable { table } => tokens.push(format!(
                "JT:{}",
                table.iter().map(|&block| name(block).to_string()).collect::<Vec<_>>().join(",")
            )),
            Kind::BitTests(tests) => {
                let tested = tests.cases.len() - usize::from(tests.contiguous);
                for case in &tests.cases[..tested] {
                    let ones = case.mask.count_ones() as i128;
                    if ones != 1 && ones != tests.range {
                        tokens.push(format!("BT:{}", case.mask));
                    }
                }
            }
        }
    }
    tokens.sort();
    tokens
}

/// Every switch of the golden file lowers as the pinned llc lowers it: a
/// table where llc made one with the same entries, the same bit-test masks.
#[test]
fn clusters_match_the_pinned_llc() {
    let golden = include_str!("switchcg_golden.txt");
    assert!(golden.starts_with("# llc 24.0.0git at d1106deb71cc"), "the golden file is not from the pin");
    let mut seen = 0;
    for line in golden.lines().filter(|line| !line.starts_with('#')) {
        let (spec, expected) = line.split_once(" => ").unwrap();
        let expected: Vec<&str> = expected.split_whitespace().filter(|token| *token != "-").collect();
        assert_eq!(tokens(&lowered(spec)), expected, "{spec}");
        seen += 1;
    }
    assert!(seen >= 20, "the golden file lists {seen} switches");
}

fn single(
    value: i128,
    to: char,
) -> CaseCluster {
    CaseCluster::range(value, value, target(to), Probability::new(1, 8))
}

/// Neighbours to one target are one range; a gap or another target ends it.
#[test]
fn rangeify_merges_neighbours_with_one_target() {
    let mut clusters = vec![single(3, 'a'), single(1, 'a'), single(2, 'a'), single(4, 'b'), single(6, 'b')];
    sort_and_rangeify(&mut clusters);
    let spans: Vec<(i128, i128, char)> = clusters.iter().map(|c| (c.low, c.high, name(c.target))).collect();
    assert_eq!(spans, [(1, 3, 'a'), (4, 4, 'b'), (6, 6, 'b')]);
    assert_eq!(clusters[0].probability, Probability::new(3, 8));
}

/// Four targets over 0..=11 are a table of 12 entries; three would be bit
/// tests.
#[test]
fn four_targets_make_a_jump_table() {
    let clusters = lowered("0:a 1:b 2:c 3:d 4:a 5:b 6:c 7:d 8:a 9:b 10:c 11:d");
    assert_eq!(clusters.len(), 1);
    assert!(matches!(&clusters[0].kind, Kind::JumpTable { table } if table.len() == 12));
}

/// A hole of the table holds the default.
#[test]
fn holes_hold_the_default() {
    let clusters = lowered("5:a 6:a 7:b 9:b 10:b 11:c 12:c 15:d 16:d");
    let Kind::JumpTable { table } = &clusters[0].kind else { panic!("{clusters:?}") };
    let names: String = table.iter().map(|&block| name(block)).collect();
    assert_eq!(names, "aabzbbcczzdd");
}

/// Two targets over eight values are one bit test of `0b01010101`; the other
/// target is the contiguous range's fall-through.
#[test]
fn two_targets_make_one_bit_test() {
    let clusters = lowered("0:a 1:b 2:a 3:b 4:a 5:b 6:a 7:b");
    let Kind::BitTests(tests) = &clusters[0].kind else { panic!("{clusters:?}") };
    assert_eq!((tests.low_bound, tests.range, tests.contiguous), (0, 7, true));
    assert_eq!(
        tests.cases.iter().map(|case| (case.mask, name(case.target))).collect::<Vec<_>>(),
        [(85, 'a'), (170, 'b')]
    );
}

/// Sparse cases are neither: five values 100 apart stay five ranges.
#[test]
fn sparse_cases_stay_ranges() {
    let clusters = lowered("0:a 100:b 200:c 300:d 400:e");
    assert_eq!(clusters.len(), 5);
    assert!(clusters.iter().all(CaseCluster::is_range));
}

/// With jump tables off nothing is a table.
#[test]
fn no_jump_tables_when_disallowed() {
    let mut clusters = clusters_of("0:a 1:b 2:c 3:d 4:a 5:b 6:c 7:d");
    sort_and_rangeify(&mut clusters);
    let target = SwitchTarget { jump_tables: false, ..SwitchTarget::x86(32) };
    find_jump_tables(&mut clusters, &target, DEFAULT);
    assert!(clusters.iter().all(CaseCluster::is_range));
}

fn split_of(probabilities: &[Probability]) -> (usize, usize) {
    let clusters: Vec<CaseCluster> = probabilities
        .iter()
        .enumerate()
        .map(|(i, &probability)| CaseCluster::range(10 * i as i128, 10 * i as i128, target('a'), probability))
        .collect();
    let item = WorkItem { first: 0, last: clusters.len() - 1, default_probability: Probability::ZERO };
    let split = split_work_item(&clusters, &item);
    (split.last_left, split.first_right)
}

/// Equally likely clusters split in the middle, the ends alternating.
#[test]
fn split_balances_equal_clusters() {
    let equal = |n: usize| vec![Probability::new(1, n as u32); n];
    assert_eq!(split_of(&equal(7)), (2, 3));
    assert_eq!(split_of(&equal(10)), (4, 5));
}

/// A leaf holds three: zero, zero, then four of a quarter split at the
/// probability as 4 | 2; the last of the four is moved right, where it ranks
/// no lower, making 3 | 3.
#[test]
fn split_moves_a_cluster_to_fill_a_leaf() {
    let (none, quarter) = (Probability::ZERO, Probability::new(1, 4));
    assert_eq!(split_of(&[none, none, quarter, quarter, quarter, quarter]), (2, 3));
}

/// llc peels the likeliest cluster before it looks for tables or bit tests:
/// `-2..=1` and `4` all to one target is a peeled range and a compare, not
/// the bit test (mask 79) the clusters alone would make. The first run of
/// the instrument's sweep read 79 against llc's nothing.
#[test]
fn a_dominant_cluster_is_peeled_before_bit_tests() {
    let clusters = lowered("-2:c -1:c 0:c 1:c 4:c");
    assert!(clusters.iter().all(CaseCluster::is_range), "{clusters:?}");
    let mut clusters = clusters_of("-2:c -1:c 0:c 1:c 4:c");
    sort_and_rangeify(&mut clusters);
    let (peeled, top) = peel_dominant_case_cluster(&mut clusters, &SwitchTarget::x86(32)).unwrap();
    assert_eq!(
        (peeled.low, peeled.high, top),
        (
            -2,
            1,
            Probability::new(1, 6)
                .plus(Probability::new(1, 6))
                .plus(Probability::new(1, 6))
                .plus(Probability::new(1, 6))
        )
    );
}

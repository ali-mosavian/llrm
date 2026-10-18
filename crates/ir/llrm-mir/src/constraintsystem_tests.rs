use super::*;

const MX: i64 = i64::MAX;
const MN: i64 = i64::MIN + 1;

/// The rows of `tools/llvmport/constraintsystem_dump.cpp`: row r has the
/// constant `ks[r % |ks|]` and variable i the coefficient `cs[(r / |ks| /
/// |cs|^i) % |cs|]`; a zero coefficient has no entry.
struct Spec {
    vars: u16,
    rows: Vec<Row>,
}

fn spec(
    vars: u16,
    ks: &[i64],
    cs: &[i64],
) -> Spec {
    let total = ks.len() * cs.len().pow(u32::from(vars));
    let rows = (0..total)
        .map(|r| {
            let mut x = r;
            let mut row = vec![Entry::new(ks[x % ks.len()], 0)];
            x /= ks.len();
            for i in 0..vars {
                let c = cs[x % cs.len()];
                x /= cs.len();
                if c != 0 {
                    row.push(Entry::new(c, i + 1));
                }
            }
            row
        })
        .collect();
    Spec { vars, rows }
}

struct Hash(u64);

impl Hash {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn mix(
        &mut self,
        byte: u8,
    ) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn done(&self) -> String {
        format!("{:016x}", self.0)
    }
}

/// `answer` of the dump: the system made of rows `ids`, asked the three
/// questions of every row of the spec.
fn answer(
    hash: &mut Hash,
    spec: &Spec,
    ids: &[usize],
) {
    let mut system = ConstraintSystem::<u32>::new();
    let mut num_vars = 0;
    for &id in ids {
        if system.add_row(&spec.rows[id], spec.vars as usize) {
            num_vars = spec.vars;
        }
    }
    hash.mix(system.size() as u8);
    let mut copy = system.clone();
    hash.mix(u8::from(copy.may_have_solution()));
    for query in &spec.rows {
        if query[query.len() - 1].id > num_vars {
            hash.mix(b'-');
            continue;
        }
        hash.mix(u8::from(system.is_condition_implied(query.clone())));
        hash.mix(u8::from(system.is_condition_implied_in_sub_system(query)));
    }
}

fn golden() -> (String, crate::hash::HashMap<String, String>) {
    let text = include_str!("constraintsystem_golden.txt");
    let mut lines = text.lines();
    let version = lines.next().expect("a version line").to_string();
    let rows = lines
        .map(|line| {
            let at = line.rfind(' ').expect("a hash");
            (line[..at].to_string(), line[at + 1..].to_string())
        })
        .collect();
    (version, rows)
}

/// The instrument's premise: the golden file is the pinned LLVM's and lists
/// every family of systems.
#[test]
fn the_golden_file_is_the_pinned_llvms_and_lists_every_system() {
    let (version, rows) = golden();
    assert_eq!(
        version, "# LLVM 24.0.0git d1106deb71cc81016406a1917d0508d2df296266",
        "golden file not from the pinned binary"
    );
    assert_eq!(rows.len(), 108 + 108 * 108 + 36 * 36 + 64 * 64 + 20);
}

/// Every system of every family answers as the pinned LLVM's does: whether it
/// may have a solution, and whether each row is implied, over the whole system
/// and over its sub-system.
#[test]
fn systems_answer_as_the_pinned_llvm_does_for_every_family() {
    let (_, rows) = golden();
    let a = spec(3, &[-1, 0, 1, 2], &[-1, 0, 1]);
    assert_eq!(a.rows.len(), 108);
    for i in 0..108 {
        let mut hash = Hash::new();
        answer(&mut hash, &a, &[i]);
        assert_eq!(hash.done(), rows[&format!("A1 {i}")], "A1 {i}");
        for j in 0..108 {
            let mut hash = Hash::new();
            answer(&mut hash, &a, &[i, j]);
            assert_eq!(hash.done(), rows[&format!("A2 {i} {j}")], "A2 {i} {j}");
        }
    }
    let b = spec(2, &[-1, 0, 1, 2], &[-1, 0, 1]);
    for i in 0..36 {
        for j in 0..36 {
            let mut hash = Hash::new();
            for l in 0..36 {
                answer(&mut hash, &b, &[i, j, l]);
            }
            assert_eq!(hash.done(), rows[&format!("B3 {i} {j}")], "B3 {i} {j}");
        }
    }
    let o = spec(2, &[MN, -1, 1, MX], &[MN, -1, 1, MX]);
    for i in 0..64 {
        for j in 0..64 {
            let mut hash = Hash::new();
            answer(&mut hash, &o, &[i, j]);
            assert_eq!(hash.done(), rows[&format!("O {i} {j}")], "O {i} {j} (overflow)");
        }
    }
}

/// Fourier-Motzkin grows past 500 rows and gives up ("may have a solution") for
/// the larger of these: the answer and the rows left are the pinned LLVM's.
/// Premise: the give-up is reached, which the sizes show.
#[test]
fn systems_that_grow_past_500_rows_give_up_as_the_pinned_llvm_does() {
    let (_, rows) = golden();
    let mut gave_up = 0;
    for n in 3..=12_u16 {
        for unsat in [false, true] {
            let mut hash = Hash::new();
            let mut system = ConstraintSystem::<u32>::new();
            for i in 1..=n {
                for j in 1..=n {
                    if i != j {
                        let (low, high) = (i.min(j), i.max(j));
                        let sign = if i < j { 1 } else { -1 };
                        system.add_row(&[Entry::new(1, 0), Entry::new(sign, low), Entry::new(-sign, high)], n as usize);
                    }
                }
            }
            if unsat {
                system.add_row(&[Entry::new(-2, 0), Entry::new(1, 1), Entry::new(-1, 2)], n as usize);
            }
            hash.mix(system.size() as u8);
            let answer = system.may_have_solution();
            hash.mix(u8::from(answer));
            hash.mix(system.size().min(255) as u8);
            if unsat && answer {
                gave_up += 1;
            }
            assert_eq!(hash.done(), rows[&format!("G {n} {}", u8::from(unsat))], "G {n} {unsat}");
        }
    }
    assert!(gave_up > 0, "premise: an unsatisfiable system was answered 'may have a solution' by giving up");
}

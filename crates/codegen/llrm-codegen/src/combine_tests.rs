use super::*;

/// A block of a made-up machine: `mul` takes `MUL`, `add` 1, `madd` `madd`,
/// and a load takes 10.
#[derive(Clone, Debug, PartialEq)]
enum Op {
    Mul(Reg, Reg, Reg),
    Add(Reg, Reg, Reg),
    Madd(Reg, Reg, Reg, Reg),
    Load(Reg),
}

const MUL: u32 = 3;

struct Toy {
    ops: Vec<Op>,
    madd_latency: u32,
    /// The accumulator of `madd` is read this many cycles late (`ReadIMA`).
    accumulator_advance: u32,
    must_reduce_depth: bool,
}

impl Toy {
    fn new(
        ops: Vec<Op>,
        madd_latency: u32,
    ) -> Self {
        Self { ops, madd_latency, accumulator_advance: 2, must_reduce_depth: false }
    }

    fn node(
        &self,
        op: &Op,
    ) -> Node {
        let plain = |reg: Reg| Use { reg, advance: 0 };
        match *op {
            Op::Mul(d, a, b) => Node::new(&[d], &[plain(a), plain(b)], MUL),
            Op::Add(d, a, b) => Node::new(&[d], &[plain(a), plain(b)], 1),
            Op::Madd(d, a, b, c) => Node::new(
                &[d],
                &[plain(a), plain(b), Use { reg: c, advance: self.accumulator_advance }],
                self.madd_latency,
            ),
            Op::Load(d) => Node::new(&[d], &[], 10),
        }
    }

    fn readers(
        &self,
        reg: Reg,
    ) -> usize {
        self.ops.iter().map(|op| self.node(op).uses.iter().filter(|used| used.reg == reg).count()).sum()
    }
}

impl CombinerIsa for Toy {
    type Pattern = (usize, usize);

    fn nodes(&self) -> Vec<Node> {
        self.ops.iter().map(|op| self.node(op)).collect()
    }

    /// `add t, c` where `t` is made by a `mul` nothing else reads.
    fn patterns(
        &self,
        root: usize,
    ) -> Vec<(usize, usize)> {
        let Op::Add(_, x, y) = self.ops[root] else { return vec![] };
        let mut found = vec![];
        for (operand, other) in [(x, y), (y, x)] {
            let _ = other;
            if let Some(at) = self.ops[..root].iter().rposition(|op| matches!(op, Op::Mul(d, ..) if *d == operand)) {
                if self.readers(operand) == 1 {
                    found.push((at, operand as usize));
                }
            }
        }
        found
    }

    fn generate(
        &mut self,
        root: usize,
        (mul, product): (usize, usize),
    ) -> Option<Replacement> {
        let (Op::Mul(_, a, b), Op::Add(d, x, y)) = (self.ops[mul].clone(), self.ops[root].clone()) else { return None };
        let c = if x as usize == product { y } else { x };
        Some(Replacement { inserted: vec![self.node(&Op::Madd(d, a, b, c))], deleted: vec![mul, root] })
    }

    fn apply(
        &mut self,
        root: usize,
        pattern: (usize, usize),
        replacement: &Replacement,
    ) {
        let _ = pattern;
        let fused = {
            let (Op::Mul(_, a, b), Op::Add(d, x, y)) =
                (self.ops[replacement.deleted[0]].clone(), self.ops[root].clone())
            else {
                panic!("a pattern of this machine")
            };
            let c = if x == d { y } else { x };
            let c = if self.ops[replacement.deleted[0]] == Op::Mul(x, a, b) { y } else { c };
            Op::Madd(d, a, b, c)
        };
        self.ops[root] = fused;
        self.ops.remove(replacement.deleted[0]);
    }

    fn objective(
        &self,
        _pattern: (usize, usize),
    ) -> Objective {
        if self.must_reduce_depth { Objective::MustReduceDepth } else { Objective::Default }
    }
}

fn plain() -> Setting {
    Setting::default()
}

/// `t = mul a, b; r = add t, c` with every input ready at once is one `madd`
/// (the pin's MUL+ADD -> MADD, 3 cycles against 4).
#[test]
fn a_multiply_feeding_an_add_is_fused() {
    let mut toy = Toy::new(vec![Op::Mul(10, 1, 2), Op::Add(11, 10, 3)], 3);
    assert!(combined(&mut toy, plain()));
    assert_eq!(toy.ops, vec![Op::Madd(11, 1, 2, 3)]);
}

/// A fusion slower than the pin's bound stays two instructions. The bound is
/// the add's depth (3, it waits for the multiply) plus the latencies of the
/// two instructions it replaces (3 + 1): a `madd` of 8 cycles loses, of 7 wins.
#[test]
fn a_fusion_that_lengthens_the_path_is_left() {
    let mut fast_enough = Toy::new(vec![Op::Mul(10, 1, 2), Op::Add(11, 10, 3)], 7);
    assert!(combined(&mut fast_enough, plain()));
    let mut toy = Toy::new(vec![Op::Mul(10, 1, 2), Op::Add(11, 10, 3)], 8);
    assert!(!combined(&mut toy, plain()));
    assert_eq!(toy.ops.len(), 2);
}

/// The add waits on a load (depth 10), the multiply does not. The bound is
/// 10 + (3 + 1) = 14; the `madd` reads the load 2 cycles late, so it starts at
/// 8 and a latency of 6 meets the bound (14) where 7 passes it (15).
#[test]
fn a_fusion_is_judged_by_the_late_operand_not_the_multiply() {
    let ops = vec![Op::Load(3), Op::Mul(10, 1, 2), Op::Add(11, 10, 3)];
    let mut as_fast = Toy::new(ops.clone(), 6);
    assert!(combined(&mut as_fast, plain()));
    let mut slower = Toy::new(ops, 7);
    assert!(!combined(&mut slower, plain()));
}

/// A product something else reads is not fused (`canCombine` wants one use).
#[test]
fn a_product_with_two_readers_is_not_fused() {
    let mut toy = Toy::new(vec![Op::Mul(10, 1, 2), Op::Add(11, 10, 3), Op::Add(12, 10, 4)], 3);
    assert!(!combined(&mut toy, plain()));
    assert_eq!(toy.ops.len(), 3);
}

/// When the block is optimized for size the shorter sequence goes in even
/// where it is slower.
#[test]
fn a_block_optimized_for_size_takes_the_shorter_sequence() {
    let mut toy = Toy::new(vec![Op::Mul(10, 1, 2), Op::Add(11, 10, 3)], 8);
    assert!(!combined(&mut Toy::new(toy.ops.clone(), 8), plain()));
    assert!(combined(&mut toy, Setting { optimize_for_size: true, in_loop: false }));
    assert_eq!(toy.ops, vec![Op::Madd(11, 1, 2, 3)]);
}

/// `MustReduceDepth` needs the new root strictly shallower: with the
/// accumulator read on time the depths are equal (the add waits on the load
/// either way) and the pattern is left; read 2 cycles late it is shallower.
#[test]
fn a_pattern_that_must_reduce_depth_is_left_when_depth_is_equal() {
    let ops = vec![Op::Load(3), Op::Mul(10, 1, 2), Op::Add(11, 10, 3)];
    let mut equal = Toy::new(ops.clone(), 3);
    equal.accumulator_advance = 0;
    equal.must_reduce_depth = true;
    assert!(!combined(&mut equal, plain()));
    let mut shallower = Toy::new(ops, 3);
    shallower.must_reduce_depth = true;
    assert!(combined(&mut shallower, plain()));
}

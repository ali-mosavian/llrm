//! Random loops for checking the recurrence form against the interpreter.
//!
//! A case is one loop, `i = x; loop { e0..en; i += step }`, whose values are
//! random expressions of the counter and the invariants `a b c s x`: add,
//! sub, mul, shl by a constant, sext, zext and trunc, up to three deep.
//! Evaluating the loop with `which = j` returns `ej` on the last trip, so
//! any value's form can be compared with what the program computes.
//!
//! Seeds are replayable: `SCEV_SEED=k` runs seed `k` alone and prints its
//! text; `SCEV_SEEDS=n` runs `n` seeds instead of the default.

use llrm_mir::interpret::{Val, run};
use llrm_mir::module::Module;

use crate::testing::{DOS, parsed};

/// xorshift64*: no dependency, the same stream everywhere.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(
        &mut self,
        n: u64,
    ) -> u64 {
        self.next() % n
    }

    pub fn chance(
        &mut self,
        percent: u64,
    ) -> bool {
        self.below(100) < percent
    }

    /// A 16-bit value, the extremes likelier.
    pub fn word(&mut self) -> u16 {
        match self.below(8) {
            0 => 0,
            1 => 1,
            2 => 0xffff,
            3 => 0x7fff,
            4 => 0x8000,
            _ => self.next() as u16,
        }
    }
}

/// A value the loop computes.
#[derive(Clone, Debug)]
pub struct Tracked {
    pub name: String,
    pub width: u32,
    /// Degree in the counter: 0 an invariant, 1 affine, more not a recurrence.
    pub degree: u32,
    /// Whether an extension of a varying value is on the way: affine only
    /// where ranges say it does not wrap.
    pub extended: bool,
}

impl Tracked {
    /// A value a recurrence form should exist for.
    pub fn affine(&self) -> bool {
        self.degree == 1 && !self.extended
    }
}

pub struct Case {
    pub seed: u64,
    pub text: String,
    pub tracked: Vec<Tracked>,
}

struct Node {
    name: String,
    /// Defined before the loop (or a parameter or constant).
    early: bool,
    width: u32,
    degree: u32,
    extended: bool,
}

struct Builder {
    rng: Rng,
    next: usize,
    pre: Vec<String>,
    body: Vec<String>,
    tracked: Vec<Tracked>,
}

impl Builder {
    fn made(
        &mut self,
        from: &[&Node],
        text: impl FnOnce(&str) -> String,
        width: u32,
        degree: u32,
        extended: bool,
    ) -> Node {
        let name = format!("e{}", self.next);
        self.next += 1;
        let line = text(&name);
        // An invariant is built before the loop, where LICM leaves it.
        let early = degree == 0 && from.iter().all(|one| one.early);
        if early {
            self.pre.push(line)
        } else {
            self.body.push(line)
        }
        self.tracked.push(Tracked { name: name.clone(), width, degree, extended });
        Node { name: format!("%{name}"), early, width, degree, extended }
    }

    fn leaf(&mut self) -> Node {
        let (name, degree) = match self.rng.below(8) {
            0..=2 => ("i".to_owned(), 1),
            3 => ("a".to_owned(), 0),
            4 => ("b".to_owned(), 0),
            5 => ("c".to_owned(), 0),
            6 => ("s".to_owned(), 0),
            _ => (format!("{}", self.rng.word() as i16), 0),
        };
        Node {
            name: if degree == 1 || name.parse::<i64>().is_err() { format!("%{name}") } else { name },
            early: degree == 0,
            width: 16,
            degree,
            extended: false,
        }
    }

    fn expr(
        &mut self,
        depth: u32,
        width: u32,
    ) -> Node {
        if depth == 0 || self.rng.chance(15) {
            let leaf = self.leaf();
            return if width == 16 { leaf } else { self.cast(leaf, width) };
        }
        match self.rng.below(10) {
            0..=5 => {
                let (first, second) = (self.expr(depth - 1, width), self.expr(depth - 1, width));
                let kind = ["add", "sub", "mul"][self.rng.below(3) as usize];
                let degree = if kind == "mul" { first.degree + second.degree } else { first.degree.max(second.degree) };
                let extended = first.extended || second.extended;
                self.made(
                    &[&first, &second],
                    |name| format!("%{name} = {kind} i{width} {}, {}", operand(&first), operand(&second)),
                    width,
                    degree,
                    extended,
                )
            }
            6 | 7 => {
                let of = self.expr(depth - 1, width);
                let count = self.rng.below(u64::from(width.min(8)));
                self.made(
                    &[&of],
                    |name| format!("%{name} = shl i{width} {}, {count}", operand(&of)),
                    width,
                    of.degree,
                    of.extended,
                )
            }
            _ => {
                let of = self.expr(depth - 1, if width == 16 { 32 } else { 16 });
                self.cast(of, width)
            }
        }
    }

    /// `of` taken to `width`: sext or zext up, trunc down.
    fn cast(
        &mut self,
        of: Node,
        width: u32,
    ) -> Node {
        if of.width == width {
            return of;
        }
        let kind = if width < of.width { "trunc" } else { ["sext", "zext"][self.rng.below(2) as usize] };
        let (from, extended) = (of.width, of.extended || (width > of.width && of.degree > 0));
        let degree = if width > of.width && of.degree > 0 { 9 } else { of.degree };
        self.made(
            &[&of],
            |name| format!("%{name} = {kind} i{from} {} to i{width}", operand(&of)),
            width,
            degree,
            extended,
        )
    }
}

fn operand(node: &Node) -> String {
    node.name.clone()
}

/// Seeds the default run covers, or `SCEV_SEEDS` of them, or `SCEV_SEED`'s alone.
pub fn seeds() -> Vec<u64> {
    if let Some(one) = std::env::var("SCEV_SEED").ok().and_then(|text| text.parse().ok()) {
        return vec![one];
    }
    let count = std::env::var("SCEV_SEEDS").ok().and_then(|text| text.parse().ok()).unwrap_or(600);
    (0..count).collect()
}

/// The loop of `seed`, as MIR text with the values it tracks.
pub fn case(seed: u64) -> Case {
    let mut rng = Rng::new(seed);
    let step = if rng.chance(40) { "%s".to_owned() } else { (rng.word() as i16).to_string() };
    let start = if rng.chance(75) { "%x".to_owned() } else { (rng.word() as i16).to_string() };
    let mut builder = Builder { rng, next: 0, pre: Vec::new(), body: Vec::new(), tracked: Vec::new() };
    for _ in 0..1 + builder.rng.below(4) {
        let width = if builder.rng.chance(25) { 32 } else { 16 };
        builder.expr(3, width);
    }
    let mut text = String::new();
    text.push_str("define i32 @f(i16 %x, i16 %s, i16 %a, i16 %b, i16 %c, i16 %n, i16 %which) {\nb0:\n");
    for line in &builder.pre {
        text.push_str(&format!("  {line}\n"));
    }
    text.push_str("  br label %b1\n\nb1:\n  %i = phi i16 [ ");
    text.push_str(&format!("{start}, %b0 ], [ %inext, %b1 ]\n  %t = phi i16 [ 0, %b0 ], [ %tnext, %b1 ]\n"));
    for line in &builder.body {
        text.push_str(&format!("  {line}\n"));
    }
    let mut last = "0".to_owned();
    for (at, one) in builder.tracked.iter().enumerate() {
        let wide = if one.width == 32 {
            one.name.clone()
        } else {
            text.push_str(&format!("  %z{at} = zext i16 %{} to i32\n", one.name));
            format!("z{at}")
        };
        text.push_str(&format!(
            "  %p{at} = icmp eq i16 %which, {at}\n  %r{at} = select i1 %p{at}, i32 %{wide}, i32 {last}\n"
        ));
        last = format!("%r{at}");
    }
    text.push_str(&format!("  %inext = add i16 %i, {step}\n  %tnext = add i16 %t, 1\n  %go = icmp ult i16 %tnext, %n\n  br i1 %go, label %b1, label %b2\n\nb2:\n  ret i32 {last}\n}}\n"));
    Case { seed, text, tracked: builder.tracked }
}

impl Case {
    pub fn module(&self) -> Module {
        let module = parsed(&format!("{DOS}{}", self.text));
        let errors = llrm_mir::verify::verify(&module);
        assert!(errors.is_empty(), "seed {}: {errors:?}\n{}", self.seed, self.text);
        module
    }
}

/// The inputs one run feeds the loop.
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub x: u16,
    pub s: u16,
    pub a: u16,
    pub b: u16,
    pub c: u16,
}

impl Inputs {
    pub fn random(rng: &mut Rng) -> Self {
        Self { x: rng.word(), s: rng.word(), a: rng.word(), b: rng.word(), c: rng.word() }
    }

    pub fn named(
        &self,
        name: &str,
    ) -> Option<u16> {
        Some(match name {
            "x" => self.x,
            "s" => self.s,
            "a" => self.a,
            "b" => self.b,
            "c" => self.c,
            _ => return None,
        })
    }
}

/// `module`'s `@f` on `inputs`, with `which` selecting the value returned
/// on the loop's trip `trip` (the last of `trip + 1`), in the low `width` bits.
pub fn observed(
    module: &Module,
    inputs: &Inputs,
    which: usize,
    trip: u16,
) -> Option<u128> {
    let int = |n: u16| Val::Int { bits: u128::from(n), width: 16 };
    let arguments = vec![
        int(inputs.x),
        int(inputs.s),
        int(inputs.a),
        int(inputs.b),
        int(inputs.c),
        int(trip + 1),
        int(which as u16),
    ];
    match run(module, "f", arguments, 100_000) {
        Ok(Val::Int { bits, .. }) => Some(bits),
        other => panic!("{other:?}"),
    }
}

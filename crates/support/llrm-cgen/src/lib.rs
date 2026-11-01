//! Random small C programs free of undefined behaviour, for differential tests
//! of a backend against a reference compiler: the 8-bit fold of a 16-bit
//! checksum must agree. No dependencies; the callers run the compilers.
//!
//! The programs use unsigned arithmetic only (narrow operands are widened
//! first), signed values come from conversions of unsigned ones, no signed
//! division, shifts are by constants or masked counts, array indexes are
//! masked, loops are bounded, recursion is bounded. A program declares `buf`
//! (16 bytes), `tab` (8 bytes) and a volatile byte `zero` that reads 0, which
//! the caller defines (the 6502 test in `data.s`, a C test in a second file),
//! and defines `test`.
//!
//! Extracted from the 6502 backend's `tests/difftest.rs`; for
//! [`DataModel::INT16`] and the same seed the text is the one that test
//! generated (`tests/golden.rs`).

use std::fmt::Write;

/// The C types of the target a program is generated for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataModel {
    /// The width of `int` (and `unsigned`): 16 for cc65, 32 for m68k and the
    /// host.
    pub int_bits: u32,
}

impl DataModel {
    /// cc65: `int` is 16 bits, `long` 32.
    pub const INT16: DataModel = DataModel { int_bits: 16 };
    /// m68k, the host: `int` and `long` are 32 bits, `short` 16.
    pub const INT32: DataModel = DataModel { int_bits: 32 };

    /// The 16-bit unsigned type: `unsigned` where that is `int`'s width, else
    /// `unsigned short`.
    pub fn u16(self) -> &'static str {
        if self.int_bits == 16 { "unsigned" } else { "unsigned short" }
    }

    /// The signed view of a 16-bit value: `int` where that is 16 bits, else
    /// `short`.
    pub fn word_signed(self) -> &'static str {
        if self.int_bits == 16 { "int" } else { "short" }
    }
}

/// What a program is generated with.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub model: DataModel,
    /// Every starting value is `+ zero`, so the compiler cannot fold the
    /// program into a constant.
    pub opaque: bool,
    /// Signed views of unsigned values; off, the shape stays and the views are
    /// unsigned.
    pub signed: bool,
    /// Keep only the first `k` statements of `test`: the way a mismatch is cut
    /// down.
    pub limit: Option<usize>,
    /// Loops that read a local array through a pointer stepped after each
    /// read (`*w++`): what a backend turns into post-increment loads. Off,
    /// the programs are the ones generated before the option existed.
    pub walks: bool,
    /// Copies of one array's first elements over another of the same type
    /// (`__builtin_memcpy` of a constant size): what a backend turns into a
    /// run of `(An)+` moves. Needs `walks`.
    pub copies: bool,
}

impl Options {
    pub const fn new(model: DataModel) -> Options {
        Options { model, opaque: false, signed: true, limit: None, walks: false, copies: false }
    }
}

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn below(
        &mut self,
        n: usize,
    ) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn chance(
        &mut self,
        percent: usize,
    ) -> bool {
        self.below(100) < percent
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ty {
    U8,
    U16,
    U32,
}

impl Ty {
    pub fn c(
        self,
        model: DataModel,
    ) -> &'static str {
        match self {
            Ty::U8 => "unsigned char",
            Ty::U16 => model.u16(),
            Ty::U32 => "unsigned long",
        }
    }

    /// The type arithmetic on it is done in: `unsigned char` would promote to a
    /// signed `int`.
    pub fn wide(self) -> &'static str {
        match self {
            Ty::U8 | Ty::U16 => "unsigned",
            Ty::U32 => "unsigned long",
        }
    }

    pub fn bits(self) -> usize {
        match self {
            Ty::U8 => 8,
            Ty::U16 => 16,
            Ty::U32 => 32,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Ty::U32 => "UL",
            _ => "U",
        }
    }
}

const TYPES: [Ty; 3] = [Ty::U8, Ty::U16, Ty::U32];

#[derive(Clone)]
struct Var {
    name: String,
    ty: Ty,
}

struct Array {
    name: String,
    ty: Ty,
    len: usize,
}

struct Helper {
    name: String,
    #[allow(dead_code)]
    ret: Ty,
    params: Vec<Ty>,
}

/// What is in scope in the function being written.
struct Scope {
    vars: Vec<Var>,
    arrays: Vec<Array>,
    /// Loop counters that are being counted and must not be assigned.
    counters: Vec<Var>,
    has_struct: bool,
}

struct Gen {
    rng: Rng,
    helpers: Vec<Helper>,
    text: String,
    options: Options,
}

impl Gen {
    fn c(
        &self,
        ty: Ty,
    ) -> &'static str {
        ty.c(self.options.model)
    }

    /// The signed type a generated program views a value through;
    /// `Options::signed` off keeps the program's shape but makes the view
    /// unsigned, to tell which failures come from signed forms.
    fn signed_view(
        &self,
        byte: bool,
    ) -> &'static str {
        match (self.options.signed, byte) {
            (true, true) => "signed char",
            (true, false) => self.options.model.word_signed(),
            (false, true) => "unsigned char",
            (false, false) => "unsigned",
        }
    }

    /// ` + zero` under `Options::opaque`: `zero` is a volatile byte that reads
    /// 0, so the compiler cannot fold the starting values (and with them
    /// the whole program) into a checksum.
    fn opaque(&self) -> &'static str {
        if self.options.opaque { " + zero" } else { "" }
    }

    fn constant(
        &mut self,
        ty: Ty,
    ) -> String {
        let value = match self.rng.below(6) {
            0 => 0,
            1 => 1,
            2 => (1u64 << (ty.bits() - 1)) - 1,
            3 => (1u64 << ty.bits()) - 1,
            4 => 1 << self.rng.below(ty.bits()),
            _ => self.rng.next() & ((1u64 << ty.bits()) - 1),
        };
        format!("{value}{}", ty.suffix())
    }

    fn leaf(
        &mut self,
        ty: Ty,
        scope: &Scope,
    ) -> String {
        match self.rng.below(10) {
            0..=4 if !scope.vars.is_empty() => {
                let v = &scope.vars[self.rng.below(scope.vars.len())];
                format!("({}){}", self.c(ty), v.name)
            }
            5 if !scope.counters.is_empty() => {
                let v = &scope.counters[self.rng.below(scope.counters.len())];
                format!("({}){}", self.c(ty), v.name)
            }
            6 if !scope.arrays.is_empty() => {
                let a = &scope.arrays[self.rng.below(scope.arrays.len())];
                let index = self.index(scope, a.len);
                format!("({}){}[{index}]", self.c(ty), a.name)
            }
            7 => {
                let index = self.index(scope, 16);
                format!("({})buf[{index}]", self.c(ty))
            }
            8 if scope.has_struct => {
                let field = ["a", "b", "c"][self.rng.below(3)];
                format!("({})s.{field}", self.c(ty))
            }
            _ => self.constant(ty),
        }
    }

    /// An index in `0..len` (a power of two).
    fn index(
        &mut self,
        scope: &Scope,
        len: usize,
    ) -> String {
        if self.rng.chance(40) {
            return format!("{}", self.rng.below(len));
        }
        let e = self.expr(Ty::U8, 1, scope);
        format!("({e} & {})", len - 1)
    }

    fn expr(
        &mut self,
        ty: Ty,
        depth: usize,
        scope: &Scope,
    ) -> String {
        if depth == 0 || self.rng.chance(15) {
            return self.leaf(ty, scope);
        }
        let w = ty.wide();
        match self.rng.below(16) {
            0..=5 => {
                let op = ["+", "-", "*", "&", "|", "^"][self.rng.below(6)];
                let (l, r) = (self.expr(ty, depth - 1, scope), self.expr(ty, depth - 1, scope));
                format!("({})(({w}){l} {op} ({w}){r})", self.c(ty))
            }
            6 | 7 => {
                let op = if self.rng.chance(50) { "<<" } else { ">>" };
                let k = self.rng.below(ty.bits().max(16));
                let k = if ty == Ty::U8 { k % 16 } else { k % ty.bits() };
                let e = self.expr(ty, depth - 1, scope);
                format!("({})(({w}){e} {op} {k})", self.c(ty))
            }
            8 => {
                let op = if self.rng.chance(50) { "/" } else { "%" };
                let (l, r) = (self.expr(ty, depth - 1, scope), self.expr(ty, depth - 1, scope));
                format!("({})(({w}){l} {op} (({w}){r} | 1))", self.c(ty))
            }
            9 => {
                let c = self.cond(depth - 1, scope);
                let (a, b) = (self.expr(ty, depth - 1, scope), self.expr(ty, depth - 1, scope));
                format!("(({c}) ? {a} : {b})")
            }
            10 => {
                let e = self.expr(ty, depth - 1, scope);
                if self.rng.chance(50) {
                    format!("({})~({w}){e}", self.c(ty))
                } else {
                    format!("({})(0 - ({w}){e})", self.c(ty))
                }
            }
            11 => {
                // a signed view of an unsigned value: arithmetic shift, or a
                // sign-extending cast
                let from = TYPES[self.rng.below(2)];
                let signed = self.signed_view(from == Ty::U8);
                let e = self.expr(from, depth - 1, scope);
                if self.rng.chance(50) {
                    let k = self.rng.below(from.bits());
                    format!("({})(({signed})({}){e} >> {k})", self.c(ty), self.c(from))
                } else {
                    format!("({})({signed})({}){e}", self.c(ty), self.c(from))
                }
            }
            12 | 13 if !self.helpers.is_empty() => self.call(ty, depth, scope),
            _ => {
                let c = self.cond(depth - 1, scope);
                format!("({})({c})", self.c(ty))
            }
        }
    }

    fn call(
        &mut self,
        ty: Ty,
        depth: usize,
        scope: &Scope,
    ) -> String {
        let at = self.rng.below(self.helpers.len());
        let (name, params) = (self.helpers[at].name.clone(), self.helpers[at].params.clone());
        let args: Vec<String> = params.iter().map(|p| self.expr(*p, depth.saturating_sub(1), scope)).collect();
        format!("({}){name}({})", self.c(ty), args.join(", "))
    }

    fn cond(
        &mut self,
        depth: usize,
        scope: &Scope,
    ) -> String {
        match self.rng.below(8) {
            0..=3 => {
                let ty = TYPES[self.rng.below(3)];
                let op = ["<", "<=", ">", ">=", "==", "!="][self.rng.below(6)];
                let (l, r) = (self.expr(ty, depth.min(1), scope), self.expr(ty, depth.min(1), scope));
                format!("{l} {op} {r}")
            }
            4 => {
                // signed compare of unsigned values
                let byte = self.rng.chance(50);
                let (ty, signed) =
                    if byte { (Ty::U8, self.signed_view(true)) } else { (Ty::U16, self.signed_view(false)) };
                let op = ["<", "<=", ">", ">="][self.rng.below(4)];
                let (l, r) = (self.expr(ty, depth.min(1), scope), self.expr(ty, depth.min(1), scope));
                format!("({signed}){l} {op} ({signed}){r}")
            }
            5 if depth > 0 => format!("({}) && ({})", self.cond(depth - 1, scope), self.cond(depth - 1, scope)),
            6 if depth > 0 => format!("({}) || ({})", self.cond(depth - 1, scope), self.cond(depth - 1, scope)),
            7 if depth > 0 => format!("!({})", self.cond(depth - 1, scope)),
            _ => self.expr(Ty::U8, 1, scope),
        }
    }

    fn assign_target(
        &mut self,
        scope: &Scope,
    ) -> Option<Var> {
        (!scope.vars.is_empty()).then(|| scope.vars[self.rng.below(scope.vars.len())].clone())
    }

    /// One to `most` statements.
    fn some(
        &mut self,
        most: usize,
        depth: usize,
        scope: &mut Scope,
        indent: usize,
        out: &mut String,
    ) {
        let count = 1 + self.rng.below(most);
        self.stmts(count, depth, scope, indent, out);
    }

    fn stmts(
        &mut self,
        count: usize,
        depth: usize,
        scope: &mut Scope,
        indent: usize,
        out: &mut String,
    ) {
        let pad = "  ".repeat(indent);
        for _ in 0..count {
            match self.rng.below(if self.options.copies {
                15
            } else if self.options.walks {
                13
            } else {
                12
            }) {
                0..=4 => {
                    if let Some(v) = self.assign_target(scope) {
                        let e = self.expr(v.ty, 3, scope);
                        writeln!(out, "{pad}{} = {e};", v.name).unwrap();
                    }
                }
                5 => {
                    if let Some(a) = scope.arrays.get(self.rng.below(scope.arrays.len().max(1))) {
                        let (name, ty, len) = (a.name.clone(), a.ty, a.len);
                        let (index, e) = (self.index(scope, len), self.expr(ty, 2, scope));
                        writeln!(out, "{pad}{name}[{index}] = {e};").unwrap();
                    }
                }
                6 if depth > 0 => {
                    let c = self.cond(2, scope);
                    writeln!(out, "{pad}if ({c}) {{").unwrap();
                    self.some(3, depth - 1, scope, indent + 1, out);
                    if self.rng.chance(50) {
                        writeln!(out, "{pad}}} else {{").unwrap();
                        self.some(3, depth - 1, scope, indent + 1, out);
                    }
                    writeln!(out, "{pad}}}").unwrap();
                }
                7 if depth > 0 => {
                    let n = 1 + self.rng.below(9);
                    let counter = Var { name: format!("i{}", scope.counters.len()), ty: Ty::U8 };
                    let form = self.rng.below(3);
                    match form {
                        0 => writeln!(out, "{pad}for ({0} = 0; {0} < {n}; {0}++) {{", counter.name).unwrap(),
                        1 => writeln!(out, "{pad}for ({0} = {n}; {0}; {0}--) {{", counter.name).unwrap(),
                        _ => {
                            writeln!(out, "{pad}{0} = {n};\n{pad}while ({0}) {{\n{pad}  {0}--;", counter.name).unwrap();
                        }
                    }
                    scope.counters.push(counter);
                    self.some(4, depth - 1, scope, indent + 1, out);
                    if self.rng.chance(25) {
                        let c = self.cond(1, scope);
                        writeln!(out, "{pad}  if ({c}) {}", if self.rng.chance(50) { "break;" } else { "continue;" })
                            .unwrap();
                    }
                    scope.counters.pop();
                    writeln!(out, "{pad}}}").unwrap();
                }
                8 if depth > 0 => {
                    let e = self.expr(Ty::U8, 2, scope);
                    writeln!(out, "{pad}switch ({e} & 3) {{").unwrap();
                    for case in 0..self.rng.below(4) {
                        writeln!(out, "{pad}case {case}: ;").unwrap();
                        self.some(2, 0, scope, indent + 1, out);
                        if self.rng.chance(80) {
                            writeln!(out, "{pad}  break;").unwrap();
                        }
                    }
                    writeln!(out, "{pad}default: ;").unwrap();
                    self.stmts(1, 0, scope, indent + 1, out);
                    writeln!(out, "{pad}}}").unwrap();
                }
                12 => {
                    if let (Some(a), Some(v)) =
                        (scope.arrays.get(self.rng.below(scope.arrays.len().max(1))), self.assign_target(scope))
                    {
                        let (name, ty, len) = (a.name.clone(), a.ty, a.len);
                        let wide = v.ty.wide();
                        let step = if self.rng.chance(50) {
                            format!(
                                "{0} = ({1})({0} {2} ({3})*w++);",
                                v.name,
                                self.c(v.ty),
                                ["+", "-", "&", "|", "^"][self.rng.below(5)],
                                wide
                            )
                        } else {
                            format!(
                                "if (({wide})*w++ {op} ({wide}){name}) {name} = ({ty})({name} + 1);",
                                op = ["<", ">", "<=", ">=", "==", "!="][self.rng.below(6)],
                                name = v.name,
                                ty = self.c(v.ty),
                            )
                        };
                        writeln!(
                            out,
                            "{pad}{{ const {} *w = {name}; unsigned char n = {len}; while (n--) {{ {step} }} }}",
                            self.c(ty)
                        )
                        .unwrap();
                    }
                }
                13 | 14 => {
                    let n = scope.arrays.len();
                    let from = self.rng.below(n.max(1));
                    let start = self.rng.below(n.max(1));
                    // the next array of the same type, which is never the
                    // source
                    let to = (0..n)
                        .map(|k| (start + k) % n)
                        .find(|&k| k != from && scope.arrays[k].ty == scope.arrays[from].ty);
                    if let (Some(src), Some(dst)) = (scope.arrays.get(from), to.map(|k| &scope.arrays[k])) {
                        let count = 1 + self.rng.below(src.len.min(dst.len).min(6));
                        writeln!(out, "{pad}__builtin_memcpy({0}, {1}, {count} * sizeof({0}[0]));", dst.name, src.name)
                            .unwrap();
                    }
                }
                9 if scope.has_struct => {
                    let e = self.expr(Ty::U8, 2, scope);
                    writeln!(out, "{pad}bump(&s, {e});").unwrap();
                }
                _ => {
                    if let Some(v) = self.assign_target(scope) {
                        let e = self.expr(v.ty, 2, scope);
                        writeln!(
                            out,
                            "{pad}{0} = ({1})({0} {2} ({3}){e});",
                            v.name,
                            self.c(v.ty),
                            ["+", "^", "|"][self.rng.below(3)],
                            v.ty.wide()
                        )
                        .unwrap();
                    }
                }
            }
            if self.rng.chance(30) {
                let ck = self.fold(scope);
                writeln!(out, "{pad}{ck}").unwrap();
            }
        }
    }

    /// `ck` rotates left and takes one more value.
    fn fold(
        &mut self,
        scope: &Scope,
    ) -> String {
        let e = self.expr(Ty::U16, 1, scope);
        format!("ck = (ck << 1 | ck >> 15) ^ {e};")
    }

    /// One function: locals, array fills, `count` statements, a checksum of
    /// everything, a return.
    fn function(
        &mut self,
        name: &str,
        ret: Ty,
        params: &[Ty],
        top: bool,
    ) {
        let mut scope = Scope { vars: Vec::new(), arrays: Vec::new(), counters: Vec::new(), has_struct: top };
        let mut head = String::new();
        let list: Vec<String> = params.iter().enumerate().map(|(n, t)| format!("{} p{n}", self.c(*t))).collect();
        let list = if list.is_empty() { "void".to_owned() } else { list.join(", ") };
        writeln!(head, "{}{} {name}({list}) {{", if top { "" } else { "static " }, self.c(ret)).unwrap();
        for (n, t) in params.iter().enumerate() {
            scope.vars.push(Var { name: format!("p{n}"), ty: *t });
        }
        let locals = 2 + self.rng.below(4);
        let mut init = String::new();
        for n in 0..locals {
            let ty = TYPES[self.rng.below(3)];
            let v = Var { name: format!("v{n}"), ty };
            let c = self.constant(ty);
            writeln!(
                head,
                "  {} {} = {};",
                self.c(ty),
                v.name,
                if self.opaque().is_empty() { c } else { format!("({})({c}{})", self.c(ty), self.opaque()) }
            )
            .unwrap();
            scope.vars.push(v);
        }
        for n in 0..self.rng.below(3) {
            let ty = TYPES[self.rng.below(3)];
            let len = [4, 8, 16][self.rng.below(3)];
            writeln!(head, "  {} a{n}[{len}];", self.c(ty)).unwrap();
            writeln!(
                init,
                "  for (j = 0; j < {len}; j++) a{n}[j] = ({})(j * {}U + {}{});",
                self.c(ty),
                1 + 2 * self.rng.below(5),
                self.rng.below(7),
                self.opaque()
            )
            .unwrap();
            scope.arrays.push(Array { name: format!("a{n}"), ty, len });
        }
        writeln!(
            head,
            "  unsigned char j, i0, i1, i2, i3;\n  {} ck = {}{};",
            self.options.model.u16(),
            self.rng.below(1000),
            self.opaque()
        )
        .unwrap();
        if top {
            writeln!(head, "  struct S s;").unwrap();
            writeln!(
                init,
                "  s.a = {}{o}; s.b = {}{o}; s.c = {}{o};",
                self.rng.below(256),
                self.rng.below(1 << u16::BITS),
                self.rng.below(256),
                o = self.opaque()
            )
            .unwrap();
        }
        let count = if top { 6 + self.rng.below(8) } else { 2 + self.rng.below(4) };
        let mut pieces = Vec::new();
        for _ in 0..count {
            let mut piece = String::new();
            self.stmts(1, 2, &mut scope, 1, &mut piece);
            pieces.push(piece);
        }
        // `Options::limit` keeps the first k statements of `test`: the way a
        // mismatch is cut down
        let keep = self.options.limit.filter(|_| top).unwrap_or(pieces.len());
        let body: String = pieces[..keep.min(pieces.len())].concat();
        let mut tail = String::new();
        for v in scope.vars.clone() {
            writeln!(tail, "  ck = (ck << 1 | ck >> 15) ^ (unsigned){0} ^ (unsigned)({0} >> 8);", v.name).unwrap();
        }
        for a in &scope.arrays {
            writeln!(tail, "  for (j = 0; j < {}; j++) ck = (ck << 1 | ck >> 15) ^ (unsigned){}[j];", a.len, a.name)
                .unwrap();
        }
        if top {
            writeln!(tail, "  ck = (ck << 1 | ck >> 15) ^ s.a ^ s.b ^ s.c;").unwrap();
        }
        let r = self.expr(ret, 2, &scope);
        writeln!(self.text, "{head}{init}{body}{tail}  return ({})({r} ^ ({})ck);\n}}\n", self.c(ret), self.c(ret))
            .unwrap();
    }
}

/// The program: a struct, a helper that takes it by pointer, helper functions,
/// and `test`.
pub fn program(
    seed: u64,
    options: &Options,
) -> String {
    let u16 = options.model.u16();
    let mut g = Gen {
        rng: Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1),
        helpers: Vec::new(),
        text: String::new(),
        options: *options,
    };
    write!(
        g.text,
        "extern const unsigned char buf[16];\nextern const unsigned char tab[8];\nextern const volatile unsigned char zero;\nstruct S {{ unsigned char a; {u16} b; unsigned char c; }};\n\
         static void bump(struct S *p, unsigned char x) {{ p->a = (unsigned char)(p->a + x); p->b = ({u16})(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }}\n\n"
    )
    .unwrap();
    for n in 0..g.rng.below(4) {
        let ret = TYPES[g.rng.below(3)];
        let params: Vec<Ty> = (0..g.rng.below(4)).map(|_| TYPES[g.rng.below(3)]).collect();
        let name = format!("h{n}");
        g.function(&name, ret, &params, false);
        g.helpers.push(Helper { name, ret, params });
    }
    g.function("test", Ty::U16, &[], true);
    g.text
}

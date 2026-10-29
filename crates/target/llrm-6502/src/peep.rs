//! Cleanup of a function's ca65 text: values tracked through A, X, Y and the cells, dead loads and
//! stores removed by liveness, and the cells packed into zero-page registers.
//!
//! DEVIATE: spike-local, on the text `select.rs` emits, standing in for register allocation, copy
//! coalescing and the post-allocation clean-up (`MOSCopyOpt`). Reason: values live in memory cells
//! until there is a shared allocator for A, X, Y and `rc2..`. Exit: the allocator.
//!
//! A cell is a virtual byte (`v_f_3`, one half `w_f_4` or `w_f_4+1` of a word); `rcN` is a physical
//! zero-page register. Anything the pass does not know makes it give the function back unchanged.

use std::collections::{BTreeSet, HashMap, HashSet};

/// The zero-page registers a cell may take: the caller-saved `rc2..rc7`.
const POOL: [&str; 6] = ["rc2", "rc3", "rc4", "rc5", "rc6", "rc7"];
/// Locations that are not cells.
const A: usize = 0;
const X: usize = 1;
const Y: usize = 2;
/// N and Z together, carry, overflow.
const Z: usize = 3;
const C: usize = 4;
const V: usize = 5;
const FIXED: usize = 6;

#[derive(Clone, Debug, PartialEq)]
enum Line {
    Label(String),
    Dir(String),
    Ins(String, String),
}

fn parse(text: &str) -> Vec<Line> {
    text.lines()
        .map(|line| {
            let t = line.trim();
            if let Some(label) = t.strip_suffix(':').filter(|l| !l.contains(' ')) {
                Line::Label(label.to_owned())
            } else if t.starts_with('.') || t.is_empty() {
                Line::Dir(t.to_owned())
            } else {
                match t.split_once(' ') {
                    Some((op, arg)) => Line::Ins(op.to_owned(), arg.trim().to_owned()),
                    None => Line::Ins(t.to_owned(), String::new()),
                }
            }
        })
        .collect()
}

fn render(lines: &[Line]) -> String {
    let mut out = String::new();
    for line in lines {
        match line {
            Line::Label(l) => out.push_str(&format!("{l}:\n")),
            Line::Dir(d) => out.push_str(&format!("{d}\n")),
            Line::Ins(op, arg) if arg.is_empty() => out.push_str(&format!("  {op}\n")),
            Line::Ins(op, arg) => out.push_str(&format!("  {op} {arg}\n")),
        }
    }
    out
}

fn is_virtual(name: &str) -> bool {
    name.starts_with("v_") || name.starts_with("w_")
}

/// `rc6` is physical register 6.
fn physical(name: &str) -> Option<usize> {
    name.strip_prefix("rc")?.parse().ok()
}

/// The operand `arg`'s cell as `(name, offset)`: `v_f_3`, `w_f_4+1`, `rc2`.
fn cell_of(arg: &str) -> Option<(String, u8)> {
    let (name, offset) = match arg.split_once('+') {
        Some((name, offset)) => (name, offset.parse().ok()?),
        None => (arg, 0),
    };
    (is_virtual(name) || physical(name).is_some()).then(|| (name.to_owned(), offset))
}

/// Where an operand reads or writes.
enum Place {
    None,
    Imm,
    Cell(String),
    /// `(rcN),y` (indexed) or `(rcN)`: the pair `rcN`, `rcN+1`.
    Pointer(usize, bool),
    Memory,
}

fn place(arg: &str) -> Option<Place> {
    if arg.is_empty() {
        return Some(Place::None);
    }
    if arg.starts_with('#') {
        return Some(Place::Imm);
    }
    if let Some(inner) = arg.strip_prefix('(') {
        let name = inner.split(')').next()?;
        return Some(Place::Pointer(physical(name)?, arg.ends_with(",y")));
    }
    let base = arg.split(',').next()?;
    let indexed = arg.contains(',');
    match cell_of(base) {
        Some((name, offset)) if !indexed => Some(Place::Cell(key(&name, offset))),
        Some(_) => None,
        None => Some(Place::Memory),
    }
}

/// A cell's name as a location: `rc3+1` is `rc4`.
fn key(
    name: &str,
    offset: u8,
) -> String {
    match physical(name) {
        Some(n) => format!("rc{}", n + usize::from(offset)),
        None if offset == 0 => name.to_owned(),
        None => format!("{name}+{offset}"),
    }
}

/// The locations an instruction reads and writes, and whether it is a branch.
struct Effect {
    uses: Vec<String>,
    defs: Vec<String>,
}

fn loc(i: usize) -> String {
    ["A", "X", "Y", "Z", "C", "V"][i].to_owned()
}

/// `ctx` is what the call or return needs beyond the instruction: for `jsr` the argument registers
/// written since the block began, for `rts` the result registers beyond A and X.
fn effect(
    op: &str,
    arg: &str,
    ctx: &[usize],
) -> Option<Effect> {
    let p = place(arg)?;
    let mut uses: Vec<String> = Vec::new();
    let mut defs: Vec<String> = Vec::new();
    let read = |uses: &mut Vec<String>| match &p {
        Place::Cell(c) => uses.push(c.clone()),
        Place::Pointer(n, y) => {
            uses.extend([format!("rc{n}"), format!("rc{}", n + 1)]);
            if *y {
                uses.push(loc(Y));
            }
        }
        _ => {}
    };
    let write = |defs: &mut Vec<String>, uses: &mut Vec<String>| match &p {
        Place::Cell(c) => defs.push(c.clone()),
        Place::Pointer(n, y) => {
            uses.extend([format!("rc{n}"), format!("rc{}", n + 1)]);
            if *y {
                uses.push(loc(Y));
            }
        }
        _ => {}
    };
    let (a, x, y, z, c, v) = (loc(A), loc(X), loc(Y), loc(Z), loc(C), loc(V));
    match op {
        "lda" => {
            read(&mut uses);
            defs.extend([a, z]);
        }
        "ldx" => {
            read(&mut uses);
            defs.extend([x, z]);
        }
        "ldy" => {
            read(&mut uses);
            defs.extend([y, z]);
        }
        "sta" => {
            uses.push(a);
            write(&mut defs, &mut uses);
        }
        "stx" => {
            uses.push(x);
            write(&mut defs, &mut uses);
        }
        "sty" => {
            uses.push(y);
            write(&mut defs, &mut uses);
        }
        "stz" => write(&mut defs, &mut uses),
        "tax" => {
            uses.push(a);
            defs.extend([x, z]);
        }
        "tay" => {
            uses.push(a);
            defs.extend([y, z]);
        }
        "txa" => {
            uses.push(x);
            defs.extend([a, z]);
        }
        "tya" => {
            uses.push(y);
            defs.extend([a, z]);
        }
        "adc" | "sbc" => {
            read(&mut uses);
            uses.extend([a.clone(), c.clone()]);
            defs.extend([a, c, v, z]);
        }
        "and" | "ora" | "eor" => {
            read(&mut uses);
            uses.push(a.clone());
            defs.extend([a, z]);
        }
        "cmp" => {
            read(&mut uses);
            uses.push(a);
            defs.extend([c, z]);
        }
        "cpx" => {
            read(&mut uses);
            uses.push(x);
            defs.extend([c, z]);
        }
        "cpy" => {
            read(&mut uses);
            uses.push(y);
            defs.extend([c, z]);
        }
        "bit" => {
            read(&mut uses);
            uses.push(a);
            defs.extend([v, z]);
        }
        "asl" | "lsr" | "rol" | "ror" => {
            if matches!(p, Place::None) || arg == "a" {
                uses.push(a.clone());
                defs.push(a);
            } else {
                read(&mut uses);
                write(&mut defs, &mut uses);
            }
            if op == "rol" || op == "ror" {
                uses.push(c.clone());
            }
            defs.extend([c, z]);
        }
        "inc" | "dec" => {
            read(&mut uses);
            write(&mut defs, &mut uses);
            defs.push(z);
        }
        "inx" | "dex" => {
            uses.push(x.clone());
            defs.extend([x, z]);
        }
        "iny" | "dey" => {
            uses.push(y.clone());
            defs.extend([y, z]);
        }
        "clc" | "sec" => defs.push(c),
        "pha" => uses.push(a),
        "pla" => defs.extend([a, z]),
        "php" => uses.extend([z, c, v]),
        "plp" => defs.extend([z, c, v]),
        "beq" | "bne" | "bmi" | "bpl" => uses.push(z),
        "bcc" | "bcs" => uses.push(c),
        "bvc" | "bvs" => uses.push(v),
        "jmp" | "bra" => {}
        "rts" => {
            uses.extend([a, x, y]);
            uses.extend([0, 1].iter().chain(&(8..16).collect::<Vec<_>>()).chain(ctx).map(|n| format!("rc{n}")));
        }
        "jsr" => {
            uses.extend([a.clone(), x.clone(), y.clone()]);
            uses.extend([0, 1].iter().chain(ctx).map(|n| format!("rc{n}")));
            defs.extend([a, x, y, z, c, v]);
            defs.extend((2..8).map(|n| format!("rc{n}")));
        }
        _ => return None,
    }
    Some(Effect { uses, defs })
}

fn is_branch(op: &str) -> bool {
    matches!(op, "beq" | "bne" | "bmi" | "bpl" | "bcc" | "bcs" | "bvc" | "bvs" | "bra" | "jmp")
}

fn ends_block(op: &str) -> bool {
    matches!(op, "jmp" | "bra" | "rts")
}

/// A pure load or transfer: nothing but its destinations change.
fn pure(
    op: &str,
    arg: &str,
) -> bool {
    matches!(op, "lda" | "ldx" | "ldy" | "tax" | "tay" | "txa" | "tya") && !arg.contains('(')
}

/// One instruction's liveness, over the whole function.
struct Flow {
    effects: Vec<Option<Effect>>,
    /// Live after each instruction.
    live_out: Vec<HashSet<String>>,
}

/// Every instruction's effect. A call reads the argument registers (`rc2..rc7`) written since the
/// block began or the last call.
fn effects(
    lines: &[Line],
    result_rcs: &[usize],
) -> Option<Vec<Option<Effect>>> {
    let mut written: Vec<usize> = Vec::new();
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let Line::Ins(op, arg) = line else {
            if matches!(line, Line::Label(_)) {
                written.clear();
            }
            out.push(None);
            continue;
        };
        let e = effect(op, arg, if op == "rts" { result_rcs } else { &written })?;
        for d in &e.defs {
            if let Some(n) = physical(d).filter(|n| (2..8).contains(n)) {
                if !written.contains(&n) {
                    written.push(n);
                }
            }
        }
        if op == "jsr" || is_branch(op) {
            written.clear();
        }
        out.push(Some(e));
    }
    Some(out)
}

fn liveness(
    lines: &[Line],
    result_rcs: &[usize],
) -> Option<Flow> {
    let effects = effects(lines, result_rcs)?;
    let labels: HashMap<&str, usize> =
        lines.iter().enumerate().filter_map(|(i, l)| if let Line::Label(n) = l { Some((n.as_str(), i)) } else { None }).collect();
    // successors by instruction index
    let successors = |i: usize| -> Vec<usize> {
        let mut out = Vec::new();
        if let Line::Ins(op, arg) = &lines[i] {
            if is_branch(op) {
                if let Some(&to) = labels.get(arg.as_str()) {
                    out.push(to);
                }
            }
            if ends_block(op) {
                return out;
            }
        }
        if i + 1 < lines.len() {
            out.push(i + 1);
        }
        out
    };
    let mut live_in: Vec<HashSet<String>> = vec![HashSet::new(); lines.len()];
    let mut live_out: Vec<HashSet<String>> = vec![HashSet::new(); lines.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for i in (0..lines.len()).rev() {
            let mut out = HashSet::new();
            for s in successors(i) {
                out.extend(live_in[s].iter().cloned());
            }
            let mut inn = out.clone();
            if let Some(e) = &effects[i] {
                for d in &e.defs {
                    inn.remove(d);
                }
                inn.extend(e.uses.iter().cloned());
            }
            if inn != live_in[i] || out != live_out[i] {
                live_in[i] = inn;
                live_out[i] = out;
                changed = true;
            }
        }
    }
    Some(Flow { effects, live_out })
}

/// Dead loads and stores out. A store to a cell nothing reads, a load or transfer into registers
/// nothing reads.
fn remove_dead(
    lines: &mut Vec<Line>,
    result_rcs: &[usize],
) -> Option<bool> {
    let flow = liveness(lines, result_rcs)?;
    let mut keep = vec![true; lines.len()];
    for (i, line) in lines.iter().enumerate() {
        let Line::Ins(op, arg) = line else { continue };
        let Some(e) = &flow.effects[i] else { continue };
        let removable = match op.as_str() {
            "sta" | "stx" | "sty" | "stz" => matches!(place(arg), Some(Place::Cell(c)) if is_virtual(&c)),
            _ => pure(op, arg),
        };
        if removable && !e.defs.is_empty() && e.defs.iter().all(|d| !flow.live_out[i].contains(d)) {
            keep[i] = false;
        }
    }
    let before = lines.len();
    let mut at = 0;
    lines.retain(|_| {
        at += 1;
        keep[at - 1]
    });
    Some(lines.len() != before)
}

/// What a register or cell holds, by value number.
#[derive(Default)]
struct Values {
    next: u32,
    regs: [Option<u32>; 3],
    cells: HashMap<String, u32>,
    immediates: HashMap<String, u32>,
    /// The value the flags N and Z describe.
    flags: Option<u32>,
}

impl Values {
    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next
    }

    fn reset(&mut self) {
        self.regs = [None; 3];
        self.cells.clear();
        self.flags = None;
    }

    fn of(
        &mut self,
        place: &Place,
        arg: &str,
    ) -> Option<u32> {
        match place {
            Place::Imm => {
                if let Some(&v) = self.immediates.get(arg) {
                    return Some(v);
                }
                let v = self.fresh();
                self.immediates.insert(arg.to_owned(), v);
                Some(v)
            }
            Place::Cell(c) => self.cells.get(c).copied(),
            _ => None,
        }
    }
}

/// Redundant loads out or turned into transfers, redundant stores out. Per block: a label forgets.
fn track_values(lines: &mut Vec<Line>) -> bool {
    let mut values = Values::default();
    let mut changed = false;
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    for line in lines.iter() {
        let Line::Ins(op, arg) = line else {
            if matches!(line, Line::Label(_)) {
                values.reset();
            }
            out.push(line.clone());
            continue;
        };
        let Some(p) = place(arg) else {
            values.reset();
            out.push(line.clone());
            continue;
        };
        let load = |reg: usize, values: &mut Values, out: &mut Vec<Line>, line: &Line, changed: &mut bool| {
            let known = values.of(&p, arg);
            let v = match known {
                Some(v) => v,
                None => {
                    let v = values.fresh();
                    if let Place::Cell(c) = &p {
                        values.cells.insert(c.clone(), v);
                    }
                    v
                }
            };
            if values.regs[reg] == Some(v) && values.flags == Some(v) {
                *changed = true;
                return;
            }
            let held = (0..3).find(|&r| r != reg && values.regs[r] == Some(v));
            let replaced = match (known.is_some(), held, reg) {
                (true, Some(A), X) => Some("tax"),
                (true, Some(A), Y) => Some("tay"),
                (true, Some(X), A) => Some("txa"),
                (true, Some(Y), A) => Some("tya"),
                _ => None,
            };
            match replaced {
                Some(t) => {
                    out.push(Line::Ins(t.to_owned(), String::new()));
                    *changed = true;
                }
                None => out.push(line.clone()),
            }
            values.regs[reg] = Some(v);
            values.flags = Some(v);
        };
        match op.as_str() {
            "lda" if !matches!(p, Place::Memory | Place::Pointer(..)) => load(A, &mut values, &mut out, line, &mut changed),
            "ldx" if !matches!(p, Place::Memory | Place::Pointer(..)) => load(X, &mut values, &mut out, line, &mut changed),
            "ldy" if !matches!(p, Place::Memory | Place::Pointer(..)) => load(Y, &mut values, &mut out, line, &mut changed),
            "lda" | "ldx" | "ldy" => {
                let reg = match op.as_str() {
                    "lda" => A,
                    "ldx" => X,
                    _ => Y,
                };
                let v = values.fresh();
                values.regs[reg] = Some(v);
                values.flags = Some(v);
                out.push(line.clone());
            }
            "sta" | "stx" | "sty" => {
                let reg = match op.as_str() {
                    "sta" => A,
                    "stx" => X,
                    _ => Y,
                };
                if let Place::Cell(c) = &p {
                    match values.regs[reg] {
                        // a store to a physical register stays: it may be a call's argument, which `jsr` reads
                        Some(v) if values.cells.get(c) == Some(&v) && is_virtual(c) => {
                            changed = true;
                            continue;
                        }
                        Some(v) => {
                            values.cells.insert(c.clone(), v);
                        }
                        None => {
                            values.cells.remove(c);
                        }
                    }
                }
                out.push(line.clone());
            }
            "tax" | "tay" | "txa" | "tya" => {
                let (from, to) = match op.as_str() {
                    "tax" => (A, X),
                    "tay" => (A, Y),
                    "txa" => (X, A),
                    _ => (Y, A),
                };
                let v = values.regs[from].unwrap_or_else(|| values.fresh());
                values.regs[from] = Some(v);
                values.regs[to] = Some(v);
                values.flags = Some(v);
                out.push(line.clone());
            }
            _ => {
                let Some(e) = effect(op, arg, &[]) else {
                    values.reset();
                    out.push(line.clone());
                    continue;
                };
                // each register or cell the instruction writes holds a new value; N and Z describe
                // the first of them, unless the instruction only compares
                let mut primary = None;
                for d in &e.defs {
                    let fresh = values.fresh();
                    match d.as_str() {
                        "A" => values.regs[A] = Some(fresh),
                        "X" => values.regs[X] = Some(fresh),
                        "Y" => values.regs[Y] = Some(fresh),
                        "Z" | "C" | "V" => continue,
                        cell => {
                            values.cells.insert(cell.to_owned(), fresh);
                        }
                    }
                    primary.get_or_insert(fresh);
                }
                if e.defs.iter().any(|d| d == "Z") {
                    values.flags = if matches!(op.as_str(), "cmp" | "cpx" | "cpy" | "bit" | "plp" | "jsr") { None } else { primary };
                }
                if op == "jsr" {
                    values.cells.retain(|c, _| is_virtual(c));
                }
                if ends_block(op) {
                    values.reset();
                }
                out.push(line.clone());
            }
        }
    }
    *lines = out;
    changed
}

/// Cells to zero-page registers by interference.
fn assign(
    lines: &mut Vec<Line>,
    result_rcs: &[usize],
) -> Option<()> {
    let flow = liveness(lines, result_rcs)?;
    // loop weight: an instruction between a label and a later branch back to it
    let labels: HashMap<&str, usize> =
        lines.iter().enumerate().filter_map(|(i, l)| if let Line::Label(n) = l { Some((n.as_str(), i)) } else { None }).collect();
    let mut depth = vec![0u32; lines.len()];
    for (i, line) in lines.iter().enumerate() {
        if let Line::Ins(op, arg) = line {
            if is_branch(op) {
                if let Some(&to) = labels.get(arg.as_str()).filter(|&&to| to <= i) {
                    for d in &mut depth[to..=i] {
                        *d += 1;
                    }
                }
            }
        }
    }
    let mut weight: HashMap<String, u64> = HashMap::new();
    let mut edges: HashMap<String, HashSet<String>> = HashMap::new();
    let mut copies: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        let Line::Ins(op, arg) = line else { continue };
        let Some(e) = &flow.effects[i] else { return None };
        for name in e.uses.iter().chain(&e.defs).filter(|n| is_virtual(n)) {
            *weight.entry(name.clone()).or_default() += 1 + 8 * u64::from(depth[i]).min(2);
        }
        for d in e.defs.iter().filter(|d| is_virtual(d) || physical(d).is_some()) {
            for l in flow.live_out[i].iter().filter(|l| *l != d) {
                if is_virtual(d) || is_virtual(l) {
                    edges.entry(d.clone()).or_default().insert(l.clone());
                    edges.entry(l.clone()).or_default().insert(d.clone());
                }
            }
        }
        // a copy `lda s; sta d` prefers one register
        if op == "sta" && i > 0 {
            if let (Line::Ins(prev, from), Some(Place::Cell(to))) = (&lines[i - 1], place(arg)) {
                if let (true, Some(Place::Cell(from))) = (prev == "lda", place(from)) {
                    copies.entry(to.clone()).or_default().insert(from.clone());
                    copies.entry(from).or_default().insert(to);
                }
            }
        }
    }
    let mut cells: Vec<String> = weight.keys().cloned().collect();
    cells.sort_by(|a, b| weight[b].cmp(&weight[a]).then(a.cmp(b)));
    let mut color: HashMap<String, &str> = HashMap::new();
    for cell in &cells {
        let taken = |reg: &str, color: &HashMap<String, &str>| {
            edges.get(cell).is_some_and(|n| n.iter().any(|o| o == reg || color.get(o) == Some(&reg)))
        };
        let preferred = copies.get(cell).into_iter().flatten().filter_map(|o| if physical(o).is_some() { POOL.iter().find(|r| **r == o) } else { color.get(o).and_then(|c| POOL.iter().find(|r| *r == c)) });
        let choice = preferred.chain(POOL.iter()).find(|reg| !taken(reg, &color));
        if let Some(reg) = choice {
            color.insert(cell.clone(), reg);
        }
    }
    for line in lines.iter_mut() {
        if let Line::Ins(_, arg) = line {
            if let Some(new) = rename(arg, &color) {
                *arg = new;
            }
        }
    }
    Some(())
}

/// `arg` with its cell replaced by the register it was given.
fn rename(
    arg: &str,
    color: &HashMap<String, &str>,
) -> Option<String> {
    let (head, tail) = match arg.split_once(',') {
        Some((h, t)) => (h, format!(",{t}")),
        None => (arg, String::new()),
    };
    let (immediate, body) = match head.strip_prefix('#') {
        Some(b) => ("#", b),
        None => ("", head),
    };
    if immediate == "#" {
        return None;
    }
    let (name, offset) = match body.split_once('+') {
        Some((n, o)) => (n, o.parse::<u8>().ok()?),
        None => (body, 0),
    };
    let reg = color.get(&key(name, offset))?;
    Some(format!("{reg}{tail}"))
}

/// The function text `body` (labels and instructions) cleaned up; `result_rcs` are the registers
/// beyond A and X that carry the result. `None` if it holds anything this pass does not model.
pub fn optimize(
    body: &str,
    result_rcs: &[usize],
) -> Option<String> {
    let mut lines = parse(body);
    effects(&lines, result_rcs)?;
    for _ in 0..6 {
        let mut changed = track_values(&mut lines);
        changed |= remove_dead(&mut lines, result_rcs)?;
        if !changed {
            break;
        }
    }
    assign(&mut lines, result_rcs)?;
    for _ in 0..6 {
        let mut changed = track_values(&mut lines);
        changed |= remove_dead(&mut lines, result_rcs)?;
        if !changed {
            break;
        }
    }
    Some(render(&lines))
}

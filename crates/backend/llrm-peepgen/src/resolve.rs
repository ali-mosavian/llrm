//! Names resolved against the instruction table, and every rule checked.

use std::collections::BTreeSet;

use indexmap::{IndexMap, IndexSet};

use crate::syntax::{
    Arg, Call, Class, Element, File, Group, Guard, Head, InsnPat, Item, NewHead, OpExpr, OperandPat, Operands, Rule,
    SetExpr, Value, Walk, WalkKind,
};
use crate::table::{self, Shape, Table};

/// The metadata fields `free(@k, SET)` can require empty.
pub const FIELDS: [&str; 15] = [
    "clobbers",
    "clobbers_high",
    "requires",
    "delivers",
    "spread",
    "group",
    "symbol",
    "frame_adjust",
    "spill_reload",
    "spill_store",
    "defines",
    "uses",
    "point",
    "unowned",
    "volatile",
];

/// Fields an override may set.
pub const OVERRIDES: [&str; 7] = ["at", "covers", "symbol", "defines", "uses", "widths", "call"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    D,
    S,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Reg,
    Mem,
    Imm,
    Held,
    Loc,
    /// A bound mnemonic.
    Name,
}

#[derive(Clone, Debug)]
pub struct Var {
    pub ty: Ty,
    pub slot: usize,
    pub side: Side,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct RInsn {
    pub ops: Vec<String>,
    /// The mnemonics allowed, each with its operation, or None for any.
    pub names: Option<Vec<(String, String)>>,
    pub dests: usize,
    /// None after `..`.
    pub sources: Option<usize>,
    pub operands: Vec<(Side, usize, OperandPat)>,
}

#[derive(Clone, Debug)]
pub struct RRule {
    pub name: String,
    pub line: usize,
    pub insns: Vec<RInsn>,
    pub gap: Option<Call>,
    pub end: bool,
    pub vars: IndexMap<String, Var>,
    /// Guards on the match alone, and those that read the rewrite (`$k`).
    pub pre: Vec<Guard>,
    pub post: Vec<Guard>,
    pub items: Vec<Item>,
    /// How many operands of each new item are destinations.
    pub item_dests: Vec<usize>,
    /// The instructions the window holds; `insns` past them are the
    /// definitions of the held values in `defs`, in order.
    pub window: usize,
    pub defs: Vec<String>,
}

#[derive(Debug)]
pub struct RGroup {
    pub name: String,
    pub walk: Walk,
    pub rules: Vec<RRule>,
    /// Instructions a window holds: the longest rule, one more to see an end.
    pub width: usize,
}

#[derive(Debug)]
pub struct RSet {
    pub names: Vec<String>,
    pub op: Option<String>,
    /// (fixed register, mnemonic), for sets whose members each name one.
    pub fixed: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct Program {
    pub metas: IndexMap<String, Vec<String>>,
    pub sets: IndexMap<String, RSet>,
    pub groups: Vec<RGroup>,
}

struct Ctx<'a> {
    table: &'a Table,
    file: &'a str,
    metas: &'a IndexMap<String, Vec<String>>,
    sets: &'a IndexMap<String, RSet>,
}

impl Ctx<'_> {
    fn error<T>(
        &self,
        line: usize,
        rule: &str,
        message: impl std::fmt::Display,
    ) -> Result<T, String> {
        Err(format!("{}:{line}: rule {rule}: {message}", self.file))
    }
}

fn eval_set(
    table: &Table,
    sets: &IndexMap<String, RSet>,
    expr: &SetExpr,
) -> Result<IndexSet<String>, String> {
    let every = || table.mnemonics.keys().cloned();
    Ok(match expr {
        SetExpr::Name(name) => match sets.get(name) {
            Some(set) => set.names.iter().cloned().collect(),
            None if table.mnemonics.contains_key(name) => IndexSet::from([name.clone()]),
            None => return Err(format!("unknown mnemonic or set '{name}'")),
        },
        SetExpr::Op(op) => {
            if !table::variant(op) {
                return Err(format!("unknown operation {op}"));
            }
            every().filter(|one| table.mnemonics[one].op == *op).collect()
        }
        SetExpr::Shape(text) => {
            every().filter(|one| table.mnemonics[one].shapes.iter().any(|shape| shape.text == *text)).collect()
        }
        SetExpr::Fixed => {
            every().filter(|one| table.mnemonics[one].shapes.iter().any(|shape| !shape.fixed.is_empty())).collect()
        }
        SetExpr::Reads(flags) => {
            let mut mask = 0;
            for flag in flags {
                mask |= if flag == "any" {
                    table::ARITHMETIC
                } else {
                    table::flag(flag).ok_or_else(|| format!("unknown flag {flag}"))?
                };
            }
            every().filter(|one| table.mnemonics[one].reads & mask != 0).collect()
        }
        SetExpr::Bin(left, op, right) => {
            let (left, right) = (eval_set(table, sets, left)?, eval_set(table, sets, right)?);
            match *op {
                "|" => left.union(&right).cloned().collect(),
                "&" => left.intersection(&right).cloned().collect(),
                _ => left.difference(&right).cloned().collect(),
            }
        }
    })
}

fn common<T: Clone + PartialEq>(items: impl IntoIterator<Item = T>) -> Option<T> {
    let mut items = items.into_iter();
    let first = items.next()?;
    items.all(|one| one == first).then_some(first)
}

pub fn resolve(
    file: &File,
    table: &Table,
    name: &str,
) -> Result<Program, String> {
    let mut metas: IndexMap<String, Vec<String>> = IndexMap::new();
    for meta in &file.metas {
        let mut fields = Vec::new();
        for field in &meta.fields {
            if let Some(included) = metas.get(field) {
                fields.extend(included.iter().cloned());
            } else if FIELDS.contains(&field.as_str()) {
                fields.push(field.clone());
            } else {
                return Err(format!(
                    "{name}:{}: meta {}: unknown field {field}; the fields are {}",
                    meta.line,
                    meta.name,
                    FIELDS.join(", ")
                ));
            }
        }
        fields.dedup();
        if metas.insert(meta.name.clone(), fields).is_some() {
            return Err(format!("{name}:{}: meta {} is defined twice", meta.line, meta.name));
        }
    }
    let mut sets: IndexMap<String, RSet> = IndexMap::new();
    for set in &file.sets {
        let names: Vec<String> = eval_set(table, &sets, &set.expr)
            .map_err(|one| format!("{name}:{}: set {}: {one}", set.line, set.name))?
            .into_iter()
            .collect();
        if names.is_empty() {
            return Err(format!("{name}:{}: set {} is empty", set.line, set.name));
        }
        let members = || names.iter().map(|one| &table.mnemonics[one]);
        // A family: each member pins one register, which names it.
        let pinned = |one: &table::Mnem| {
            one.shapes.iter().flat_map(|shape| shape.fixed.iter().cloned()).collect::<IndexSet<String>>()
        };
        let fixed = if members().all(|one| pinned(one).len() == 1) {
            names.iter().map(|one| (pinned(&table.mnemonics[one])[0].clone(), one.clone())).collect()
        } else {
            Vec::new()
        };
        let made = RSet { op: common(members().map(|one| one.op.clone())), fixed, names };
        if sets.insert(set.name.clone(), made).is_some() {
            return Err(format!("{name}:{}: set {} is defined twice", set.line, set.name));
        }
    }
    let cx = Ctx { table, file: name, metas: &metas, sets: &sets };
    let mut groups = Vec::new();
    let mut seen = IndexSet::new();
    for group in &file.groups {
        groups.push(resolve_group(&cx, group, &mut seen)?);
    }
    Ok(Program { metas, sets, groups })
}

fn resolve_group(
    cx: &Ctx,
    group: &Group,
    seen: &mut IndexSet<String>,
) -> Result<RGroup, String> {
    let Some(walk) = group.walk.clone() else {
        return Err(format!("{}:{}: group {} has no walk", cx.file, group.line, group.name));
    };
    if group.rules.is_empty() {
        return Err(format!("{}:{}: group {} has no rules", cx.file, group.line, group.name));
    }
    let mut rules = Vec::new();
    for rule in &group.rules {
        if !seen.insert(rule.name.clone()) {
            return cx.error(rule.line, &rule.name, "is defined twice");
        }
        rules.push(resolve_rule(cx, rule, &walk)?);
    }
    if walk.kind == WalkKind::Gap {
        let first = &rules[0];
        for rule in &rules[1..] {
            if rule.gap != first.gap {
                return cx.error(
                    rule.line,
                    &rule.name,
                    format!("its gap differs from rule {}'s; a gap group has one", first.name),
                );
            }
            if let Some(gap) = &rule.gap {
                for var in call_idents(gap) {
                    let (Some(a), Some(b)) = (rule.vars.get(&var), first.vars.get(&var)) else { continue };
                    if (a.side, a.index) != (b.side, b.index) {
                        return cx.error(
                            rule.line,
                            &rule.name,
                            format!("binds {var} elsewhere than rule {} does", first.name),
                        );
                    }
                }
            }
        }
    }
    let longest = rules.iter().map(|rule| rule.window).max().unwrap_or(0);
    let width = longest + usize::from(rules.iter().any(|rule| rule.end));
    Ok(RGroup { name: group.name.clone(), walk, rules, width })
}

fn call_idents(call: &Call) -> Vec<String> {
    let mut out = Vec::new();
    for arg in &call.args {
        match arg {
            Arg::Ident(one) => out.push(one.clone()),
            Arg::Call(inner) => out.extend(call_idents(inner)),
            _ => {}
        }
    }
    out
}

/// How many of `listed` operands are destinations, by the forms' shapes:
/// one count must fit, else `/` has to say.
fn dests_of<'a>(
    shapes: impl Iterator<Item = &'a Shape>,
    listed: usize,
    rest: bool,
    what: &str,
) -> Result<usize, String> {
    let counts: BTreeSet<usize> = shapes
        .filter(
            |shape| if rest { shape.dests + shape.sources >= listed } else { shape.dests + shape.sources == listed },
        )
        .map(|shape| shape.dests.min(listed))
        .collect();
    match counts.len() {
        1 => Ok(counts.into_iter().next().expect("one")),
        0 => Err(format!("no form of {what} takes {listed} operand(s)")),
        _ => Err(format!("{what}'s forms take {listed} operand(s) with {counts:?} destinations; part them with '/'")),
    }
}

fn resolve_head<'t>(
    cx: &Ctx<'t>,
    rule: &Rule,
    head: &Head,
) -> Result<(Vec<String>, Option<Vec<(String, String)>>, Vec<&'t Shape>), String> {
    let t = cx.table;
    let names: Vec<String> = match head {
        Head::Any(ops) => {
            for op in ops {
                if !table::variant(op) {
                    return cx.error(rule.line, &rule.name, format!("unknown operation {op}"));
                }
            }
            let shapes = ops.iter().flat_map(|op| t.shapes_of(op)).collect();
            return Ok((ops.clone(), None, shapes));
        }
        Head::Name(one) => match cx.sets.get(one) {
            Some(set) => set.names.clone(),
            None if t.mnemonics.contains_key(one) => vec![one.clone()],
            None => return cx.error(rule.line, &rule.name, format!("unknown mnemonic or set '{one}'")),
        },
        Head::Alt(names) => {
            for one in names {
                if !t.mnemonics.contains_key(one) {
                    return cx.error(rule.line, &rule.name, format!("unknown mnemonic '{one}'"));
                }
            }
            names.clone()
        }
    };
    let shapes = names.iter().flat_map(|one| &t.mnemonics[one].shapes).collect();
    let ops: Vec<String> =
        names.iter().map(|one| t.mnemonics[one].op.clone()).collect::<IndexSet<_>>().into_iter().collect();
    let names = names.iter().map(|one| (one.clone(), t.mnemonics[one].op.clone())).collect();
    Ok((ops, Some(names), shapes))
}

fn resolve_rule(
    cx: &Ctx,
    rule: &Rule,
    walk: &Walk,
) -> Result<RRule, String> {
    let fail = |message: String| cx.error(rule.line, &rule.name, message);
    let mut insns = Vec::new();
    let mut gap = None;
    let mut end = false;
    let mut vars: IndexMap<String, Var> = IndexMap::new();
    let mut defs: Vec<String> = Vec::new();
    for (at, element) in rule.pattern.iter().enumerate() {
        if end {
            return fail("'$' ends the pattern".into());
        }
        if !defs.is_empty() && !matches!(element, Element::Def(..)) {
            return fail("definitions follow the window's instructions".into());
        }
        match element {
            Element::Def(value, pattern) => {
                if vars.get(value).is_none_or(|var| var.ty != Ty::Held) {
                    return fail(format!("def {value}: {value} is not a held value the window binds"));
                }
                defs.push(value.clone());
                insns.push(resolve_insn(cx, rule, pattern, insns.len(), &mut vars)?);
            }
            Element::End => {
                if insns.is_empty() {
                    return fail("'$' follows an instruction".into());
                }
                end = true;
            }
            Element::Gap(call) => {
                if at != 1 || gap.is_some() {
                    return fail("a gap '...' comes only after the first instruction".into());
                }
                gap = Some(call.clone());
            }
            Element::Insn(pattern) => insns.push(resolve_insn(cx, rule, pattern, insns.len(), &mut vars)?),
        }
    }
    let window = insns.len() - defs.len();
    if window == 0 {
        return fail("matches nothing".into());
    }
    if !defs.is_empty() && (walk.kind != WalkKind::Window || gap.is_some() || end) {
        return fail("only a window walk matches definitions".into());
    }
    if gap.is_some() && insns.len() < 2 {
        return fail("a gap needs an instruction after it".into());
    }
    match walk.kind {
        WalkKind::Gap if gap.is_none() => {
            return fail("a gap walk's rules have a gap after the first instruction".into());
        }
        WalkKind::Gap => {}
        _ if gap.is_some() => return fail("only a gap walk takes a gap".into()),
        WalkKind::Each if insns.len() != 1 || end => return fail("an each walk matches one instruction".into()),
        _ if end => return fail("only a gap walk anchors at '$'".into()),
        _ => {}
    }
    if let Some(gap) = &gap {
        check_call(cx, rule, gap, &vars, insns.len(), 0, true, true)?;
        for var in call_idents(gap) {
            if vars.get(&var).is_some_and(|one| one.slot != 0) {
                return fail(format!("the gap reads {var}, which the first instruction does not bind"));
            }
        }
    }

    // In place, each item is what one matched slot becomes, in order.
    let in_place = matches!(walk.kind, WalkKind::Each | WalkKind::Slide | WalkKind::Gap);
    if in_place && rule.rewrite.len() != window {
        return fail(format!("rewrites {window} instructions in place into {} items", rule.rewrite.len()));
    }
    let mut item_dests = Vec::new();
    for (index, item) in rule.rewrite.iter().enumerate() {
        if !matches!(item, Item::New { .. }) {
            item_dests.push(0);
        }
        let (slot, over) = match item {
            Item::Keep { slot, over } | Item::Anchor { slot, over } => (*slot, over.as_slice()),
            Item::Drop { slot } => (*slot, &[][..]),
            Item::New { slot, head, operands, over } => {
                item_dests.push(check_new(cx, rule, &insns, &vars, head, operands, index)?);
                (*slot, over.as_slice())
            }
        };
        if slot >= insns.len() {
            return fail(format!("@{slot} is past the pattern's {} instructions", insns.len()));
        }
        if slot >= window && !matches!(item, Item::Drop { .. }) {
            return fail(format!("@{slot} is a definition, which a rewrite can only drop"));
        }
        if in_place && slot != index {
            return fail(format!("item {index} rewrites @{slot}; in place, item i rewrites @i"));
        }
        if matches!(item, Item::Drop { .. }) && !matches!(walk.kind, WalkKind::Window | WalkKind::Slide) {
            return fail("only a window or slide walk drops".into());
        }
        for one in over {
            if !OVERRIDES.contains(&one.field.as_str()) {
                return fail(format!("cannot set {}; the fields are {}", one.field, OVERRIDES.join(", ")));
            }
            match &one.value {
                Value::Field(slot, field) => {
                    if *slot >= insns.len() || !OVERRIDES.contains(&field.as_str()) {
                        return fail(format!("@{slot}.{field} is not a field of the match"));
                    }
                }
                Value::Bool(_) if one.field != "symbol" => return fail(format!("{} is not a flag", one.field)),
                Value::Call(call) => check_call(cx, rule, call, &vars, insns.len(), index, false, false)?,
                _ => {}
            }
        }
    }

    let (mut pre, mut post) = (Vec::new(), Vec::new());
    for guard in &rule.guards {
        check_call(cx, rule, &guard.call, &vars, insns.len(), rule.rewrite.len(), true, false)?;
        if reads_built(&guard.call) { post.push(guard.clone()) } else { pre.push(guard.clone()) }
    }
    Ok(RRule {
        name: rule.name.clone(),
        line: rule.line,
        insns,
        gap,
        end,
        vars,
        pre,
        post,
        items: rule.rewrite.clone(),
        item_dests,
        window,
        defs,
    })
}

fn reads_built(call: &Call) -> bool {
    call.args
        .iter()
        .any(
            |arg| match arg {
                Arg::Built(_) => true,
                Arg::Call(inner) => reads_built(inner),
                _ => false,
            },
        )
}

fn head_text(head: &Head) -> String {
    match head {
        Head::Name(one) => one.clone(),
        Head::Alt(names) => format!("({})", names.join("|")),
        Head::Any(ops) => format!("any({})", ops.join("|")),
    }
}

fn resolve_insn(
    cx: &Ctx,
    rule: &Rule,
    pattern: &InsnPat,
    slot: usize,
    vars: &mut IndexMap<String, Var>,
) -> Result<RInsn, String> {
    let fail = |message: String| cx.error(rule.line, &rule.name, message);
    let (ops, names, shapes) = resolve_head(cx, rule, &pattern.head)?;
    let dests = match pattern.split {
        Some(split) => split,
        None => dests_of(shapes.into_iter(), pattern.operands.len(), pattern.rest, &head_text(&pattern.head))
            .or_else(|message| cx.error(rule.line, &rule.name, message))?,
    };
    if let Some(bound) = &pattern.bind {
        if matches!(pattern.head, Head::Any(_)) {
            return fail(format!("{bound} binds a mnemonic; any(...) has none to bind"));
        }
        if vars.insert(bound.clone(), Var { ty: Ty::Name, slot, side: Side::D, index: 0 }).is_some() {
            return fail(format!("{bound} is bound twice"));
        }
    }
    let mut operands = Vec::new();
    for (at, operand) in pattern.operands.iter().enumerate() {
        let (side, index) = if at < dests { (Side::D, at) } else { (Side::S, at - dests) };
        if let OperandPat::Bind { name, kind } = operand {
            match (vars.get(name), kind) {
                (Some(var), None) if var.ty == Ty::Name => {
                    return fail(format!("{name} names a mnemonic, not an operand"));
                }
                (Some(_), None) => {}
                (Some(_), Some(_)) => return fail(format!("{name} is bound twice; its later uses take no kind")),
                (None, None) => return fail(format!("{name} is not bound; give its kind, as {name}:reg")),
                (None, Some((class, _))) => {
                    let ty = match class {
                        Class::Reg => Ty::Reg,
                        Class::Mem => Ty::Mem,
                        Class::Imm | Class::Sym => Ty::Imm,
                        Class::Held => Ty::Held,
                        Class::Any => Ty::Loc,
                    };
                    vars.insert(name.clone(), Var { ty, slot, side, index });
                }
            }
        }
        operands.push((side, index, operand.clone()));
    }
    let sources = (!pattern.rest).then(|| pattern.operands.len() - dests);
    Ok(RInsn { ops, names, dests, sources, operands })
}

#[allow(clippy::too_many_arguments)]
fn check_call(
    cx: &Ctx,
    rule: &Rule,
    call: &Call,
    vars: &IndexMap<String, Var>,
    slots: usize,
    built: usize,
    guard_context: bool,
    crossing: bool,
) -> Result<(), String> {
    let fail = |message: String| cx.error(rule.line, &rule.name, message);
    for arg in &call.args {
        match arg {
            Arg::Slot(slot) if *slot >= slots => {
                return fail(format!("@{slot} is past the pattern's {slots} instructions"));
            }
            Arg::Built(k) if !guard_context || *k >= built => {
                return fail(format!("${k} is not an earlier item of the rewrite"));
            }
            Arg::Crossed if !crossing => return fail("'*' is the crossed instruction, only in a gap".into()),
            Arg::Ident(name) => {
                let known = vars.contains_key(name)
                    || cx.metas.contains_key(name)
                    || cx.sets.contains_key(name)
                    || table::flag(name).is_some()
                    || name == "gap";
                if !known {
                    return fail(format!("{name} in {}(...) is not bound, a meta, a set or a flag", call.name));
                }
            }
            Arg::Call(inner) => check_call(cx, rule, inner, vars, slots, built, guard_context, crossing)?,
            _ => {}
        }
    }
    Ok(())
}

fn check_new(
    cx: &Ctx,
    rule: &Rule,
    insns: &[RInsn],
    vars: &IndexMap<String, Var>,
    head: &NewHead,
    operands: &Operands,
    index: usize,
) -> Result<usize, String> {
    let fail = |message: String| cx.error(rule.line, &rule.name, message);
    let (listed, split) = match operands {
        Operands::List(list, split) => (list.len(), *split),
        Operands::Map(_) => (0, Some(0)),
    };
    let dests = |shapes: Vec<&Shape>, what: &str| match split {
        Some(split) => Ok(split),
        None => dests_of(shapes.into_iter(), listed, false, what).or_else(|message| fail(message)),
    };
    let made = match head {
        NewHead::Name(name) => match vars.get(name) {
            Some(var) if var.ty == Ty::Name => {
                if insns[var.slot].ops.len() != 1 {
                    return fail(format!("{name} may be several operations; rewrite with =@{} instead", var.slot));
                }
                split.unwrap_or(insns[var.slot].dests)
            }
            Some(_) => return fail(format!("{name} is an operand, not a mnemonic")),
            None => match cx.table.mnemonics.get(name) {
                Some(found) => dests(found.shapes.iter().collect(), name)?,
                None => return fail(format!("unknown mnemonic '{name}'")),
            },
        },
        NewHead::Copy(slot) if *slot >= insns.len() => return fail(format!("=@{slot} is past the pattern")),
        NewHead::Copy(slot) => split.unwrap_or(insns[*slot].dests),
        NewHead::Family(set, key) => {
            let Some(found) = cx.sets.get(set) else { return fail(format!("unknown set {set}")) };
            if found.fixed.is_empty() || found.op.is_none() {
                return fail(format!("set {set} is no family: each member needs one fixed register and one operation"));
            }
            if vars.get(key).is_none_or(|var| var.ty != Ty::Reg) {
                return fail(format!("{set}[{key}] needs {key} bound to a register"));
            }
            dests(found.names.iter().flat_map(|one| &cx.table.mnemonics[one].shapes).collect(), set)?
        }
    };
    if listed < made && !matches!(operands, Operands::Map(_)) {
        return fail(format!("the rewrite lists {listed} operand(s) for {made} destination(s)"));
    }
    match operands {
        Operands::Map(call) => {
            if !matches!(head, NewHead::Copy(_)) {
                return fail("'map f(...)' maps the operands of an =@k copy".into());
            }
            check_call(cx, rule, call, vars, insns.len(), index, false, false)?;
        }
        Operands::List(list, _) => {
            for one in list {
                match one {
                    OpExpr::Var(name) => match vars.get(name) {
                        None => return fail(format!("{name} is not bound")),
                        Some(var) if var.ty == Ty::Name => {
                            return fail(format!("{name} names a mnemonic, not an operand"));
                        }
                        _ => {}
                    },
                    OpExpr::Call(call) => check_call(cx, rule, call, vars, insns.len(), index, false, false)?,
                    OpExpr::Lit(..) => {}
                }
            }
        }
    }
    Ok(made)
}

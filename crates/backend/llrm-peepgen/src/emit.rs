//! Rust for the automata, the rules at their accepting states, and the table.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::automaton::{Automaton, Key, Node, NodeId, Part, Val};
use crate::resolve::{Program, RGroup, RRule, Side, Ty};
use crate::syntax::{Arg, Call, Item, NewHead, OpExpr, Operands, Skip, Value, WalkKind};
use crate::table::{self, Table};

fn upper(name: &str) -> String {
    name.to_uppercase()
}

fn side(side: Side) -> &'static str {
    match side {
        Side::D => "Side::D",
        Side::S => "Side::S",
    }
}

fn key_expr(key: Key) -> String {
    match key {
        Key::Op(slot) => format!("w.op({slot})"),
        Key::Name(slot) => format!("w.name({slot})"),
        Key::Dests(slot) => format!("w.dests({slot})"),
        Key::Sources(slot) => format!("w.sources({slot})"),
        Key::Operand(slot, s, index, part) => {
            let what = match part {
                Part::Kind => "kind",
                Part::Width => "width",
                Part::Bare => "bare",
                Part::Value => "value",
            };
            format!("w.{what}({slot}, {}, {index})", side(s))
        }
    }
}

fn val_pat(value: &Val) -> String {
    match value {
        Val::Op(op) => format!("Some(Operation::{op})"),
        Val::Name(name) => format!("Some({name:?})"),
        Val::Count(count) => count.to_string(),
        Val::Kind(kind) => format!("Kind::{kind}"),
        Val::Width(width) => format!("Some({width})"),
        Val::Bool(flag) => format!("Some({flag})"),
        Val::Int(value) => format!("Some({value})"),
    }
}

struct Rule<'a> {
    rule: &'a RRule,
    program: &'a Program,
    table: &'a Table,
}

impl Rule<'_> {
    fn arg(&self, arg: &Arg) -> String {
        match arg {
            Arg::Slot(slot) => format!("w.slot({slot})"),
            Arg::Built(k) => format!("&n{k}"),
            Arg::Crossed => "x".into(),
            Arg::Int(value) => value.to_string(),
            Arg::List(values) => format!("&{values:?}"),
            Arg::Call(call) => self.call("make", call),
            Arg::Ident(name) => {
                if self.rule.vars.contains_key(name) {
                    format!("v_{name}")
                } else if self.program.metas.contains_key(name) {
                    format!("META_{}", upper(name))
                } else if self.program.sets.contains_key(name) {
                    format!("&SET_{}", upper(name))
                } else if table::flag(name).is_some() {
                    format!("Lanes::flags(RflagsBits::{name})")
                } else {
                    "w.gap()".into()
                }
            }
        }
    }

    fn call(&self, module: &str, call: &Call) -> String {
        let args: Vec<String> = call.args.iter().map(|arg| self.arg(arg)).collect();
        if module == "guards" {
            format!("guards::{}(cx{}{})", call.name, if args.is_empty() { "" } else { ", " }, args.join(", "))
        } else {
            format!("{module}::{}({})", call.name, args.join(", "))
        }
    }

    fn loc(&self, name: &str) -> String {
        match self.rule.vars[name].ty {
            Ty::Reg => format!("Loc::Reg(v_{name})"),
            Ty::Mem => format!("Loc::Mem(v_{name}.clone())"),
            Ty::Imm => format!("Loc::Imm(v_{name}.clone())"),
            Ty::Held => format!("Loc::Held(v_{name})"),
            Ty::Loc | Ty::Name => format!("v_{name}.clone()"),
        }
    }

    fn operand(&self, one: &OpExpr) -> String {
        match one {
            OpExpr::Var(name) => self.loc(name),
            OpExpr::Call(call) => self.call("make", call),
            OpExpr::Lit(value, width) => format!("Loc::Imm(Imm {{ value: {value}, width: {width}, address: None }})"),
        }
    }

    fn semantics(&self, head: &NewHead, operands: &Operands, dests: usize) -> Result<String, String> {
        let split = |dests: usize| -> Result<(String, String), String> {
            let Operands::List(list, _) = operands else { unreachable!("checked: a map needs a copy") };
            let code: Vec<String> = list.iter().map(|one| self.operand(one)).collect();
            Ok((code[..dests].join(", "), code[dests..].join(", ")))
        };
        let fresh = |op: &str, name: String, dests: usize| -> Result<String, String> {
            let (dests, sources) = split(dests)?;
            Ok(format!(
                "Semantics {{ op: Operation::{op}, name: Some({name}.to_owned()), dests: vec![{dests}], sources: vec![{sources}], target: None, indirect: false }}"
            ))
        };
        match head {
            NewHead::Name(name) => match self.rule.vars.get(name) {
                Some(var) => fresh(&self.rule.insns[var.slot].ops[0], format!("v_{name}"), dests),
                None => fresh(&self.table.mnemonics[name].op, format!("{name:?}"), dests),
            },
            NewHead::Family(set, key) => {
                let op = self.program.sets[set].op.clone().expect("checked");
                fresh(&op, format!("SET_{}.by_fixed(v_{key}.register)?", upper(set)), dests)
            }
            NewHead::Copy(slot) => {
                let parts = match operands {
                    Operands::Map(call) => {
                        let args: Vec<String> = call.args.iter().map(|arg| self.arg(arg)).collect();
                        let map = |list: &str| {
                            format!("{list}: s.{list}.iter().map(|o| make::{}(o, {})).collect()", call.name, args.join(", "))
                        };
                        format!("{}, {}", map("dests"), map("sources"))
                    }
                    Operands::List(..) => {
                        let (dests, sources) = split(dests)?;
                        format!("dests: vec![{dests}], sources: vec![{sources}]")
                    }
                };
                Ok(format!("{{ let s = w.what({slot})?; Semantics {{ {parts}, ..s.clone() }} }}"))
            }
        }
    }

    fn overrides(&self, over: &[crate::syntax::Override]) -> String {
        let mut out = String::new();
        for one in over {
            let value = match &one.value {
                Value::Field(slot, field) => format!("w.insn({slot}).{field}.clone()"),
                Value::Empty => "Vec::new()".into(),
                Value::Bool(flag) => format!("Some({flag})"),
                Value::Call(call) => self.call("make", call),
            };
            let _ = write!(out, " n.{} = {value};", one.field);
        }
        out
    }

    fn body(&self) -> Result<String, String> {
        let rule = self.rule;
        let mut out = String::new();
        let len = rule.window;
        if rule.end {
            let _ = writeln!(out, "    if w.len() != {len} {{ return None; }}");
        }
        let bind = |out: &mut String, slots: std::ops::Range<usize>| {
            for (name, var) in rule.vars.iter().filter(|(_, var)| slots.contains(&var.slot)) {
                let accessor = match var.ty {
                    Ty::Name => {
                        let _ = writeln!(out, "    let v_{name} = w.name({})?;", var.slot);
                        continue;
                    }
                    Ty::Reg => "reg",
                    Ty::Mem => "mem",
                    Ty::Imm => "imm",
                    Ty::Held => "held",
                    Ty::Loc => "loc",
                };
                let _ = writeln!(out, "    let v_{name} = w.{accessor}({}, {}, {})?;", var.slot, side(var.side), var.index);
            }
        };
        bind(&mut out, 0..len);
        // Each definition joins the window, once its shape is checked.
        for (at, value) in rule.defs.iter().enumerate() {
            let slot = len + at;
            let insn = &rule.insns[slot];
            let _ = writeln!(out, "    let w = w.defined(v_{value}.value)?;");
            let heads: Vec<String> = match &insn.names {
                Some(names) => names.iter().map(|(name, op)| format!("(Some(Operation::{op}), Some({name:?}))")).collect(),
                None => insn.ops.iter().map(|op| format!("(Some(Operation::{op}), _)")).collect(),
            };
            let _ = writeln!(out, "    if !matches!((w.op({slot}), w.name({slot})), {}) {{ return None; }}", heads.join(" | "));
            for (key, values) in crate::automaton::operand_tests(insn, slot, &rule.vars) {
                let pats: Vec<String> = values.iter().map(val_pat).collect();
                let _ = writeln!(out, "    if !matches!({}, {}) {{ return None; }}", key_expr(key), pats.join(" | "));
            }
            bind(&mut out, slot..slot + 1);
        }
        for (slot, insn) in rule.insns.iter().enumerate() {
            for (s, index, operand) in &insn.operands {
                if let crate::syntax::OperandPat::Bind { name, kind: None } = operand {
                    let var = &rule.vars[name];
                    let here = if var.ty == Ty::Loc { format!("v_{name}") } else { format!("&{}", self.loc(name)) };
                    let _ = writeln!(out, "    if w.loc({slot}, {}, {index})? != {here} {{ return None; }}", side(*s));
                }
            }
        }
        let guard = |out: &mut String, guard: &crate::syntax::Guard| {
            let call = self.call("guards", &guard.call);
            let test = if guard.negate { call } else { format!("!{call}") };
            let _ = writeln!(out, "    if {test} {{ return None; }}");
        };
        for one in &rule.pre {
            guard(&mut out, one);
        }
        let mut outs = Vec::new();
        for (index, item) in rule.items.iter().enumerate() {
            let made = match item {
                Item::Keep { slot, over } if over.is_empty() => format!("Arc::clone(w.insn({slot}))"),
                Item::Keep { slot, over } => {
                    format!("{{ let mut n = (**w.insn({slot})).clone();{} Arc::new(n) }}", self.overrides(over))
                }
                Item::Anchor { slot, over } if over.is_empty() => format!("lir::anchor(Arc::clone(w.insn({slot})))"),
                Item::Anchor { slot, over } => format!(
                    "{{ let mut n = (*lir::anchor(Arc::clone(w.insn({slot})))).clone();{} Arc::new(n) }}",
                    self.overrides(over)
                ),
                Item::Drop { slot } => format!("Arc::clone(w.insn({slot}))"),
                Item::New { slot, head, operands, over } => format!(
                    "{{ let mut n = (**w.insn({slot})).clone(); n.what = Some({});{} Arc::new(n) }}",
                    self.semantics(head, operands, rule.item_dests[index])?,
                    self.overrides(over)
                ),
            };
            let _ = writeln!(out, "    let n{index} = {made};");
            let wrap = match item {
                Item::Drop { slot } if *slot >= len => "Out::Retire",
                Item::Drop { .. } => "Out::Drop",
                _ => "Out::Put",
            };
            outs.push(format!("{wrap}(n{index})"));
        }
        for one in &rule.post {
            guard(&mut out, one);
        }
        let _ = writeln!(out, "    Some(Rewrite {{ len: {len}, out: vec![{}] }})", outs.join(", "));
        Ok(out)
    }

    fn crossing(&self) -> String {
        let gap = self.rule.gap.as_ref().expect("a gap rule");
        let mut out = String::new();
        let mut needed = Vec::new();
        collect(gap, &mut needed);
        for name in needed {
            let Some(var) = self.rule.vars.get(&name) else { continue };
            let accessor = match var.ty {
                Ty::Name => {
                    let _ = writeln!(out, "    let Some(v_{name}) = w.name(0) else {{ return false }};");
                    continue;
                }
                Ty::Reg => "reg",
                Ty::Mem => "mem",
                Ty::Imm => "imm",
                Ty::Held => "held",
                Ty::Loc => "loc",
            };
            let _ = writeln!(out, "    let Some(v_{name}) = w.{accessor}(0, {}, {}) else {{ return false }};", side(var.side), var.index);
        }
        let _ = writeln!(out, "    {}", self.call("guards", gap));
        out
    }
}

fn collect(call: &Call, out: &mut Vec<String>) {
    for arg in &call.args {
        match arg {
            Arg::Ident(name) if !out.contains(name) => out.push(name.clone()),
            Arg::Call(inner) => collect(inner, out),
            _ => {}
        }
    }
}

fn reachable(automaton: &Automaton, from: NodeId, head: bool, out: &mut Vec<NodeId>) {
    if from == 0 || out.contains(&from) || automaton.boundary(from) == head {
        return;
    }
    out.push(from);
    if let Node::Switch { arms, default, .. } = &automaton.nodes[from] {
        for (_, arm) in arms {
            reachable(automaton, *arm, head, out);
        }
        reachable(automaton, *default, head, out);
    }
}

fn emit_group(program: &Program, table: &Table, group: &RGroup, automaton: &Automaton) -> Result<String, String> {
    let mut out = String::new();
    let name = &group.name;
    let gap = group.walk.kind == WalkKind::Gap;
    let _ = writeln!(out, "\n/// {} rules, {} states.", group.rules.len(), automaton.states());
    let _ = writeln!(out, "pub mod {name} {{\n    use super::*;\n");
    let _ = writeln!(
        out,
        "    pub static MATCHER: Matcher = Matcher {{ width: {}, head, tail, cross: {} }};\n",
        group.width,
        if gap { "Some(cross)" } else { "None" }
    );

    // The first instruction's states, returning the state they reach.
    let state = |id: NodeId| -> String {
        if id == 0 || automaton.boundary(id) { id.to_string() } else { format!("h{id}(w)") }
    };
    let _ = writeln!(out, "    fn head(w: &Window) -> u32 {{\n        {}\n    }}\n", state(automaton.root));
    let mut heads = Vec::new();
    reachable(automaton, automaton.root, true, &mut heads);
    for id in &heads {
        let Node::Switch { key, arms, default } = &automaton.nodes[*id] else { unreachable!("a head state switches") };
        let _ = writeln!(out, "    fn h{id}(w: &Window) -> u32 {{\n        match {} {{", key_expr(*key));
        for (value, arm) in arms {
            let _ = writeln!(out, "            {} => {},", val_pat(value), state(*arm));
        }
        let _ = writeln!(out, "            _ => {},\n        }}\n    }}\n", state(*default));
    }

    // The rest, from each state the first instruction reaches.
    let mut boundaries: Vec<NodeId> = Vec::new();
    let mut tails = Vec::new();
    let mut seen = Vec::new();
    let visit = |id: NodeId, boundaries: &mut Vec<NodeId>| {
        if id != 0 && automaton.boundary(id) && !boundaries.contains(&id) {
            boundaries.push(id);
        }
    };
    visit(automaton.root, &mut boundaries);
    for id in &heads {
        if let Node::Switch { arms, default, .. } = &automaton.nodes[*id] {
            for (_, arm) in arms {
                visit(*arm, &mut boundaries);
            }
            visit(*default, &mut boundaries);
        }
    }
    for id in &boundaries {
        let mut from = Vec::new();
        reachable(automaton, *id, false, &mut from);
        for one in from {
            if !seen.contains(&one) {
                seen.push(one);
                tails.push(one);
            }
        }
    }
    let _ = writeln!(out, "    fn tail(cx: &Cx, w: &Window, state: u32) -> Option<Rewrite> {{\n        match state {{");
    for id in &boundaries {
        let _ = writeln!(out, "            {id} => n{id}(cx, w),");
    }
    let _ = writeln!(out, "            _ => None,\n        }}\n    }}\n");
    let next = |id: NodeId| if id == 0 { "None".to_owned() } else { format!("n{id}(cx, w)") };
    for id in &tails {
        let _ = write!(out, "    fn n{id}(cx: &Cx, w: &Window) -> Option<Rewrite> {{\n        ");
        match &automaton.nodes[*id] {
            Node::Fail => unreachable!("failure is no function"),
            Node::Accept(rules) => {
                let calls: Vec<String> = rules.iter().map(|rule| format!("r_{}(cx, w)", group.rules[*rule].name)).collect();
                let _ = write!(out, "{}", calls[0]);
                for call in &calls[1..] {
                    let _ = write!(out, ".or_else(|| {call})");
                }
                let _ = writeln!(out);
            }
            Node::Switch { key, arms, default } => {
                let _ = writeln!(out, "match {} {{", key_expr(*key));
                for (value, arm) in arms {
                    let _ = writeln!(out, "            {} => {},", val_pat(value), next(*arm));
                }
                let _ = writeln!(out, "            _ => {},\n        }}", next(*default));
            }
        }
        let _ = writeln!(out, "    }}\n");
    }

    for rule in &group.rules {
        let emitter = Rule { rule, program, table };
        let body = emitter.body().map_err(|one| format!("group {name}: {one}"))?;
        let _ = writeln!(out, "    /// Line {}.\n    fn r_{}(cx: &Cx, w: &Window) -> Option<Rewrite> {{\n{body}    }}\n", rule.line, rule.name);
    }

    if gap {
        // Every rule of a gap group has the same gap, so the first a state
        // can still accept says whether an instruction may be crossed.
        let _ = writeln!(out, "    fn cross(cx: &Cx, w: &Window, x: &Arc<Insn>, state: u32) -> bool {{\n        match state {{");
        let mut crossing = BTreeSet::new();
        for id in &boundaries {
            if let Some(first) = automaton.reachable(*id).into_iter().next() {
                crossing.insert(first);
                let _ = writeln!(out, "            {id} => c_{}(cx, w, x),", group.rules[first].name);
            }
        }
        let _ = writeln!(out, "            _ => false,\n        }}\n    }}\n");
        for rule in crossing.into_iter().map(|at| &group.rules[at]) {
            let emitter = Rule { rule, program, table };
            let _ = writeln!(out, "    fn c_{}(cx: &Cx, w: &Window, x: &Arc<Insn>) -> bool {{\n{}    }}\n", rule.name, emitter.crossing());
        }
    }
    let _ = writeln!(out, "}}");

    let skip = |skip: Skip| match skip {
        Skip::None => "Skip::None",
        Skip::Meta => "Skip::Meta",
        Skip::Inert => "Skip::Inert",
        Skip::Nothing => "Skip::Nothing",
    };
    let walk = &group.walk;
    let call = match walk.kind {
        WalkKind::Each => format!("walk::each(body, facts, &{name}::MATCHER)"),
        WalkKind::Window => format!("walk::window(body, facts, &{name}::MATCHER, {})", skip(walk.skip)),
        WalkKind::Slide => format!("walk::slide(body, facts, &{name}::MATCHER, {})", walk.advance),
        WalkKind::Gap => format!(
            "walk::gap(body, facts, &{name}::MATCHER, {}, {}, {})",
            skip(walk.skip),
            walk.first_original,
            walk.resume_past
        ),
    };
    let _ = writeln!(out, "\npub fn {name}(body: &LirBody, facts: &Facts) -> LirBody {{\n    {call}\n}}");
    if walk.kind == WalkKind::Window {
        let _ = writeln!(
            out,
            "\npub fn {name}_insns(insns: &[Arc<Insn>], facts: &Facts) -> Vec<Arc<Insn>> {{\n    walk::window_insns(insns, facts, &{name}::MATCHER, {})\n}}",
            skip(walk.skip)
        );
    }
    Ok(out)
}

pub fn rules(program: &Program, table: &Table, automata: &[Automaton], source: &str) -> Result<String, String> {
    let mut out = String::new();
    let _ = writeln!(out, "// Generated by llrm-peepgen from {source}. Do not edit.\n");
    for (name, fields) in &program.metas {
        let bits: Vec<String> = fields.iter().map(|one| format!("field::{}", upper(one))).collect();
        let _ = writeln!(out, "pub const META_{}: u32 = {};", upper(name), bits.join(" | "));
    }
    for (name, set) in &program.sets {
        let names: Vec<String> = set.names.iter().map(|one| format!("{one:?}")).collect();
        let fixed: Vec<String> =
            set.fixed.iter().map(|(register, one)| format!("(Register::{}, {one:?})", upper(register))).collect();
        let _ = writeln!(
            out,
            "pub static SET_{}: Set = Set {{ names: &[{}], fixed: &[{}] }};",
            upper(name),
            names.join(", "),
            fixed.join(", ")
        );
    }
    for (group, automaton) in program.groups.iter().zip(automata) {
        out.push_str(&emit_group(program, table, group, automaton)?);
    }
    Ok(out)
}

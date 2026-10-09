//! Instruction selection generated from `patterns.isel`, as TableGen
//! generates LLVM's DAG selector: the patterns become a decision automaton
//! over MIR opcodes, types and operand kinds, and a Rust emitter per
//! pattern. `build.rs` runs it; the selector includes what it writes.
//!
//! Dependency-free, so `build.rs` can include it with `#[path]`.

pub mod automaton;
pub mod parse;

use std::fmt::Write as _;

use automaton::{Automaton, State};
use parse::{Call, Expr, OperandPattern, Pattern, Step};

/// The instruction description's reader.
#[path = "../../../../../../target/llrm-x86/src/parse.rs"]
pub mod description;

/// What each operand constructor may make, as `x86.instr` spells kinds,
/// and how many arguments it takes.
pub const CONSTRUCTORS: [(&str, &str, usize); 13] = [
    ("loaded", "m", 1),
    ("held", "r", 1),
    ("source", "ri", 1),
    ("byte", "r", 1),
    ("count", "ri", 1),
    ("narrowed", "r", 1),
    ("result", "r", 0),
    ("fresh", "r", 0),
    ("float", "s", 1),
    ("fresult", "s", 0),
    ("cell", "m", 1),
    ("access", "m", 1),
    ("imm", "i", 2),
];

pub struct Generated {
    pub code: String,
    pub automaton: Automaton,
}

fn refused<T>(
    pattern: &Pattern,
    what: impl std::fmt::Display,
) -> Result<T, String> {
    Err(format!("patterns.isel:{}: pattern `{}`: {what}", pattern.line, pattern.name))
}

/// Where each operand a pattern binds is, as Rust: a root operand, or an
/// operand of the instruction defining one.
fn bindings(pattern: &Pattern) -> Vec<(String, String)> {
    fn walk(
        operands: &[OperandPattern],
        at: &dyn Fn(usize) -> String,
        out: &mut Vec<(String, String)>,
    ) {
        for (index, one) in operands.iter().enumerate() {
            let rust = at(index);
            if let Some(name) = &one.binding {
                out.push((name.clone(), rust.clone()));
            }
            if let Some(nested) = &one.nested {
                walk(&nested.operands, &|inner| format!("self.inner({rust}, {inner})"), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&pattern.operands, &|index| format!("m.ops[{index}]"), &mut out);
    out
}

fn names(list: &Option<Vec<String>>) -> String {
    list.as_ref().map_or("None".into(), |list| format!("Some(&{list:?})"))
}

/// What the automaton does not test of a pattern's operands: those past
/// its reach, literals, and nested instructions.
fn structure(
    operands: &[OperandPattern],
    at: &dyn Fn(usize) -> String,
    root: bool,
    terms: &mut Vec<String>,
) {
    for (index, one) in operands.iter().enumerate() {
        let rust = at(index);
        if !root || index >= parse::OPERANDS {
            terms.push(format!("self.operand_is({rust}, {}, {})", names(&one.kinds), names(&one.types)));
        }
        if let Some(literal) = one.literal {
            terms.push(format!("self.literal({rust}) == Some({literal})"));
        }
        if let Some(nested) = &one.nested {
            terms.push(format!(
                "self.defines({rust}, &{:?}, {}, {})",
                nested.opcodes,
                names(&nested.result),
                nested.operands.len()
            ));
            structure(&nested.operands, &|inner| format!("self.inner({rust}, {inner})"), false, terms);
        }
    }
}

struct Checker<'a> {
    pattern: &'a Pattern,
    forms: &'a [description::Form],
    bound: Vec<(String, String)>,
    lets: Vec<(String, String)>,
}

impl Checker<'_> {
    /// A MIR operand argument of a predicate, hook or cost.
    fn argument(
        &self,
        expr: &Expr,
    ) -> Result<String, String> {
        match expr {
            Expr::Name(name) => match self.bound.iter().find(|(one, _)| one == name) {
                Some((_, rust)) => Ok(rust.clone()),
                None => refused(self.pattern, format!("`{name}` is not an operand the match binds")),
            },
            Expr::Int(value) => Ok(format!("{value}i64")),
            Expr::Call(name, _) => refused(self.pattern, format!("`{name}(..)` where an operand is expected")),
        }
    }

    fn call(
        &self,
        prefix: &str,
        call: &Call,
    ) -> Result<String, String> {
        let args: Vec<String> = call.args.iter().map(|one| self.argument(one)).collect::<Result<_, _>>()?;
        let lead = if prefix == "hook" { "m, out" } else { "m" };
        let args = std::iter::once(lead.to_owned()).chain(args).collect::<Vec<_>>().join(", ");
        Ok(format!("self.{prefix}_{}({args})", call.name))
    }

    /// A LIR operand: its Rust and the kinds it may be.
    fn operand(
        &self,
        expr: &Expr,
    ) -> Result<(String, String), String> {
        let (name, args) = match expr {
            Expr::Name(name) => {
                if let Some((_, kinds)) = self.lets.iter().find(|(one, _)| one == name) {
                    return Ok((format!("{name}.clone()"), kinds.clone()));
                }
                if self.bound.iter().any(|(one, _)| one == name) {
                    return refused(
                        self.pattern,
                        format!("`{name}` is a MIR operand; say held({name}) or source({name})"),
                    );
                }
                (name.as_str(), &[][..])
            }
            Expr::Call(name, args) => (name.as_str(), &args[..]),
            Expr::Int(value) => {
                return refused(self.pattern, format!("`{value}` is no LIR operand; say imm({value}, width)"));
            }
        };
        let Some(&(_, kinds, arity)) = CONSTRUCTORS.iter().find(|(one, _, _)| *one == name) else {
            return refused(self.pattern, format!("no operand constructor `{name}`"));
        };
        if args.len() != arity {
            return refused(self.pattern, format!("`{name}` takes {arity} arguments, not {}", args.len()));
        }
        let args: Vec<String> = args.iter().map(|one| self.argument(one)).collect::<Result<_, _>>()?;
        let args = std::iter::once("m, out".to_owned()).chain(args).collect::<Vec<_>>().join(", ");
        Ok((format!("self.op_{name}({args})?"), kinds.to_owned()))
    }

    /// The forms of `name` whose operands take `dests` and `sources`.
    fn form(
        &self,
        name: &str,
        dests: &[String],
        sources: &[String],
    ) -> Result<&description::Form, String> {
        let forms: Vec<&description::Form> = self.forms.iter().filter(|one| one.name == name).collect();
        if forms.is_empty() {
            return refused(self.pattern, format!("x86.instr has no `{name}`"));
        }
        if let Some(other) = forms.iter().find(|one| one.operation != forms[0].operation) {
            return refused(self.pattern, format!("`{name}` is both {} and {}", forms[0].operation, other.operation));
        }
        let fits = |operand: &description::Operand, kinds: &String| kinds.chars().all(|kind| operand.allows(kind));
        forms
            .iter()
            .copied()
            .find(|form| {
                form.dests.len() == dests.len()
                    && form.sources.len() == sources.len()
                    && dests.iter().enumerate().all(|(index, kinds)| {
                        fits(form.operand(description::Side::Dest, index).expect("a checked destination"), kinds)
                    })
                    && sources.iter().enumerate().all(|(index, kinds)| {
                        fits(form.operand(description::Side::Source, index).expect("a checked source"), kinds)
                    })
            })
            .map_or_else(
                || refused(self.pattern, format!("x86.instr has no `{name}` taking {dests:?} <- {sources:?}")),
                Ok,
            )
    }
}

/// Whether the pattern holds: its structure, then its predicates -- or,
/// for a pattern that covers, that the cover phase chose it here.
fn condition(
    checker: &Checker,
    covering: bool,
) -> Result<String, String> {
    let pattern = checker.pattern;
    let mut terms = Vec::new();
    structure(&pattern.operands, &|index| format!("m.ops[{index}]"), true, &mut terms);
    if pattern.covers.is_empty() || covering {
        for call in &pattern.when {
            let rust = checker.call("is", call)?;
            terms.push(if call.negated { format!("!{rust}") } else { rust });
        }
    } else {
        for name in &pattern.covers {
            terms.push(format!("self.covered_by({}, m.inst)", checker.argument(&Expr::Name(name.clone()))?));
        }
    }
    Ok(if terms.is_empty() { "true".into() } else { terms.join(" && ") })
}

/// The pattern's emitter.
fn body(checker: &mut Checker) -> Result<String, String> {
    let mut code = String::new();
    for step in &checker.pattern.body.clone() {
        match step {
            Step::Let(name, expr) => {
                let (rust, kinds) = checker.operand(expr)?;
                writeln!(code, "                let {name} = {rust};").unwrap();
                checker.lets.push((name.clone(), kinds));
            }
            Step::Emit { name, dests, sources, volatile } => {
                let mut locals = Vec::new();
                let mut kinds = (Vec::new(), Vec::new());
                for (side, exprs) in [(0, dests), (1, sources)] {
                    for (index, expr) in exprs.iter().enumerate() {
                        let (rust, kind) = checker.operand(expr)?;
                        let local = format!("{}{index}", if side == 0 { "d" } else { "s" });
                        writeln!(code, "                let {local} = {rust};").unwrap();
                        locals.push(local);
                        if side == 0 { kinds.0.push(kind) } else { kinds.1.push(kind) }
                    }
                }
                let form = checker.form(name, &kinds.0, &kinds.1)?;
                let operation = format!(
                    "{:?}",
                    llrm_lir::Operation::named(&form.operation).expect("x86.instr's operations are checked")
                );
                let (d, s) = locals.split_at(dests.len());
                writeln!(code, "                out.push(self.emitted(m, Operation::{operation}, {name:?}, vec![{}], vec![{}], {volatile}));", d.join(", "), s.join(", ")).unwrap();
            }
            Step::Chain { first, rest, left, right } => {
                for name in [first, rest] {
                    checker.form(name, &["r".to_owned()], &["r".to_owned(), "ri".to_owned()])?;
                }
                let (left, right) = (checker.argument(left)?, checker.argument(right)?);
                writeln!(code, "                self.wide_chain(m, out, {first:?}, {rest:?}, {left}, {right})?;")
                    .unwrap();
            }
            Step::Hook(call) => writeln!(code, "                {}?;", checker.call("hook", call)?).unwrap(),
            Step::Refuse(Some(message)) => writeln!(code, "                return refuse({message:?});").unwrap(),
            Step::Refuse(None) => {
                writeln!(code, "                return refuse(self.function.instruction(m.inst).opcode.mnemonic());")
                    .unwrap()
            }
            Step::Nothing => {}
        }
    }
    Ok(code)
}

/// The selector of the target `name` (`x86-m16`): its tables and a `SELECTOR`
/// that holds them with the methods the patterns became, those named for it.
pub fn generate(
    forms_text: &str,
    patterns_text: &str,
    name: &str,
) -> Result<Generated, String> {
    let ident = name.replace('-', "_");
    let forms = description::parse(forms_text)?;
    let pinned = description::pinned(forms_text)?;
    if forms.iter().filter(|form| !form.fixed.is_empty()).ne(pinned.iter()) {
        return Err("the fixed-operand reader disagrees with the instruction description".to_owned());
    }
    if forms.iter().filter_map(|form| form.code_name(None)).any(|code| code.contains("{w}")) {
        return Err("an instruction code name kept its width placeholder".to_owned());
    }
    let patterns = parse::parse(patterns_text)?;
    let automaton = automaton::build(&patterns.patterns);
    let mut code = String::new();
    writeln!(code, "// @generated by build.rs from {name}'s patterns.isel and x86.instr.\n").unwrap();
    writeln!(code, "const _: () = assert!(crate::backend::isel::matcher::OPERANDS == {});\n", parse::OPERANDS).unwrap();
    writeln!(code, "pub(super) const OPCODES: [&str; {}] = {:?};", parse::OPCODES.len(), parse::OPCODES).unwrap();
    writeln!(code, "pub(super) const TYPES: [&str; {}] = {:?};", parse::TYPES.len(), parse::TYPES).unwrap();
    writeln!(code, "pub(super) const KINDS: [&str; {}] = {:?};", parse::KINDS.len(), parse::KINDS).unwrap();
    writeln!(
        code,
        "pub(super) const COMMUTATIVE: [&str; {}] = {:?};",
        patterns.commutative.len(),
        patterns.commutative
    )
    .unwrap();
    writeln!(code, "pub(super) const ROOT: Option<usize> = {:?};", automaton.root).unwrap();
    writeln!(code, "pub(super) static STATES: [State; {}] = [", automaton.states.len()).unwrap();
    for state in &automaton.states {
        match state {
            State::Test { feature, edges, default } => {
                writeln!(code, "    State::Test {{ feature: {feature}, edges: &{edges:?}, default: {default:?} }},")
                    .unwrap();
            }
            State::Leaf(patterns) => writeln!(code, "    State::Leaf(&{patterns:?}),").unwrap(),
        }
    }
    writeln!(code, "];").unwrap();
    let groups: Vec<Option<usize>> = {
        let mut seen: Vec<&str> = Vec::new();
        patterns
            .patterns
            .iter()
            .map(|one| {
                one.group
                    .as_deref()
                    .map(
                        |group| {
                            if !seen.contains(&group) {
                                seen.push(group);
                            }
                            seen.iter().position(|&known| known == group).expect("seen")
                        },
                    )
            })
            .collect()
    };
    writeln!(code, "pub(super) static GROUPS: [Option<usize>; {}] = {groups:?};", groups.len()).unwrap();
    let covering: Vec<bool> = patterns.patterns.iter().map(|one| !one.covers.is_empty()).collect();
    writeln!(code, "pub(super) static COVERS: [bool; {}] = {covering:?};", covering.len()).unwrap();
    let (mut holds, mut covers, mut costs, mut emits) = (String::new(), String::new(), String::new(), String::new());
    for (index, pattern) in patterns.patterns.iter().enumerate() {
        let mut checker = Checker { pattern, forms: &forms, bound: bindings(pattern), lets: Vec::new() };
        writeln!(holds, "            {index} => {},", condition(&checker, false)?).unwrap();
        if !pattern.covers.is_empty() {
            let marks: Vec<String> = pattern
                .covers
                .iter()
                .map(|name| {
                    checker.argument(&Expr::Name(name.clone())).map(|rust| format!("self.cover({rust}, m.inst);"))
                })
                .collect::<Result<_, _>>()?;
            writeln!(covers, "            {index} => {{\n                if !({}) {{\n                    return false;\n                }}\n                {}\n            }}", condition(&checker, true)?, marks.join(" ")).unwrap();
        }
        let emit = body(&mut checker)?;
        if pattern.group.is_some() {
            let Some(call) = &pattern.cost else { return refused(pattern, "a group member has no cost") };
            writeln!(costs, "            {index} => {},", checker.call("cost", call)?).unwrap();
        }
        writeln!(emits, "            {index} => {{ // {}\n{emit}            }}", pattern.name).unwrap();
    }
    writeln!(
        code,
        "
impl Selector<'_, '_, '_> {{
    pub(super) fn {ident}_holds(&mut self, pattern: usize, m: &Match) -> bool {{
        match pattern {{
{holds}            _ => unreachable!(\"no pattern {{pattern}}\"),
        }}
    }}

    /// The cover phase: whether a covering pattern holds here, and if so
    /// the instructions it covers marked.
    pub(super) fn {ident}_covers(&mut self, pattern: usize, m: &Match) -> bool {{
        match pattern {{
{covers}            _ => unreachable!(\"pattern {{pattern}} covers nothing\"),
        }}
        true
    }}

    pub(super) fn {ident}_cost(&mut self, pattern: usize, m: &Match) -> Result<i64, Unselected> {{
        match pattern {{
{costs}            _ => unreachable!(\"pattern {{pattern}} is in no group\"),
        }}
    }}

    pub(super) fn {ident}_emit(&mut self, pattern: usize, m: &Match, out: &mut Vec<Arc<Insn>>) -> Result<(), Unselected> {{
        match pattern {{
{emits}            _ => unreachable!(\"no pattern {{pattern}}\"),
        }}
        Ok(())
    }}
}}

pub static SELECTOR: Compiled = Compiled {{
    name: {name:?},
    opcodes: &OPCODES,
    types: &TYPES,
    kinds: &KINDS,
    commutative: &COMMUTATIVE,
    root: ROOT,
    states: &STATES,
    groups: &GROUPS,
    covers: &COVERS,
    holds: |selector, pattern, m| selector.{ident}_holds(pattern, m),
    covers_here: |selector, pattern, m| selector.{ident}_covers(pattern, m),
    cost: |selector, pattern, m| selector.{ident}_cost(pattern, m),
    emit: |selector, pattern, m, out| selector.{ident}_emit(pattern, m, out),
    rules: &crate::backend::peep::targets::{ident}::RULES,
}};"
    )
    .unwrap();
    Ok(Generated { code, automaton })
}

#[cfg(test)]
mod tests {
    use super::automaton::State;
    use super::generate;

    const FORMS: &str =
        "add binary rm/^0,rmi 16 alu_rr - - - -/- Add_rm{w}_r{w}\nneg unary rm/^0 16 alu_rr - - - -/- Neg_rm{w}\n";

    fn refused(patterns: &str) -> String {
        generate(FORMS, patterns, "test").err().expect("refused")
    }

    #[test]
    fn a_malformed_pattern_is_refused_with_its_line_and_why() {
        let unknown = "pattern p\n  match add(a, b)\n  emit sub result <- held(a), source(b)\nend\n";
        assert_eq!(refused(unknown), "patterns.isel:1: pattern `p`: x86.instr has no `sub`");
        let shape = "pattern p\n  match add(a, b)\n  emit add result <- held(a)\nend\n";
        assert_eq!(refused(shape), "patterns.isel:1: pattern `p`: x86.instr has no `add` taking [\"r\"] <- [\"r\"]");
        let unbound = "pattern p\n  match add(a, b)\n  emit neg result <- held(c)\nend\n";
        assert_eq!(refused(unbound), "patterns.isel:1: pattern `p`: `c` is not an operand the match binds");
        assert_eq!(refused("pattern p\n  match plus(a, b)\n  nothing\nend\n"), "patterns.isel:2: no MIR opcode `plus`");
        let cover = "pattern p\n  match add(a, b)\n  cover a\n  nothing\nend\n";
        assert_eq!(refused(cover), "patterns.isel:1: pattern `p` covers `a`, which is no instruction its match nests");
        assert_eq!(
            refused("pattern p\n  match add(a, b)\nend\n"),
            "patterns.isel:1: pattern `p` selects nothing; say `nothing` if it means to"
        );
    }

    /// Patterns that share an opcode and a type test them once: one state
    /// each, however many patterns go through it.
    #[test]
    fn two_patterns_sharing_a_prefix_share_its_states() {
        let patterns = "pattern any\n  match add.i16(a, b)\n  nothing\nend\npattern zero\n  match add.i16(a, #0)\n  nothing\nend\n";
        let generated = generate(FORMS, patterns, "test").expect("generates");
        let states = &generated.automaton.states;
        let testing = |feature: usize| {
            states.iter().filter(|one| matches!(one, State::Test { feature: f, .. } if *f == feature)).count()
        };
        assert_eq!((testing(0), testing(1)), (1, 1));
        assert_eq!(states.len(), 6, "{states:?}");
        assert!(states.contains(&State::Leaf(vec![0, 1])) && states.contains(&State::Leaf(vec![0])));
    }

    /// A second target's selector, generated beside the first, defined the same
    /// methods under the same names: they were `pattern_holds` on one
    /// `Selector`.
    #[test]
    fn each_targets_selector_has_methods_named_for_it() {
        let patterns = "pattern any\n  match add.i16(a, b)\n  nothing\nend\n";
        let (first, second) =
            (generate(FORMS, patterns, "x86-m16").unwrap().code, generate(FORMS, patterns, "x86-m32").unwrap().code);
        assert!(first.contains("fn x86_m16_holds(") && !first.contains("x86_m32"));
        assert!(second.contains("fn x86_m32_holds(") && second.contains("name: \"x86-m32\""));
    }
}

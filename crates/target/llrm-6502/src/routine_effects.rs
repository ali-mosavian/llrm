//! What a call of a runtime routine does to the `rcN` cells: the ones it reads
//! (its arguments) and the ones it always writes (its results).

use std::collections::HashMap;

use crate::routines::{cells_of, runtime};

/// What a call of a runtime routine does to the `rcN` cells (`N < 64`), as
/// masks with bit `N`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellEffects {
    /// Cells some path reads before writing them: the call's arguments.
    pub reads: u64,
    /// Cells every path writes before it returns: the call's results.
    pub writes: u64,
}

/// The cells a call of `name` reads and the cells it always writes, from the
/// routine's own text and its callees' (a backward and a forward dataflow over
/// its instructions). `None` when that is not known: not a runtime routine, an
/// index with no `[rcA..rcB]` span, a local `jsr`, a fall-through.
///
/// The pin's analogue is the call's operand list: only the registers it lists
/// are read, and a register it defines is not live across it
/// (`MachineInstr::implicit_operands`, `LivePhysRegs`).
pub fn cell_effects(name: &str) -> Option<CellEffects> {
    type Answers = HashMap<String, Option<CellEffects>>;
    static ANSWERS: std::sync::Mutex<Option<Answers>> = std::sync::Mutex::new(None);
    let cached =
        ANSWERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get_or_insert_default().get(name).copied();
    if let Some(answer) = cached {
        return answer;
    }
    let answer = effects_of(name, &mut Vec::new());
    ANSWERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get_or_insert_default()
        .insert(name.to_owned(), answer);
    answer
}

/// One instruction: what it reads and always writes, and where it goes next.
struct Node {
    reads: u64,
    writes: u64,
    next: Vec<usize>,
}

fn effects_of(
    name: &str,
    stack: &mut Vec<String>,
) -> Option<CellEffects> {
    if stack.iter().any(|open| open == name) {
        return None;
    }
    let text = *runtime().routines.get(name)?;
    stack.push(name.to_owned());
    let result = analyse(text, stack);
    stack.pop();
    result
}

fn analyse(
    text: &str,
    stack: &mut Vec<String>,
) -> Option<CellEffects> {
    let runtime = runtime();
    let bit = |n: u8| (n < 64).then(|| 1u64 << n);
    // the instructions and the labels
    let mut lines: Vec<(&str, String, &str)> = Vec::new();
    // the line with its comment: an index names its cells there
    let mut labels: HashMap<&str, usize> = HashMap::new();
    for line in text.lines() {
        let code = line.split(';').next().unwrap_or("").trim();
        if code.is_empty() || code.starts_with(".export") || code.starts_with(".import") {
            continue;
        }
        if code.starts_with('.') {
            return None;
        }
        if let Some(label) = code.strip_suffix(':') {
            labels.insert(label, lines.len());
            continue;
        }
        let (op, arg) = code.split_once(char::is_whitespace).unwrap_or((code, ""));
        lines.push((line, op.to_ascii_lowercase(), arg.trim()));
    }
    let count = lines.len();
    if labels.values().any(|&at| at >= count) {
        return None;
    }
    let mut nodes: Vec<Node> = Vec::with_capacity(count);
    // a jump out of the routine is a node of the callee's effects with no
    // successor, placed after the routine's own
    let mut tails: Vec<Node> = Vec::new();
    let tail_of = |routine: &str, stack: &mut Vec<String>, tails: &mut Vec<Node>| -> Option<usize> {
        let fx = effects_of(routine, stack)?;
        tails.push(Node { reads: fx.reads, writes: fx.writes, next: Vec::new() });
        Some(count + tails.len() - 1)
    };
    for (at, (line, op, arg)) in lines.iter().enumerate() {
        let following = (at + 1 < count).then_some(at + 1);
        let mut node = Node { reads: 0, writes: 0, next: Vec::new() };
        match op.as_str() {
            "rts" => {}
            "jmp" if labels.contains_key(arg) => node.next.push(labels[arg]),
            "jmp" if runtime.routines.contains_key(arg) => node.next.push(tail_of(arg, stack, &mut tails)?),
            "jsr" if runtime.routines.contains_key(arg) => {
                let fx = effects_of(arg, stack)?;
                (node.reads, node.writes) = (fx.reads, fx.writes);
                node.next.push(following?);
            }
            "jmp" | "jsr" | "rti" | "brk" => return None,
            "bcc" | "bcs" | "beq" | "bne" | "bmi" | "bpl" | "bvc" | "bvs" | "bra" => {
                if let Some(to) = labels.get(arg) {
                    node.next.push(*to);
                } else if runtime.routines.contains_key(arg) {
                    node.next.push(tail_of(arg, stack, &mut tails)?);
                } else {
                    return None;
                }
                if op != "bra" {
                    node.next.push(following?);
                }
            }
            _ => {
                let macro_cells = runtime.macros.get(op.as_str());
                let mut named = match macro_cells {
                    Some(cells) => cells.clone()?,
                    None => Vec::new(),
                };
                named.extend(cells_of(line)?);
                let store = matches!(op.as_str(), "sta" | "stx" | "sty" | "stz");
                let direct = arg.strip_prefix("rc").is_some_and(|digits| digits.bytes().all(|b| b.is_ascii_digit()));
                for n in &named {
                    let mask = bit(*n)?;
                    if store && direct && macro_cells.is_none() {
                        node.writes |= mask;
                    } else {
                        node.reads |= mask;
                        // a pointer is a pair
                        if arg.starts_with('(') {
                            node.reads |= bit(n + 1)?;
                        }
                    }
                }
                node.next.push(following?);
            }
        }
        nodes.push(node);
    }
    nodes.extend(tails);
    if count == 0 {
        return None;
    }
    // backward: the cells read before they are written, from each instruction
    let mut live = vec![0u64; nodes.len()];
    loop {
        let mut changed = false;
        for at in (0..nodes.len()).rev() {
            let out = nodes[at].next.iter().fold(0u64, |acc, &to| acc | live[to]);
            let now = nodes[at].reads | (out & !nodes[at].writes);
            if now != live[at] {
                live[at] = now;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // forward: the cells written on every path to each instruction
    let mut before = vec![u64::MAX; nodes.len()];
    before[0] = 0;
    loop {
        let mut changed = false;
        for at in 0..nodes.len() {
            let after = before[at] | nodes[at].writes;
            for &to in &nodes[at].next {
                let meet = before[to] & after;
                if to != 0 && meet != before[to] {
                    before[to] = meet;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let writes = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.next.is_empty())
        .fold(u64::MAX, |acc, (at, node)| acc & (before[at] | node.writes));
    Some(CellEffects { reads: live[0], writes })
}

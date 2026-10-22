//! The pin's `-print-pipeline-passes` against llrm's slot registry: every slot
//! of the pin's pipeline at -O1, -O2, -O3 and -Os (tests/fixtures/pin-pipeline,
//! made by tools/pairs/pin_pipeline.py) is matched, in order, to the slots
//! `llvm_order` builds, or is listed as a gap. `LLRM_PIN_SLOT_TABLE=FILE`
//! writes the table.

use crate::parity::{self, Entry, Phase};
use crate::pipeline::{self, Applied, Options};

const FIXTURES: [(&str, &str); 4] = [
    ("-O1", include_str!("../../../../tests/fixtures/pin-pipeline/O1.txt")),
    ("-O2", include_str!("../../../../tests/fixtures/pin-pipeline/O2.txt")),
    ("-O3", include_str!("../../../../tests/fixtures/pin-pipeline/O3.txt")),
    ("-Os", include_str!("../../../../tests/fixtures/pin-pipeline/Os.txt")),
];

/// One pass of the pin's printed pipeline.
#[derive(Clone, Debug)]
struct PinSlot {
    /// The phase of llrm's registry the nest it stands in is: the first
    /// top-level function group is Early, the second Cleanup, the function
    /// groups of the CGSCC Simplify, those after it Late, the rest Module.
    phase: Phase,
    /// Its place in the nest, as the pin prints it (`cgscc/devirt<4>/function`).
    path: String,
    /// The pass without its parameters.
    name: String,
    text: String,
}

/// `pass<params>` as (`pass`, whole).
fn split_params(text: &str) -> (&str, &str) {
    (text.split('<').next().unwrap_or(text), text)
}

/// The passes of one printed pipeline, in order, with their nest.
fn parse(fixture: &str) -> Vec<PinSlot> {
    let line = fixture.lines().rev().find(|one| !one.starts_with(';')).expect("a pipeline line");
    let mut tokens = Vec::new();
    let (mut angle, mut buffer) = (0_i32, String::new());
    for ch in line.chars() {
        match ch {
            '<' => angle += 1,
            '>' => angle -= 1,
            _ => {}
        }
        if angle == 0 && matches!(ch, ',' | '(' | ')') {
            tokens.push((std::mem::take(&mut buffer), ch));
        } else {
            buffer.push(ch);
        }
    }
    tokens.push((buffer, ' '));
    let mut path: Vec<String> = Vec::new();
    let mut out = Vec::new();
    let mut top_functions = 0;
    let mut seen_cgscc = false;
    // The top-level function groups, by order, once their analyses-only
    // members are left out.
    let mut group_phase: Vec<Phase> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let (name, ch) = tokens[index].clone();
        index += 1;
        if ch == '(' {
            if path.is_empty() && split_params(&name).0 == "function" {
                // Phase of a top-level function group: decided when its first
                // real pass shows; a group with only analyses is Module.
                top_functions += 1;
                group_phase.push(Phase::Module);
            }
            if path.is_empty() && name == "cgscc" {
                seen_cgscc = true;
            }
            path.push(name);
            continue;
        }
        if !name.is_empty() {
            let (bare, whole) = split_params(&name);
            let noise = matches!(bare, "require" | "invalidate" | "verify" | "print");
            if !noise {
                let in_cgscc_function = path.first().is_some_and(|one| one == "cgscc")
                    && path.iter().any(|one| split_params(one).0 == "function");
                let phase = if in_cgscc_function {
                    Phase::Simplify
                } else if path.first().is_some_and(|one| split_params(one).0 == "function") {
                    let at = top_functions - 1;
                    if group_phase[at] == Phase::Module {
                        let real_groups = group_phase.iter().filter(|one| **one != Phase::Module).count();
                        group_phase[at] = if seen_cgscc {
                            Phase::Late
                        } else if real_groups == 0 {
                            Phase::Early
                        } else {
                            Phase::Cleanup
                        };
                    }
                    group_phase[at]
                } else {
                    Phase::Module
                };
                out.push(PinSlot { phase, path: path.join("/"), name: bare.to_owned(), text: whole.to_owned() });
            }
        }
        if ch == ')' {
            path.pop();
        }
    }
    out
}

/// The pass names an entry's `llvm` field names (`simplifycfg, jump-threading`
/// is two; `licm<allowspeculation>` is `licm`).
fn llvm_names(entry: &Entry) -> Vec<String> {
    entry
        .llvm
        .split(',')
        .filter_map(|part| part.trim().split_whitespace().next())
        .map(|word| word.split('<').next().unwrap_or(word).to_owned())
        .collect()
}

/// The line a slot stands at (`combine@1242` is 1242): the slots of one line
/// are one slot of the pin's.
fn group_of(slot: &str) -> String {
    slot.split_once('@').map_or_else(|| slot.to_owned(), |split| split.1.to_owned())
}

fn applied(level: &str) -> Applied {
    let mut options = match level {
        "-O1" => Options::basic(),
        "-O3" => Options::speed(),
        "-Os" => Options::size(),
        _ => Options::standard(),
    };
    options.llvm_order = true;
    Applied { options, ..Applied::default() }
}

/// The registry entries the pipeline of `level` builds in `phase`, grouped by
/// the line they stand at, in the order built.
fn built_groups(level: &str, phase: Phase) -> Vec<(String, Vec<Entry>)> {
    let mut groups: Vec<(String, Vec<Entry>)> = Vec::new();
    for slot in pipeline::llvm_order(&applied(level), phase).iter().map(|one| one.name()) {
        let Some(entry) = parity::REGISTRY.iter().find(|one| one.slot == slot && one.phase == phase) else { continue };
        let group = group_of(slot);
        match groups.iter_mut().find(|(name, _)| *name == group) {
            Some((_, entries)) => entries.push(*entry),
            None => groups.push((group, vec![*entry])),
        }
    }
    groups
}

#[derive(Debug)]
struct Row {
    level: &'static str,
    pin: PinSlot,
    /// The group of ours it stands against, or why none does.
    ours: String,
    state: &'static str,
}

fn rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for (level, fixture) in FIXTURES {
        let pin = parse(fixture);
        for phase in [Phase::Early, Phase::Cleanup, Phase::Simplify, Phase::Late, Phase::Module] {
            let groups = built_groups(level, phase);
            let mut taken = vec![false; groups.len()];
            let mut cursor = 0;
            for slot in pin.iter().filter(|one| one.phase == phase) {
                let wants = |(_, entries): &(String, Vec<Entry>)| {
                    entries.iter().any(|entry| llvm_names(entry).iter().any(|name| *name == slot.name))
                };
                let forward = (cursor..groups.len()).find(|&at| !taken[at] && wants(&groups[at]));
                let (at, state) = match forward {
                    Some(at) => (Some(at), "match"),
                    None => (groups.iter().enumerate().position(|(at, group)| !taken[at] && wants(group)), "inverted"),
                };
                match at {
                    Some(at) => {
                        taken[at] = true;
                        if state == "match" {
                            cursor = at + 1;
                        }
                        let kinds: Vec<String> = groups[at].1.iter().map(|one| format!("{}:{:?}", one.slot, one.kind)).collect();
                        rows.push(Row { level, pin: slot.clone(), ours: kinds.join(" + "), state });
                    }
                    None => rows.push(Row { level, pin: slot.clone(), ours: String::new(), state: "none" }),
                }
            }
            for (at, (group, entries)) in groups.iter().enumerate() {
                if !taken[at] {
                    let names: Vec<String> = entries.iter().map(|one| one.slot.to_owned()).collect();
                    let pin_name = PinSlot { phase, path: String::new(), name: String::new(), text: String::new() };
                    rows.push(Row { level, pin: pin_name, ours: format!("{group}: {}", names.join(" + ")), state: "ours-only" });
                }
            }
        }
    }
    rows
}

#[test]
fn the_pin_slot_table_is_written_on_request() {
    let rows = rows();
    assert!(rows.iter().any(|one| one.state == "match"), "nothing matched: the fixtures or the registry moved");
    let Ok(path) = std::env::var("LLRM_PIN_SLOT_TABLE") else { return };
    let mut text = String::from("| level | phase | pin slot | nest | state | ours |\n|---|---|---|---|---|---|\n");
    for row in &rows {
        text.push_str(&format!(
            "| {} | {:?} | {} | {} | {} | {} |\n",
            row.level, row.pin.phase, row.pin.text, row.pin.path, row.state, row.ours
        ));
    }
    std::fs::write(path, text).expect("table written");
}

//! Microfixture oracle harness for graph-backend semantic verification.
//!
//! Each fixture under `fixtures/buildprs/micro/<name>/` stores a tiny `.prs`
//! grammar, minimal `peropcod.txt`, and captured DOS `prsstate.asm` outputs for
//! O0-O2. Tests compare generated and golden tables with a semantic interpreter.

use std::path::{Path, PathBuf};

use crate::buildprs_encoder::{BranchTarget, EncodeConfig, StateDecoder, StateEntry, ND_BRANCH};
use crate::buildprs_generator::{
    generate_tables_from_graph, generate_tables_from_prsstate, parse_opcode_equates_from_peropcod,
    GenerateError, GeneratedParserTables, ENCODE1BYTE_QBASIC_11,
};
use crate::buildprs_grammar::{parse_grammar, parse_grammar_file, GrammarFile};
use crate::buildprs_graph::OptLevel;

pub const MICRO_FIXTURE_ROOT: &str = "../../fixtures/buildprs/micro";

/// Maximum abstract input length when exhaustively probing microfixture semantics.
pub const MICRO_SEMANTIC_MAX_INPUT_LEN: usize = 4;

/// Plain grammar constructs exercised before `<Cg...>` hint families.
pub const PLAIN_MICRO_FIXTURES: &[&str] = &[
    "emit_only",
    "sequence",
    "alternative",
    "optional",
    "repeat",
    "mark_emit",
    "empty",
];

/// Priority `<Cg...>` hint families from the microfixture parity plan.
pub const CG_HINT_MICRO_FIXTURES: &[&str] =
    &["cg_1or2_args", "cg_0or1_args", "cg_stmt_cnt", "cg_call"];

/// Focused reproductions of full `qbasbnf` parity gaps. These are DOS-captured
/// but not part of the always-green parity set until the related algorithm is fixed.
pub const FULL_GAP_MICRO_FIXTURES: &[&str] = &[
    "shared_optcomma_exp",
    "case_repeat_alt",
    "nested_optional_mark_tail",
    "common_shared_suffix",
    "end_keyword_default",
    "exit_grouped_keywords_tail",
    "repeat_action_tail",
    "for_step_default_tail",
    "get_record_alternatives",
    "input_prompt_alternatives",
    "line_box_flags_tail",
    "lock_range_tail",
    "on_goto_gosub_tail",
    "open_mode_access_lock_tail",
    "resume_optional_lit0_label_next",
    "common_shared_action_tail",
    "event_switch_shared_tail",
    "call_calls_repeated_args",
    "early_o1_statement_block",
    "early_o1_declare_def_block",
    "expression_tail_o1_block",
    "print_tail_o1_block",
    "declare_expression_tail_o1_block",
    "post_declare_o1_block",
];

pub const DOS_O2_HANG_MICRO_FIXTURES: &[&str] = &["open_mode_access_lock_tail"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicroFixture {
    pub name: String,
    pub grammar: GrammarFile,
    pub peropcod: String,
    pub golden_o0: Option<String>,
    pub golden_o1: Option<String>,
    pub golden_o2: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicroFixtureDiff {
    pub fixture: String,
    pub opt_level: OptLevel,
    pub generated_len: usize,
    pub golden_len: usize,
    pub byte_diff_ranges: Vec<(usize, usize)>,
    pub decoded_entry_diffs: Vec<DecodedEntryDiff>,
    pub state_bytes_match: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedEntryDiff {
    pub entry_index: usize,
    pub generated_offset: usize,
    pub golden_offset: usize,
    pub generated_entry: String,
    pub golden_entry: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTrace {
    pub outcome: SemanticOutcome,
    pub consumed: usize,
    pub effects: Vec<SemanticEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticOutcome {
    Accept,
    Reject,
    Invalid,
    StepLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticEffect {
    Mark(u8),
    Emit(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiff {
    pub generated_start_offset: usize,
    pub golden_start_offset: usize,
    pub input: Vec<u16>,
    pub generated: SemanticTrace,
    pub golden: SemanticTrace,
}

impl std::fmt::Display for SemanticDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "start generated={} golden={} input={:?}",
            self.generated_start_offset, self.golden_start_offset, self.input
        )?;
        writeln!(f, "  generated: {:?}", self.generated)?;
        writeln!(f, "  golden:    {:?}", self.golden)
    }
}

impl std::fmt::Display for MicroFixtureDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "fixture={} opt={:?} generated_len={} golden_len={} byte_ranges={} entry_diffs={} match={}",
            self.fixture,
            self.opt_level,
            self.generated_len,
            self.golden_len,
            self.byte_diff_ranges.len(),
            self.decoded_entry_diffs.len(),
            self.state_bytes_match
        )?;
        for diff in self.decoded_entry_diffs.iter().take(20) {
            writeln!(
                f,
                "  entry {:03}: generated@{} {} | golden@{} {}",
                diff.entry_index,
                diff.generated_offset,
                diff.generated_entry,
                diff.golden_offset,
                diff.golden_entry
            )?;
        }
        Ok(())
    }
}

pub fn micro_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MICRO_FIXTURE_ROOT)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join(MICRO_FIXTURE_ROOT))
}

pub fn load_micro_fixture(name: &str) -> Result<MicroFixture, GenerateError> {
    let root = micro_fixture_root().join(name);
    let grammar = parse_grammar_file(root.join("grammar.prs"))
        .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
    let peropcod = std::fs::read_to_string(root.join("peropcod.txt"))
        .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
    let golden_o0 = read_optional_dos_prsstate(&root.join("o0/prsstate.asm"))?;
    let golden_o1 = read_optional_dos_prsstate(&root.join("o1/prsstate.asm"))?;
    let golden_o2 = read_optional_dos_prsstate(&root.join("o2/prsstate.asm"))?;
    Ok(MicroFixture {
        name: name.to_string(),
        grammar,
        peropcod,
        golden_o0,
        golden_o1,
        golden_o2,
    })
}

pub fn load_micro_fixture_from_strings(
    name: &str,
    grammar_source: &str,
    peropcod: &str,
    golden_o0: Option<&str>,
    golden_o1: Option<&str>,
    golden_o2: Option<&str>,
) -> Result<MicroFixture, GenerateError> {
    let grammar = parse_grammar(grammar_source)
        .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?;
    Ok(MicroFixture {
        name: name.to_string(),
        grammar,
        peropcod: peropcod.to_string(),
        golden_o0: golden_o0.map(str::to_string),
        golden_o1: golden_o1.map(str::to_string),
        golden_o2: golden_o2.map(str::to_string),
    })
}

pub fn generate_graph_tables_for_fixture(
    fixture: &MicroFixture,
    opt_level: OptLevel,
) -> Result<GeneratedParserTables, GenerateError> {
    generate_tables_from_graph(&fixture.grammar, &fixture.peropcod, opt_level)
}

pub fn golden_tables_for_fixture(
    fixture: &MicroFixture,
    opt_level: OptLevel,
) -> Result<Option<GeneratedParserTables>, GenerateError> {
    let golden_source = match opt_level {
        OptLevel::O0 => fixture.golden_o0.as_deref(),
        OptLevel::O1 => fixture.golden_o1.as_deref(),
        OptLevel::O2 => fixture.golden_o2.as_deref(),
    };
    let Some(golden_source) = golden_source else {
        return Ok(None);
    };
    let opcodes = parse_opcode_equates_from_peropcod(&fixture.peropcod);
    Ok(Some(
        generate_tables_from_prsstate(golden_source, &opcodes)
            .map_err(|error| GenerateError::ArtifactParse(error.to_string()))?,
    ))
}

pub fn compare_micro_fixture(
    fixture: &MicroFixture,
    opt_level: OptLevel,
) -> Result<MicroFixtureDiff, GenerateError> {
    let generated = generate_graph_tables_for_fixture(fixture, opt_level)?;
    let golden = golden_tables_for_fixture(fixture, opt_level)?.ok_or_else(|| {
        GenerateError::ArtifactParse(format!("missing golden for {:?}", opt_level))
    })?;
    Ok(compare_generated_to_golden(
        &fixture.name,
        opt_level,
        &generated,
        &golden,
    ))
}

pub fn compare_generated_to_golden(
    fixture_name: &str,
    opt_level: OptLevel,
    generated: &GeneratedParserTables,
    golden: &GeneratedParserTables,
) -> MicroFixtureDiff {
    let byte_diff_ranges = byte_diff_ranges(&generated.state, &golden.state);
    let generated_entries = decode_state_entries(&generated.state);
    let golden_entries = decode_state_entries(&golden.state);
    let compared = generated_entries.len().min(golden_entries.len());
    let mut decoded_entry_diffs = Vec::new();
    for (entry_index, ((generated_offset, generated_entry), (golden_offset, golden_entry))) in
        generated_entries
            .iter()
            .zip(&golden_entries)
            .take(compared)
            .enumerate()
    {
        if generated_entry != golden_entry {
            decoded_entry_diffs.push(DecodedEntryDiff {
                entry_index,
                generated_offset: *generated_offset,
                golden_offset: *golden_offset,
                generated_entry: generated_entry.clone(),
                golden_entry: golden_entry.clone(),
            });
        }
    }
    if generated_entries.len() != golden_entries.len() {
        decoded_entry_diffs.push(DecodedEntryDiff {
            entry_index: compared,
            generated_offset: generated_entries
                .get(compared)
                .map_or(0, |(offset, _)| *offset),
            golden_offset: golden_entries
                .get(compared)
                .map_or(0, |(offset, _)| *offset),
            generated_entry: format!("count={}", generated_entries.len()),
            golden_entry: format!("count={}", golden_entries.len()),
        });
    }

    MicroFixtureDiff {
        fixture: fixture_name.to_string(),
        opt_level,
        generated_len: generated.state.len(),
        golden_len: golden.state.len(),
        byte_diff_ranges,
        decoded_entry_diffs,
        state_bytes_match: generated.state == golden.state,
    }
}

pub fn assert_micro_fixture_parity(
    fixture: &MicroFixture,
    opt_level: OptLevel,
) -> Result<(), String> {
    let diff = compare_micro_fixture(fixture, opt_level)
        .map_err(|error| format!("fixture {} {:?}: {error}", fixture.name, opt_level))?;
    if diff.state_bytes_match {
        return Ok(());
    }
    Err(diff.to_string())
}

pub fn compare_micro_fixture_semantics(
    fixture: &MicroFixture,
    opt_level: OptLevel,
    max_input_len: usize,
) -> Result<Option<SemanticDiff>, GenerateError> {
    let generated = generate_graph_tables_for_fixture(fixture, opt_level)?;
    let golden = golden_tables_for_fixture(fixture, opt_level)?.ok_or_else(|| {
        GenerateError::ArtifactParse(format!("missing golden for {:?}", opt_level))
    })?;
    Ok(first_semantic_diff(&generated, &golden, max_input_len))
}

pub fn assert_micro_fixture_semantics(
    fixture: &MicroFixture,
    opt_level: OptLevel,
) -> Result<(), String> {
    let diff = compare_micro_fixture_semantics(fixture, opt_level, MICRO_SEMANTIC_MAX_INPUT_LEN)
        .map_err(|error| format!("fixture {} {:?}: {error}", fixture.name, opt_level))?;
    let Some(diff) = diff else {
        return Ok(());
    };
    Err(format!(
        "fixture {} {:?} semantic mismatch:\n{diff}",
        fixture.name, opt_level
    ))
}

fn first_semantic_diff(
    generated: &GeneratedParserTables,
    golden: &GeneratedParserTables,
    max_input_len: usize,
) -> Option<SemanticDiff> {
    let alphabet = semantic_alphabet(generated, golden);
    let starts = semantic_start_offsets(generated, golden);
    let inputs = bounded_inputs(&alphabet, max_input_len);
    first_semantic_diff_for_inputs(generated, golden, &starts, &inputs)
}

fn first_semantic_diff_for_inputs(
    generated: &GeneratedParserTables,
    golden: &GeneratedParserTables,
    starts: &[(usize, usize)],
    inputs: &[Vec<u16>],
) -> Option<SemanticDiff> {
    for (generated_start_offset, golden_start_offset) in starts {
        for input in inputs {
            let generated_trace = run_semantic_trace(generated, *generated_start_offset, input);
            let golden_trace = run_semantic_trace(golden, *golden_start_offset, input);
            if generated_trace != golden_trace {
                return Some(SemanticDiff {
                    generated_start_offset: *generated_start_offset,
                    golden_start_offset: *golden_start_offset,
                    input: input.clone(),
                    generated: generated_trace,
                    golden: golden_trace,
                });
            }
        }
    }
    None
}

#[cfg(test)]
fn sampled_inputs(alphabet: &[u16]) -> Vec<Vec<u16>> {
    let mut inputs = vec![Vec::new()];
    inputs.extend(alphabet.iter().map(|symbol| vec![*symbol]));
    inputs.extend(alphabet.windows(2).map(|symbols| symbols.to_vec()));
    for chunk in alphabet.chunks(8) {
        inputs.push(chunk.to_vec());
    }
    inputs
}

fn semantic_start_offsets(
    generated: &GeneratedParserTables,
    golden: &GeneratedParserTables,
) -> Vec<(usize, usize)> {
    let mut starts = vec![(0, 0)];
    starts.extend(
        generated
            .dispatch
            .int_nt_disp
            .iter()
            .zip(&golden.dispatch.int_nt_disp)
            .map(|(generated_offset, golden_offset)| {
                (usize::from(*generated_offset), usize::from(*golden_offset))
            }),
    );
    starts.sort_unstable();
    starts.dedup();
    starts
}

fn semantic_alphabet(
    generated: &GeneratedParserTables,
    golden: &GeneratedParserTables,
) -> Vec<u16> {
    let mut alphabet = Vec::new();
    collect_consuming_node_ids(generated, &mut alphabet);
    collect_consuming_node_ids(golden, &mut alphabet);
    alphabet.sort_unstable();
    alphabet.dedup();
    alphabet
}

fn collect_consuming_node_ids(tables: &GeneratedParserTables, alphabet: &mut Vec<u16>) {
    let config = semantic_encode_config();
    let consuming_base = u16::from(ND_BRANCH) + 1 + tables.dispatch.int_nt_disp.len() as u16;
    let mut decoder = StateDecoder::with_config(&tables.state, config);
    while let Ok(Some(entry)) = decoder.next() {
        if let StateEntry::Node { node_id, .. } = entry {
            if node_id >= consuming_base {
                alphabet.push(node_id);
            }
        }
    }
}

fn bounded_inputs(alphabet: &[u16], max_len: usize) -> Vec<Vec<u16>> {
    let mut inputs = Vec::new();
    let mut current = Vec::new();
    collect_bounded_inputs(alphabet, max_len, &mut current, &mut inputs);
    inputs
}

fn collect_bounded_inputs(
    alphabet: &[u16],
    remaining: usize,
    current: &mut Vec<u16>,
    inputs: &mut Vec<Vec<u16>>,
) {
    inputs.push(current.clone());
    if remaining == 0 {
        return;
    }
    for symbol in alphabet {
        current.push(*symbol);
        collect_bounded_inputs(alphabet, remaining - 1, current, inputs);
        current.pop();
    }
}

fn run_semantic_trace(
    tables: &GeneratedParserTables,
    start_offset: usize,
    input: &[u16],
) -> SemanticTrace {
    let mut budget = 2048;
    execute_state(tables, start_offset, input, 0, Vec::new(), &mut budget, 0)
}

fn execute_state(
    tables: &GeneratedParserTables,
    mut pc: usize,
    input: &[u16],
    mut consumed: usize,
    mut effects: Vec<SemanticEffect>,
    budget: &mut usize,
    depth: usize,
) -> SemanticTrace {
    if depth > 64 {
        return SemanticTrace {
            outcome: SemanticOutcome::StepLimit,
            consumed,
            effects,
        };
    }
    loop {
        if *budget == 0 {
            return SemanticTrace {
                outcome: SemanticOutcome::StepLimit,
                consumed,
                effects,
            };
        }
        *budget -= 1;

        let Some((entry, next_pc)) = decode_semantic_entry(&tables.state, pc) else {
            return SemanticTrace {
                outcome: SemanticOutcome::Invalid,
                consumed,
                effects,
            };
        };

        match entry {
            StateEntry::Accept => {
                return SemanticTrace {
                    outcome: SemanticOutcome::Accept,
                    consumed,
                    effects,
                };
            }
            StateEntry::Reject => {
                return SemanticTrace {
                    outcome: SemanticOutcome::Reject,
                    consumed,
                    effects,
                };
            }
            StateEntry::Mark(slot) => {
                effects.push(SemanticEffect::Mark(slot));
                pc = next_pc;
            }
            StateEntry::Emit(opcode) => {
                effects.push(SemanticEffect::Emit(opcode));
                pc = next_pc;
            }
            StateEntry::Branch(branch) => match branch_target_offset(branch) {
                Some(target) => pc = target,
                None => {
                    return SemanticTrace {
                        outcome: SemanticOutcome::Accept,
                        consumed,
                        effects,
                    };
                }
            },
            StateEntry::Node { node_id, branch } => {
                if let Some(root) = internal_root_offset(tables, node_id) {
                    let candidate = execute_state(
                        tables,
                        root,
                        input,
                        consumed,
                        effects.clone(),
                        budget,
                        depth + 1,
                    );
                    if candidate.outcome == SemanticOutcome::Accept {
                        consumed = candidate.consumed;
                        effects = candidate.effects;
                        match branch_target_offset(branch) {
                            Some(target) => pc = target,
                            None => {
                                return SemanticTrace {
                                    outcome: SemanticOutcome::Accept,
                                    consumed,
                                    effects,
                                };
                            }
                        }
                    } else {
                        pc = next_pc;
                    }
                } else if input.get(consumed) == Some(&node_id) {
                    consumed += 1;
                    match branch_target_offset(branch) {
                        Some(target) => pc = target,
                        None => {
                            return SemanticTrace {
                                outcome: SemanticOutcome::Accept,
                                consumed,
                                effects,
                            };
                        }
                    }
                } else {
                    pc = next_pc;
                }
            }
        }
    }
}

fn decode_semantic_entry(state: &[u8], pc: usize) -> Option<(StateEntry, usize)> {
    let mut decoder = StateDecoder::with_config_at(state, semantic_encode_config(), pc);
    let entry = decoder.next().ok()??;
    Some((entry, decoder.pc()))
}

fn internal_root_offset(tables: &GeneratedParserTables, node_id: u16) -> Option<usize> {
    let first_internal = u16::from(ND_BRANCH) + 1;
    let internal_index = node_id.checked_sub(first_internal)? as usize;
    tables
        .dispatch
        .int_nt_disp
        .get(internal_index)
        .copied()
        .map(usize::from)
}

fn branch_target_offset(branch: BranchTarget) -> Option<usize> {
    match branch {
        BranchTarget::Accept => None,
        BranchTarget::Relative(offset) | BranchTarget::Absolute(offset) => Some(offset),
    }
}

fn semantic_encode_config() -> EncodeConfig {
    EncodeConfig::new(ENCODE1BYTE_QBASIC_11)
}

fn read_optional_dos_prsstate(path: &Path) -> Result<Option<String>, GenerateError> {
    match std::fs::read_to_string(path) {
        Ok(source) if is_dos_prsstate(&source) => Ok(Some(source)),
        Ok(_) => Err(GenerateError::ArtifactParse(format!(
            "{} is not a DOS buildprs PRSSTATE.ASM capture",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(GenerateError::ArtifactParse(error.to_string())),
    }
}

fn is_dos_prsstate(source: &str) -> bool {
    source.contains("THIS IS NOT A SOURCE FILE")
        && source.contains("This file was created by program 'buildprs'")
        && source.contains("tState")
}

fn byte_diff_ranges(generated: &[u8], golden: &[u8]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let max_len = generated.len().max(golden.len());
    let mut index = 0;
    while index < max_len {
        if generated.get(index) == golden.get(index) {
            index += 1;
            continue;
        }
        let start = index;
        while index < max_len && generated.get(index) != golden.get(index) {
            index += 1;
        }
        ranges.push((start, index));
    }
    ranges
}

fn decode_state_entries(bytes: &[u8]) -> Vec<(usize, String)> {
    let mut entries = Vec::new();
    let mut pc = 0;
    while pc < bytes.len() {
        let offset = pc;
        let Some(entry) = decode_state_entry(bytes, &mut pc) else {
            entries.push((offset, "truncated".to_string()));
            break;
        };
        entries.push((offset, entry));
    }
    entries
}

fn decode_state_entry(bytes: &[u8], pc: &mut usize) -> Option<String> {
    let first = read_diag_byte(bytes, pc)?;
    match first {
        0 => Some("accept".to_string()),
        1 => Some("reject".to_string()),
        2 => Some(format!("mark({})", read_diag_byte(bytes, pc)?)),
        3 => {
            let lo = read_diag_byte(bytes, pc)?;
            let hi = read_diag_byte(bytes, pc)?;
            Some(format!("emit({})", u16::from_le_bytes([lo, hi])))
        }
        _ => {
            *pc -= 1;
            let node_id = decode_diag_node_id(bytes, pc)?;
            let branch = decode_diag_branch(bytes, pc)?;
            if node_id == 4 {
                Some(format!("branch({branch})"))
            } else {
                Some(format!("node({node_id},{branch})"))
            }
        }
    }
}

fn read_diag_byte(bytes: &[u8], pc: &mut usize) -> Option<u8> {
    let value = *bytes.get(*pc)?;
    *pc += 1;
    Some(value)
}

fn decode_diag_node_id(bytes: &[u8], pc: &mut usize) -> Option<u16> {
    let first = read_diag_byte(bytes, pc)?;
    if first < ENCODE1BYTE_QBASIC_11 {
        return Some(u16::from(first));
    }
    let second = read_diag_byte(bytes, pc)?;
    Some(((u16::from(first) << 8) | u16::from(second)) - 255 * u16::from(ENCODE1BYTE_QBASIC_11))
}

fn decode_diag_branch(bytes: &[u8], pc: &mut usize) -> Option<String> {
    let first = read_diag_byte(bytes, pc)?;
    if first == 255 {
        return Some("accept".to_string());
    }
    let cursor_after = *pc as isize;
    if first < ENCODE1BYTE_QBASIC_11 {
        let target = if first < ENCODE1BYTE_QBASIC_11 / 2 {
            cursor_after + isize::from(first)
        } else {
            cursor_after + isize::from(first) - isize::from(ENCODE1BYTE_QBASIC_11)
        };
        return Some(format!("relative({target})"));
    }
    let second = read_diag_byte(bytes, pc)?;
    let raw = (u16::from(first) << 8) | u16::from(second);
    let target = i32::from(raw) - 256 * i32::from(ENCODE1BYTE_QBASIC_11);
    Some(format!("absolute({target})"))
}

fn plain_micro_fixture_grammar(name: &str) -> Option<&'static str> {
    Some(match name {
        "emit_only" => {
            r#"TOKENS:
   tkBEEP ("BEEP");

Statements:
   tkBEEP EMIT(opStBeep);

Functions:

NonTerminals:
"#
        }
        "sequence" => {
            r#"TOKENS:
   tkA ("A"),
   tkB ("B");

Statements:
   tkA tkB EMIT(opStBeep);

Functions:

NonTerminals:
"#
        }
        "alternative" => {
            r#"TOKENS:
   tkA ("A"),
   tkB ("B"),
   tkC ("C");

Statements:
   tkA (tkB | tkC) EMIT(opStBeep);

Functions:

NonTerminals:
"#
        }
        "optional" => {
            r#"TOKENS:
   tkA ("A"),
   tkB ("B");

Statements:
   tkA [tkB] EMIT(opStBeep);

Functions:

NonTerminals:
"#
        }
        "repeat" => {
            r#"TOKENS:
   tkA ("A"),
   tkB ("B");

Statements:
   tkA {tkB} EMIT(opStBeep);

Functions:

NonTerminals:
"#
        }
        "mark_emit" => {
            r#"TOKENS:
   tkA ("A");

Statements:
   tkA MARK(3) EMIT(opStBeep);

Functions:

NonTerminals:
"#
        }
        "empty" => {
            r#"TOKENS:
   tkA ("A");

Statements:
   tkA;

Functions:

NonTerminals:
empty:
   ;
"#
        }
        _ => return None,
    })
}

const PLAIN_MICRO_PEROPCOD: &str = "opStBeep|x
";

pub fn ensure_plain_micro_fixture_sources(name: &str) -> std::io::Result<()> {
    let root = micro_fixture_root().join(name);
    std::fs::create_dir_all(&root)?;
    let grammar = plain_micro_fixture_grammar(name).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("unknown plain micro fixture {name}"),
        )
    })?;
    std::fs::write(root.join("grammar.prs"), grammar)?;
    std::fs::write(root.join("peropcod.txt"), PLAIN_MICRO_PEROPCOD)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEROPCOD: &str = "opStBeep|x\n";

    fn emit_only_fixture() -> MicroFixture {
        load_micro_fixture_from_strings(
            "emit_only",
            r#"TOKENS:
   tkBEEP ("BEEP"),

Statements:
   tkBEEP EMIT(opStBeep);

Functions:

NonTerminals:
"#,
            PEROPCOD,
            None,
            None,
            None,
        )
        .expect("emit_only fixture should load")
    }

    fn fixture_state_bytes(name: &str, opt_level: OptLevel) -> Vec<u8> {
        let fixture = load_micro_fixture(name)
            .unwrap_or_else(|error| panic!("fixture {name} should load: {error}"));
        generate_graph_tables_for_fixture(&fixture, opt_level)
            .unwrap_or_else(|error| panic!("fixture {name} should generate: {error}"))
            .state
    }

    fn assert_fixture_state(name: &str, expected: &[u8]) {
        assert_eq!(
            fixture_state_bytes(name, OptLevel::O0),
            expected,
            "{name} O0 graph bytes should match the focused DOS regression shape"
        );
    }

    #[test]
    fn regression_plain_micro_sequence_uses_grammar_derived_token_offsets() {
        assert_fixture_state("sequence", &[6, 1, 1, 3, 0, 0, 0]);
    }

    #[test]
    fn regression_alternative_arms_branch_directly_to_success_without_glue() {
        assert_fixture_state("alternative", &[6, 3, 7, 1, 1, 3, 0, 0, 0]);
    }

    #[test]
    fn regression_token_order_places_special_tokens_before_alphabetic_words() {
        assert_fixture_state("cg_1or2_args", &[7, 1, 1, 5, 1, 0, 7, 255, 1]);
    }

    #[test]
    fn regression_multi_item_optional_rejects_after_required_tail_failure() {
        assert_fixture_state("cg_1or2_args", &[7, 1, 1, 5, 1, 0, 7, 255, 1]);
    }

    #[test]
    fn regression_optional_branch_to_accept_uses_immediate_accept_operand() {
        assert_fixture_state("cg_0or1_args", &[6, 255, 0]);
    }

    #[test]
    fn regression_multi_item_repeat_loops_after_required_tail_success() {
        assert_fixture_state(
            "cg_call",
            &[2, 1, 9, 1, 1, 5, 1, 0, 9, 1, 1, 7, 3, 6, 255, 1, 9, 217, 1],
        );
    }

    fn assert_fixture_semantics_for_opt_levels(fixture: &MicroFixture, name: &str) {
        assert_micro_fixture_semantics(fixture, OptLevel::O0)
            .unwrap_or_else(|report| panic!("O0 semantics failed for {name}:\n{report}"));
        assert_micro_fixture_semantics(fixture, OptLevel::O1)
            .unwrap_or_else(|report| panic!("O1 semantics failed for {name}:\n{report}"));
        if !DOS_O2_HANG_MICRO_FIXTURES.contains(&name) {
            assert_micro_fixture_semantics(fixture, OptLevel::O2)
                .unwrap_or_else(|report| panic!("O2 semantics failed for {name}:\n{report}"));
        }
    }

    #[test]
    fn graph_backend_matches_cg_hint_microfixtures() {
        for name in CG_HINT_MICRO_FIXTURES {
            let fixture = load_micro_fixture(name)
                .unwrap_or_else(|error| panic!("fixture {name} should load: {error}"));
            let has_hint = fixture
                .grammar
                .statements
                .rules
                .iter()
                .any(|rule| rule.production.cg_hint.is_some());
            assert!(
                has_hint,
                "fixture {name} should include a cg_hint production"
            );
            assert!(
                fixture.golden_o0.is_some(),
                "fixture {name} is missing DOS O0 prsstate.asm"
            );
            assert!(
                fixture.golden_o1.is_some(),
                "fixture {name} is missing DOS O1 prsstate.asm"
            );
            assert!(
                fixture.golden_o2.is_some(),
                "fixture {name} is missing DOS O2 prsstate.asm"
            );
            assert_fixture_semantics_for_opt_levels(&fixture, name);
        }
    }

    #[test]
    #[ignore = "full qbasbnf graph O0 byte parity gate"]
    fn graph_backend_full_o0_matches_qbasic_11_default_prsstate_fixture() {
        let grammar = crate::buildprs_grammar::parse_grammar_file(
            "../../grammar/qbasbnf.prs",
        )
        .expect("qbasbnf should parse");
        let peropcod =
            std::fs::read_to_string("../../grammar/peropcod.txt")
                .expect("peropcod should read");
        let golden_source =
            std::fs::read_to_string("../../fixtures/buildprs/qbasic-1.1-o0/PRSSTATE.ASM")
                .expect("golden prsstate should read");
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod);
        let golden =
            generate_tables_from_prsstate(&golden_source, &opcodes).expect("golden should decode");
        let generated = generate_tables_from_graph(&grammar, &peropcod, OptLevel::O0)
            .expect("graph backend should generate tables");
        let diff = compare_generated_to_golden("qbasic-1.1-o0", OptLevel::O0, &generated, &golden);
        assert!(diff.state_bytes_match, "full O0 parity failed:\n{diff}");
    }

    #[test]
    #[ignore = "full qbasbnf graph O0 parity is not complete yet; run for progress metrics"]
    fn graph_backend_full_o0_parity_progress_against_qbasic_11_fixture() {
        let grammar = crate::buildprs_grammar::parse_grammar_file(
            "../../grammar/qbasbnf.prs",
        )
        .expect("qbasbnf should parse");
        let peropcod =
            std::fs::read_to_string("../../grammar/peropcod.txt")
                .expect("peropcod should read");
        let golden_source =
            std::fs::read_to_string("../../fixtures/buildprs/qbasic-1.1-o0/PRSSTATE.ASM")
                .expect("golden prsstate should read");
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod);
        let golden =
            generate_tables_from_prsstate(&golden_source, &opcodes).expect("golden should decode");
        let generated = generate_tables_from_graph(&grammar, &peropcod, OptLevel::O0)
            .expect("graph backend should generate tables");
        let diff = compare_generated_to_golden("qbasic-1.1-o0", OptLevel::O0, &generated, &golden);
        println!("{diff}");
        assert!(
            generated.state.len() as isize >= golden.state.len() as isize - 400,
            "graph O0 should stay within 400 bytes of golden while parity work continues"
        );
    }

    #[test]
    fn micro_harness_decodes_state_entries_for_emit_only_graph_output() {
        let fixture = emit_only_fixture();
        let generated = generate_graph_tables_for_fixture(&fixture, OptLevel::O1)
            .expect("graph backend should generate emit_only tables");
        let entries = decode_state_entries(&generated.state);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1, "emit(0)");
        assert_eq!(entries[1].1, "accept");
    }

    #[test]
    fn micro_harness_reports_compact_entry_diffs() {
        let fixture = emit_only_fixture();
        let mut generated = generate_graph_tables_for_fixture(&fixture, OptLevel::O1)
            .expect("graph backend should generate emit_only tables");
        generated.state[0] ^= 0xFF;
        let golden = GeneratedParserTables {
            state: vec![3, 0, 0, 0],
            dispatch: generated.dispatch.clone(),
            statement_offsets: std::collections::BTreeMap::new(),
            statement_offset_list: Vec::new(),
            function_offsets: std::collections::BTreeMap::new(),
        };
        let diff = compare_generated_to_golden("emit_only", OptLevel::O1, &generated, &golden);
        assert!(!diff.state_bytes_match);
        assert!(!diff.decoded_entry_diffs.is_empty());
        assert!(diff.to_string().contains("emit_only"));
    }

    fn full_qbasic_semantic_diff_for_sampled_inputs(opt_level: OptLevel) -> Option<SemanticDiff> {
        let grammar = crate::buildprs_grammar::parse_grammar_file(
            "../../grammar/qbasbnf.prs",
        )
        .expect("qbasbnf should parse");
        let peropcod =
            std::fs::read_to_string("../../grammar/peropcod.txt")
                .expect("peropcod should read");
        let golden_source =
            std::fs::read_to_string("../../fixtures/buildprs/qbasic-1.1/prsstate.asm")
                .expect("golden prsstate should read");
        let opcodes = parse_opcode_equates_from_peropcod(&peropcod);
        let golden =
            generate_tables_from_prsstate(&golden_source, &opcodes).expect("golden should decode");
        let generated = generate_tables_from_graph(&grammar, &peropcod, opt_level)
            .expect("graph backend should generate tables");

        let alphabet = semantic_alphabet(&generated, &golden);
        let starts = semantic_start_offsets(&generated, &golden);
        let inputs = sampled_inputs(&alphabet);
        first_semantic_diff_for_inputs(&generated, &golden, &starts, &inputs)
    }

    #[test]
    fn full_qbasic_o1_is_semantically_equivalent_for_sampled_inputs() {
        let diff = full_qbasic_semantic_diff_for_sampled_inputs(OptLevel::O1);
        assert_eq!(diff, None);
    }

    #[test]
    fn full_qbasic_o2_is_semantically_equivalent_for_sampled_inputs() {
        let diff = full_qbasic_semantic_diff_for_sampled_inputs(OptLevel::O2);
        assert_eq!(diff, None);
    }

    #[test]
    fn micro_fixtures_have_checked_in_dos_goldens_for_all_captured_levels() {
        let names = PLAIN_MICRO_FIXTURES
            .iter()
            .chain(CG_HINT_MICRO_FIXTURES.iter())
            .chain(FULL_GAP_MICRO_FIXTURES.iter());
        for name in names {
            let fixture = load_micro_fixture(name)
                .unwrap_or_else(|error| panic!("fixture {name} should load: {error}"));
            assert!(
                fixture.golden_o0.is_some(),
                "fixture {name} is missing DOS o0/prsstate.asm"
            );
            assert!(
                fixture.golden_o1.is_some(),
                "fixture {name} is missing DOS o1/prsstate.asm"
            );
            if DOS_O2_HANG_MICRO_FIXTURES.contains(name) {
                assert!(
                    fixture.golden_o2.is_none(),
                    "fixture {name} should not check in DOS o2/prsstate.asm because DOS buildprs hangs"
                );
            } else {
                assert!(
                    fixture.golden_o2.is_some(),
                    "fixture {name} is missing DOS o2/prsstate.asm"
                );
            }
        }
    }

    #[test]
    fn graph_backend_o0_is_semantically_equivalent_for_plain_microfixtures() {
        for name in PLAIN_MICRO_FIXTURES {
            let fixture = load_micro_fixture(name)
                .unwrap_or_else(|error| panic!("fixture {name} should load: {error}"));
            assert_micro_fixture_semantics(&fixture, OptLevel::O0)
                .unwrap_or_else(|report| panic!("O0 semantics failed for {name}:\n{report}"));
        }
    }

    #[test]
    fn graph_backend_o1_is_semantically_equivalent_for_plain_microfixtures() {
        for name in PLAIN_MICRO_FIXTURES {
            let fixture = load_micro_fixture(name)
                .unwrap_or_else(|error| panic!("fixture {name} should load: {error}"));
            assert_micro_fixture_semantics(&fixture, OptLevel::O1)
                .unwrap_or_else(|report| panic!("O1 semantics failed for {name}:\n{report}"));
        }
    }

    #[test]
    fn graph_backend_o2_is_semantically_equivalent_for_plain_microfixtures() {
        for name in PLAIN_MICRO_FIXTURES {
            let fixture = load_micro_fixture(name)
                .unwrap_or_else(|error| panic!("fixture {name} should load: {error}"));
            assert_micro_fixture_semantics(&fixture, OptLevel::O2)
                .unwrap_or_else(|report| panic!("O2 semantics failed for {name}:\n{report}"));
        }
    }

    #[test]
    fn graph_backend_is_semantically_equivalent_for_full_gap_microfixtures() {
        for name in FULL_GAP_MICRO_FIXTURES {
            let fixture = load_micro_fixture(name)
                .unwrap_or_else(|error| panic!("fixture {name} should load: {error}"));
            assert_fixture_semantics_for_opt_levels(&fixture, name);
        }
    }
}

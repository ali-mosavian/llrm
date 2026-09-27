//! Generate and validate `prstab.inc` / `prstab.h` equates from grammar-derived data.
//!
//! This module consolidates the scalar constants emitted by the original DOS
//! `buildprs` tool: `NTOKENS`, `NUMNT*`, `RWF_*`, `CB_RW_MAX`, `IRW_ALPHA_FIRST`,
//! fixed `ND_*` / `ENCODE1BYTE` values, and `STI_*` state entry offsets.
//!
//! `STI_*` offsets require byte-accurate state lowering. Until that pass is
//! complete, inject them from golden artifacts via [`StiOffsetInput`].

use std::collections::BTreeMap;

use crate::buildprs_artifacts::{parse_equates, symbols_with_prefix, ParseEquatesError};
use crate::buildprs_dispatch::derive_nonterminal_dispatch_order;
use crate::buildprs_grammar::{GrammarFile, TokenDef};
use crate::buildprs_tokens::{self, RwfConstants, TokenArtifacts};

pub use crate::buildprs_generator::ENCODE1BYTE_QBASIC_11;

/// `ND_*` directive bytes in generated `prstab.inc`.
pub const ND_ACCEPT: u8 = 0;
pub const ND_REJECT: u8 = 1;
pub const ND_MARK: u8 = 2;
pub const ND_EMIT: u8 = 3;
pub const ND_BRANCH: u8 = 4;

/// Accept pseudo-token id used in indexed token-dispatch nodes.
pub const STT_ACCEPT: u8 = 0xFF;

/// Scalar constants mirrored in `prstab.inc` / `prstab.h`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrstabConstants {
    pub encode1byte: u8,
    pub nd_accept: u8,
    pub nd_reject: u8,
    pub nd_mark: u8,
    pub nd_emit: u8,
    pub nd_branch: u8,
    pub stt_accept: u8,
    pub ntokens: u16,
    pub numntint: u16,
    pub numntext: u16,
    pub numnt: u16,
    pub irw_alpha_first: u16,
    pub cb_rw_max: u8,
    pub rwf: RwfConstants,
    /// `STI_*` symbol → absolute byte offset into `tState`.
    pub sti_offsets: BTreeMap<String, u16>,
}

/// Injected `STI_*` offsets used when state lowering is incomplete.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StiOffsetInput {
    pub offsets: BTreeMap<String, u16>,
}

impl StiOffsetInput {
    /// Parse `STI_*` equates from golden `prstab.inc` text.
    pub fn from_prstab_inc(text: &str) -> Result<Self, PrstabParseError> {
        let equates = parse_equates(text).map_err(PrstabParseError::from)?;
        Ok(Self::from_sti_equates(&equates))
    }

    /// Parse `STI_*` `#define`s from golden `prstab.h` text.
    pub fn from_prstab_h(text: &str) -> Result<Self, PrstabParseError> {
        let defines = parse_prstab_defines(text)?;
        Ok(Self::from_sti_equates(&defines))
    }

    /// Build `STI_{Name}` entries from resolved equate / define maps.
    pub fn from_sti_equates(equates: &BTreeMap<String, i64>) -> Self {
        Self {
            offsets: symbols_with_prefix(equates, "STI_")
                .into_iter()
                .map(|(name, value)| (name.to_string(), value as u16))
                .collect(),
        }
    }

    /// Derive `STI_{Name}` from `tIntNtDisp` offsets for selected internal NTs.
    pub fn from_int_nt_offsets(
        internal_names: &[String],
        offsets: &[u16],
        nt_names: &[&str],
    ) -> Self {
        let mut injected = BTreeMap::new();
        for nt_name in nt_names {
            if let Some(index) = internal_names.iter().position(|name| name == nt_name) {
                if let Some(&offset) = offsets.get(index) {
                    injected.insert(format!("STI_{nt_name}"), offset);
                }
            }
        }
        Self { offsets: injected }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrstabParseError {
    pub artifact: &'static str,
    pub detail: String,
}

impl std::fmt::Display for PrstabParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.artifact, self.detail)
    }
}

impl std::error::Error for PrstabParseError {}

impl From<ParseEquatesError> for PrstabParseError {
    fn from(error: ParseEquatesError) -> Self {
        Self {
            artifact: "prstab.inc",
            detail: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrstabParityMismatch {
    pub artifact: &'static str,
    pub detail: String,
}

impl std::fmt::Display for PrstabParityMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.artifact, self.detail)
    }
}

impl std::error::Error for PrstabParityMismatch {}

/// Build `prstab` constants from grammar token/dispatch data and injected `STI_*` offsets.
pub fn generate_prstab_constants(grammar: &GrammarFile, sti: &StiOffsetInput) -> PrstabConstants {
    let token_artifacts = buildprs_tokens::generate_token_artifacts(&grammar.tokens);
    generate_prstab_constants_from_parts(&token_artifacts, grammar, sti)
}

/// Build `prstab` constants from precomputed token artifacts.
pub fn generate_prstab_constants_from_parts(
    token_artifacts: &TokenArtifacts,
    grammar: &GrammarFile,
    sti: &StiOffsetInput,
) -> PrstabConstants {
    let dispatch = derive_nonterminal_dispatch_order(grammar);
    let numntint = u16::try_from(dispatch.internal.len()).expect("internal NT count fits in u16");
    let numntext = u16::try_from(dispatch.external.len()).expect("external NT count fits in u16");

    PrstabConstants {
        encode1byte: ENCODE1BYTE_QBASIC_11,
        nd_accept: ND_ACCEPT,
        nd_reject: ND_REJECT,
        nd_mark: ND_MARK,
        nd_emit: ND_EMIT,
        nd_branch: ND_BRANCH,
        stt_accept: STT_ACCEPT,
        ntokens: u16::try_from(token_artifacts.token_count).expect("token count fits in u16"),
        numntint,
        numntext,
        numnt: numntint + numntext,
        irw_alpha_first: token_artifacts.irw_alpha_first,
        cb_rw_max: longest_reserved_word_spelling(&grammar.tokens),
        rwf: token_artifacts.rwf,
        sti_offsets: sti.offsets.clone(),
    }
}

fn longest_reserved_word_spelling(tokens: &[TokenDef]) -> u8 {
    tokens
        .iter()
        .map(|token| token.spelling.len())
        .max()
        .unwrap_or(0)
        .try_into()
        .expect("longest reserved word spelling fits in u8")
}

impl PrstabConstants {
    /// Flatten scalar equates for comparison against golden artifacts.
    pub fn scalar_equates(&self) -> BTreeMap<String, i64> {
        let mut equates = BTreeMap::from([
            ("ENCODE1BYTE".to_string(), i64::from(self.encode1byte)),
            ("NTOKENS".to_string(), i64::from(self.ntokens)),
            ("ND_ACCEPT".to_string(), i64::from(self.nd_accept)),
            ("ND_REJECT".to_string(), i64::from(self.nd_reject)),
            ("ND_MARK".to_string(), i64::from(self.nd_mark)),
            ("ND_EMIT".to_string(), i64::from(self.nd_emit)),
            ("ND_BRANCH".to_string(), i64::from(self.nd_branch)),
            ("STT_ACCEPT".to_string(), i64::from(self.stt_accept)),
            ("NUMNTINT".to_string(), i64::from(self.numntint)),
            ("NUMNTEXT".to_string(), i64::from(self.numntext)),
            ("NUMNT".to_string(), i64::from(self.numnt)),
            (
                "IRW_ALPHA_FIRST".to_string(),
                i64::from(self.irw_alpha_first),
            ),
            ("CB_RW_MAX".to_string(), i64::from(self.cb_rw_max)),
            ("RWF_OPERATOR".to_string(), i64::from(self.rwf.operator)),
            ("RWF_FUNC".to_string(), i64::from(self.rwf.func)),
            ("RWF_STR".to_string(), i64::from(self.rwf.str_suffix)),
            ("RWF_FUNC_CG".to_string(), i64::from(self.rwf.func_cg)),
            ("RWF_STMT_CG".to_string(), i64::from(self.rwf.stmt_cg)),
            ("RWF_NO_DIRECT".to_string(), i64::from(self.rwf.no_direct)),
            ("RWF_NSTMTS".to_string(), i64::from(self.rwf.nstmts_mask)),
        ]);

        for (name, value) in &self.sti_offsets {
            equates.insert(name.clone(), i64::from(*value));
        }

        equates
    }
}

/// Compare generated constants against golden `prstab.inc` equates.
pub fn compare_to_prstab_inc(
    generated: &PrstabConstants,
    prstab_text: &str,
) -> Result<(), PrstabParityMismatch> {
    let golden = parse_equates(prstab_text).map_err(|error| PrstabParityMismatch {
        artifact: "prstab.inc",
        detail: format!("parse error: {error}"),
    })?;
    compare_equate_maps(generated, "prstab.inc", &golden)
}

/// Compare generated constants against golden `prstab.h` `#define`s.
pub fn compare_to_prstab_h(
    generated: &PrstabConstants,
    prstab_h_text: &str,
) -> Result<(), PrstabParityMismatch> {
    let golden = parse_prstab_defines(prstab_h_text).map_err(|error| PrstabParityMismatch {
        artifact: "prstab.h",
        detail: error.detail,
    })?;
    compare_equate_maps(generated, "prstab.h", &golden)
}

fn compare_equate_maps(
    generated: &PrstabConstants,
    artifact: &'static str,
    golden: &BTreeMap<String, i64>,
) -> Result<(), PrstabParityMismatch> {
    let expected = generated.scalar_equates();

    for (name, value) in &expected {
        match golden.get(name) {
            Some(actual) if actual == value => {}
            Some(actual) => {
                return Err(PrstabParityMismatch {
                    artifact,
                    detail: format!("{name}: expected {value}, golden {actual}"),
                });
            }
            None => {
                return Err(PrstabParityMismatch {
                    artifact,
                    detail: format!("missing golden symbol {name}"),
                });
            }
        }
    }

    for name in golden.keys() {
        if name.starts_with("STI_") && !expected.contains_key(name) {
            return Err(PrstabParityMismatch {
                artifact,
                detail: format!("generated constants missing {name}"),
            });
        }
    }

    Ok(())
}

/// Parse `#define NAME <value>` lines from generated `prstab.h`.
pub fn parse_prstab_defines(text: &str) -> Result<BTreeMap<String, i64>, PrstabParseError> {
    let mut defines = BTreeMap::new();

    for (line_idx, line) in text.lines().enumerate() {
        let line_no = line_idx + 1;
        let trimmed = strip_c_comment(line).trim();
        if trimmed.is_empty() || trimmed.starts_with('#') && !trimmed.starts_with("#define") {
            continue;
        }

        let Some(rest) = trimmed.strip_prefix("#define") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some((name, value_text)) = split_define_value(rest) else {
            return Err(PrstabParseError {
                artifact: "prstab.h",
                detail: format!("line {line_no}: malformed #define"),
            });
        };

        let value = match parse_numeric_literal(value_text) {
            Some(value) => value,
            None => continue,
        };

        if defines.insert(name.to_string(), value).is_some() {
            return Err(PrstabParseError {
                artifact: "prstab.h",
                detail: format!("line {line_no}: duplicate define {name}"),
            });
        }
    }

    Ok(defines)
}

fn split_define_value(rest: &str) -> Option<(&str, &str)> {
    let mut parts = rest.split_whitespace();
    let name = parts.next()?;
    let value = parts.next()?;
    Some((name, value))
}

fn strip_c_comment(line: &str) -> &str {
    line.split("/*").next().unwrap_or(line)
}

fn parse_numeric_literal(text: &str) -> Option<i64> {
    let literal = text.trim();
    if let Some(stem) = literal
        .strip_prefix("0x")
        .or_else(|| literal.strip_prefix("0X"))
    {
        return i64::from_str_radix(stem, 16).ok();
    }
    if let Some(stem) = literal
        .strip_suffix('H')
        .or_else(|| literal.strip_suffix('h'))
    {
        return i64::from_str_radix(stem, 16).ok();
    }
    literal.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buildprs_dispatch::parse_golden_dispatch_tables;
    use crate::buildprs_grammar::{parse_grammar_file, parse_token_decls_file};
    use crate::buildprs_tokens::IRW_ALPHA_FIRST;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("../../fixtures/buildprs/qbasic-1.1/{name}"))
            .unwrap_or_else(|error| panic!("read fixture {name}: {error}"))
    }

    fn grammar_path() -> &'static str {
        "../../grammar/qbasbnf.prs"
    }

    fn qbasic_11_constants_with_fixture_sti() -> PrstabConstants {
        let grammar = parse_grammar_file(grammar_path()).expect("qbasbnf should parse");
        let sti = StiOffsetInput::from_prstab_inc(&fixture("prstab.inc")).expect("STI equates");
        generate_prstab_constants(&grammar, &sti)
    }

    #[test]
    fn qbasic_11_prstab_constants_match_inc_fixture() {
        let generated = qbasic_11_constants_with_fixture_sti();
        compare_to_prstab_inc(&generated, &fixture("prstab.inc"))
            .expect("generated constants should match prstab.inc");
    }

    #[test]
    fn qbasic_11_prstab_constants_match_h_fixture() {
        let generated = qbasic_11_constants_with_fixture_sti();
        compare_to_prstab_h(&generated, &fixture("prstab.h"))
            .expect("generated constants should match prstab.h");
    }

    #[test]
    fn qbasic_11_nt_and_rw_counts_match_fixture() {
        let generated = qbasic_11_constants_with_fixture_sti();

        assert_eq!(generated.ntokens, 246);
        assert_eq!(generated.numntint, 29);
        assert_eq!(generated.numntext, 49);
        assert_eq!(generated.numnt, 78);
        assert_eq!(generated.irw_alpha_first, IRW_ALPHA_FIRST);
        assert_eq!(generated.cb_rw_max, 9);
        assert_eq!(generated.encode1byte, 224);
        assert_eq!(generated.stt_accept, 0xFF);
    }

    #[test]
    fn qbasic_11_rwf_constants_match_fixture() {
        let generated = qbasic_11_constants_with_fixture_sti();
        let rwf = RwfConstants::QBASIC_11;

        assert_eq!(generated.rwf, rwf);
    }

    #[test]
    fn sti_injection_from_prstab_inc_matches_fixture() {
        let sti = StiOffsetInput::from_prstab_inc(&fixture("prstab.inc")).expect("STI equates");

        assert_eq!(sti.offsets["STI_AsClausePrim"], 2463);
        assert_eq!(sti.offsets["STI_AsClause"], 2511);
        assert_eq!(sti.offsets["STI_AsClauseAny"], 2518);
    }

    #[test]
    fn sti_injection_from_prsstate_int_nt_disp_matches_fixture() {
        let golden = parse_golden_dispatch_tables(&fixture("prsstate.asm"));
        let sti = StiOffsetInput::from_int_nt_offsets(
            &golden.internal_names,
            &golden.internal_offsets,
            &["AsClausePrim", "AsClause", "AsClauseAny"],
        );

        assert_eq!(sti.offsets["STI_AsClausePrim"], 2463);
        assert_eq!(sti.offsets["STI_AsClause"], 2511);
        assert_eq!(sti.offsets["STI_AsClauseAny"], 2518);
    }

    #[test]
    fn sti_injection_from_prstab_h_matches_inc_fixture() {
        let from_inc =
            StiOffsetInput::from_prstab_inc(&fixture("prstab.inc")).expect("prstab.inc STI");
        let from_h = StiOffsetInput::from_prstab_h(&fixture("prstab.h")).expect("prstab.h STI");

        assert_eq!(from_inc, from_h);
    }

    #[test]
    fn cb_rw_max_derived_from_token_spellings() {
        let tokens = parse_token_decls_file(grammar_path()).expect("tokens should parse");
        assert_eq!(longest_reserved_word_spelling(&tokens), 9);
    }

    #[test]
    fn parse_prstab_h_reads_hex_and_decimal_defines() {
        let defines = parse_prstab_defines(&fixture("prstab.h")).expect("prstab.h defines");

        assert_eq!(defines["NTOKENS"], 246);
        assert_eq!(defines["STT_ACCEPT"], 0xFF);
        assert_eq!(defines["RWF_OPERATOR"], 0x80);
        assert_eq!(defines["STI_AsClausePrim"], 2463);
    }
}

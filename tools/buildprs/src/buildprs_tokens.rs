//! Generate and validate `buildprs` token-layer artifacts from grammar `TOKENS:`.
//!
//! Covers `IRW_*` ids, `RWF_*` flag constants, `mpIRWtoChar`, `mpIRWtoIOP`,
//! per-letter `tRw` bucket bytes, and `ORW_*` numeric offsets. Statement/function
//! parse offsets and `<Cg...>` hints come from lowering when available; parity
//! scaffold tests can inject them from captured `prsrwt.asm` / `prsorw.inc`.

use std::collections::BTreeMap;

use crate::buildprs_artifacts::{parse_equates, parse_irw_equates, ParseDataError};
use crate::buildprs_grammar::TokenDef;

/// Number of leading single-character / punctuation tokens in QBasic 1.1.
pub const IRW_ALPHA_FIRST: u16 = 25;

/// `RWF_*` flag values emitted into `prstab.inc` by the original `buildprs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RwfConstants {
    pub operator: u8,
    pub func: u8,
    pub str_suffix: u8,
    pub func_cg: u8,
    pub stmt_cg: u8,
    pub no_direct: u8,
    pub nstmts_mask: u8,
}

impl RwfConstants {
    /// Values from captured `fixtures/buildprs/qbasic-1.1/prstab.inc`.
    pub const QBASIC_11: Self = Self {
        operator: 0x80,
        func: 0x40,
        str_suffix: 0x04,
        func_cg: 0x10,
        stmt_cg: 0x08,
        no_direct: 0x20,
        nstmts_mask: 0x03,
    };
}

/// Token-layer artifacts derived from a parsed `TOKENS:` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenArtifacts {
    pub token_count: usize,
    pub irw_alpha_first: u16,
    pub rwf: RwfConstants,
    /// `IRW_*` symbol → dense id (0..token_count-1) in grammar declaration order.
    pub irw_ids: BTreeMap<String, u16>,
    /// `IRW_*` symbols in declaration order.
    pub irw_order: Vec<String>,
    /// `tk*` grammar name → `IRW_*` symbol.
    pub tk_to_irw: BTreeMap<String, String>,
    /// First [`IRW_ALPHA_FIRST`] spellings as bytes for `mpIRWtoChar`.
    pub mp_irw_to_char: Vec<u8>,
    /// First [`IRW_ALPHA_FIRST`] operator indices for `mpIRWtoIOP` (`0xFF` = none).
    pub mp_irw_to_iop: Vec<u8>,
    /// Per-token `RWF_*` bits inferable from `TOKENS:` attributes only.
    pub token_rwf_flags: BTreeMap<String, u8>,
    /// `ORW_*` symbol names for alphabetic reserved words (offsets deferred).
    pub orw_names: Vec<String>,
}

/// One statement production attached to a reserved-word `tRw` entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RwStmtLowering {
    pub stmt_offset: u16,
    pub cg_fn: Option<String>,
    pub cg_arg: Option<String>,
}

/// Lowered statement/function metadata for one alphabetic reserved word.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RwEntryLowering {
    pub func_offset: Option<u16>,
    pub func_cg_fn: Option<String>,
    pub func_cg_arg: Option<String>,
    pub stmt_entries: Vec<RwStmtLowering>,
    pub am_resolver: Option<String>,
}

/// Per-`ORW_*` lowering hints keyed by lister symbol (e.g. `ORW_PRINT`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RwLoweringInput {
    pub entries: BTreeMap<String, RwEntryLowering>,
}

/// Stable numeric stand-ins for symbolic `DW` operands in `prsrwt.asm`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RwSymbolCodec {
    symbols: BTreeMap<String, u16>,
    next: u16,
}

impl RwSymbolCodec {
    pub const SYMBOL_BASE: u16 = 0x8000;

    pub fn new() -> Self {
        Self {
            symbols: BTreeMap::new(),
            next: Self::SYMBOL_BASE,
        }
    }

    pub fn intern(&mut self, symbol: &str) -> u16 {
        if let Some(value) = self.symbols.get(symbol) {
            return *value;
        }
        let value = self.next;
        self.next = self.next.wrapping_add(1);
        self.symbols.insert(symbol.to_string(), value);
        value
    }

    pub fn lookup(&self, symbol: &str) -> Option<u16> {
        self.symbols.get(symbol).copied()
    }

    pub fn is_symbol_value(value: u16) -> bool {
        value >= Self::SYMBOL_BASE
    }

    pub fn symbol_name(&self, value: u16) -> Option<&str> {
        self.symbols
            .iter()
            .find_map(|(name, &encoded)| (encoded == value).then_some(name.as_str()))
    }
}

impl Default for RwSymbolCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// One per-letter reserved-word bucket (`t41Rw` … `t59Rw`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RwBucket {
    pub letter: char,
    pub first_irw: u16,
    pub bytes: Vec<u8>,
}

/// Generated reserved-word table bytes and `ORW_*` offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservedWordTableArtifacts {
    pub buckets: Vec<RwBucket>,
    pub trw_pointers: Vec<u16>,
    pub flat_blob: Vec<u8>,
    pub orw_offsets: BTreeMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenParityMismatch {
    pub artifact: &'static str,
    pub detail: String,
}

impl std::fmt::Display for TokenParityMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.artifact, self.detail)
    }
}

impl std::error::Error for TokenParityMismatch {}

/// Build token artifacts from grammar token declarations.
pub fn generate_token_artifacts(tokens: &[TokenDef]) -> TokenArtifacts {
    let rwf = RwfConstants::QBASIC_11;
    let ordered_tokens = buildprs_token_order(tokens);
    let mut irw_ids = BTreeMap::new();
    let mut irw_order = Vec::with_capacity(ordered_tokens.len());
    let mut tk_to_irw = BTreeMap::new();
    let mut token_rwf_flags = BTreeMap::new();
    let mut orw_names = Vec::new();

    for (index, token) in ordered_tokens.iter().enumerate() {
        let irw_name = irw_name_for_token(token, tokens);
        let id = u16::try_from(index).expect("token index fits in u16");
        irw_ids.insert(irw_name.clone(), id);
        irw_order.push(irw_name.clone());
        tk_to_irw.insert(token.name.clone(), irw_name.clone());
        token_rwf_flags.insert(irw_name.clone(), rwf_flags_from_token(token, rwf));

        if is_alpha_reserved_word(token) {
            orw_names.push(orw_name_from_irw(&irw_name));
        }
    }

    let prefix_len = usize::from(IRW_ALPHA_FIRST.min(ordered_tokens.len() as u16));
    let mp_irw_to_char = ordered_tokens[..prefix_len]
        .iter()
        .map(|token| spelling_to_byte(&token.spelling))
        .collect();
    let mp_irw_to_iop = ordered_tokens[..prefix_len]
        .iter()
        .map(|token| iop_for_special_token(token).unwrap_or(0xFF))
        .collect();

    TokenArtifacts {
        token_count: ordered_tokens.len(),
        irw_alpha_first: IRW_ALPHA_FIRST,
        rwf,
        irw_ids,
        irw_order,
        tk_to_irw,
        mp_irw_to_char,
        mp_irw_to_iop,
        token_rwf_flags,
        orw_names,
    }
}

fn buildprs_token_order(tokens: &[TokenDef]) -> Vec<&TokenDef> {
    let mut ordered = tokens
        .iter()
        .filter(|token| !is_alpha_reserved_word(token))
        .collect::<Vec<_>>();
    let mut alpha = tokens
        .iter()
        .filter(|token| is_alpha_reserved_word(token))
        .collect::<Vec<_>>();
    alpha.sort_by(|left, right| left.spelling.cmp(&right.spelling));
    ordered.extend(alpha);
    ordered
}

/// Map a grammar `tk*` name to the generated `IRW_*` symbol.
pub fn irw_name_from_tk(tk_name: &str) -> String {
    let suffix = tk_name.strip_prefix("tk").unwrap_or(tk_name);
    format!("IRW_{suffix}")
}

/// Map a token declaration to the `IRW_*` symbol `buildprs` emits.
pub fn irw_name_for_token(token: &TokenDef, _tokens: &[TokenDef]) -> String {
    let spelling = &token.spelling;
    if is_alpha_keyword_spelling(spelling) {
        let base = spelling.trim_end_matches('$');
        if spelling.ends_with('$') {
            return format!("IRW_{base}_");
        }
        return format!("IRW_{spelling}");
    }

    irw_name_from_tk(&token.name)
}

fn is_alpha_keyword_spelling(spelling: &str) -> bool {
    spelling
        .chars()
        .all(|ch| ch.is_ascii_alphabetic() || ch == '$')
        && spelling
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
}

/// Map an `IRW_*` symbol to the lister `ORW_*` symbol.
pub fn orw_name_from_irw(irw_name: &str) -> String {
    let suffix = irw_name.strip_prefix("IRW_").unwrap_or(irw_name);
    format!("ORW_{suffix}")
}

/// Compare generated artifacts against captured `prsirw.inc` equates.
pub fn compare_irw_equates(
    artifacts: &TokenArtifacts,
    prsirw_text: &str,
) -> Result<(), TokenParityMismatch> {
    let golden = parse_irw_equates(prsirw_text).map_err(|error| TokenParityMismatch {
        artifact: "prsirw.inc",
        detail: format!("parse error: {error}"),
    })?;

    if golden.len() != artifacts.token_count {
        return Err(TokenParityMismatch {
            artifact: "prsirw.inc",
            detail: format!(
                "token count mismatch: grammar {} vs golden {}",
                artifacts.token_count,
                golden.len()
            ),
        });
    }

    for (name, &expected) in &artifacts.irw_ids {
        match golden.get(name) {
            Some(actual) if i64::from(expected) == *actual => {}
            Some(actual) => {
                return Err(TokenParityMismatch {
                    artifact: "prsirw.inc",
                    detail: format!("{name}: expected {expected}, golden {actual}"),
                });
            }
            None => {}
        }
    }

    for (name, actual) in &golden {
        match artifacts.irw_ids.get(name) {
            Some(expected) if i64::from(*expected) == *actual => {}
            Some(expected) => {
                return Err(TokenParityMismatch {
                    artifact: "prsirw.inc",
                    detail: format!("{name}: expected {expected}, golden {actual}"),
                });
            }
            None => {
                return Err(TokenParityMismatch {
                    artifact: "prsirw.inc",
                    detail: format!("missing generated equate {name}"),
                });
            }
        }
    }

    Ok(())
}

/// Compare `RWF_*` constants and `NTOKENS` / `IRW_ALPHA_FIRST` against `prstab.inc`.
pub fn compare_prstab_constants(
    artifacts: &TokenArtifacts,
    prstab_text: &str,
) -> Result<(), TokenParityMismatch> {
    let golden = parse_equates(prstab_text).map_err(|error| TokenParityMismatch {
        artifact: "prstab.inc",
        detail: format!("parse error: {error}"),
    })?;

    let expected = [
        ("NTOKENS", i64::from(artifacts.token_count as u16)),
        ("IRW_ALPHA_FIRST", i64::from(artifacts.irw_alpha_first)),
        ("RWF_OPERATOR", i64::from(artifacts.rwf.operator)),
        ("RWF_FUNC", i64::from(artifacts.rwf.func)),
        ("RWF_STR", i64::from(artifacts.rwf.str_suffix)),
        ("RWF_FUNC_CG", i64::from(artifacts.rwf.func_cg)),
        ("RWF_STMT_CG", i64::from(artifacts.rwf.stmt_cg)),
        ("RWF_NO_DIRECT", i64::from(artifacts.rwf.no_direct)),
        ("RWF_NSTMTS", i64::from(artifacts.rwf.nstmts_mask)),
    ];

    for (name, value) in expected {
        match golden.get(name) {
            Some(actual) if *actual == value => {}
            Some(actual) => {
                return Err(TokenParityMismatch {
                    artifact: "prstab.inc",
                    detail: format!("{name}: expected {value}, golden {actual}"),
                });
            }
            None => {
                return Err(TokenParityMismatch {
                    artifact: "prstab.inc",
                    detail: format!("missing golden equate {name}"),
                });
            }
        }
    }

    Ok(())
}

/// Compare `mpIRWtoChar` / `mpIRWtoIOP` prefix tables against `prsrwt.asm`.
pub fn compare_mp_tables(
    artifacts: &TokenArtifacts,
    prsrwt_text: &str,
) -> Result<(), TokenParityMismatch> {
    let golden_char =
        parse_mp_byte_table(prsrwt_text, "mpIRWtoChar").map_err(|error| TokenParityMismatch {
            artifact: "prsrwt.asm",
            detail: format!("mpIRWtoChar: {error}"),
        })?;
    let golden_iop =
        parse_mp_byte_table(prsrwt_text, "mpIRWtoIOP").map_err(|error| TokenParityMismatch {
            artifact: "prsrwt.asm",
            detail: format!("mpIRWtoIOP: {error}"),
        })?;

    if golden_char != artifacts.mp_irw_to_char {
        return Err(TokenParityMismatch {
            artifact: "prsrwt.asm",
            detail: format!(
                "mpIRWtoChar mismatch at prefix len {}: expected {:?}, golden {:?}",
                artifacts.mp_irw_to_char.len(),
                artifacts.mp_irw_to_char,
                golden_char
            ),
        });
    }

    if golden_iop != artifacts.mp_irw_to_iop {
        return Err(TokenParityMismatch {
            artifact: "prsrwt.asm",
            detail: format!(
                "mpIRWtoIOP mismatch: expected {:?}, golden {:?}",
                artifacts.mp_irw_to_iop, golden_iop
            ),
        });
    }

    Ok(())
}

/// Verify `ORW_*` inventory matches alphabetic grammar tokens in `prsorw.inc`.
pub fn compare_orw_inventory(
    artifacts: &TokenArtifacts,
    prsorw_text: &str,
) -> Result<(), TokenParityMismatch> {
    let golden = parse_orw_equates(prsorw_text).map_err(|error| TokenParityMismatch {
        artifact: "prsorw.inc",
        detail: format!("parse error: {error}"),
    })?;

    if golden.len() != artifacts.orw_names.len() {
        return Err(TokenParityMismatch {
            artifact: "prsorw.inc",
            detail: format!(
                "ORW count mismatch: grammar {} vs golden {}",
                artifacts.orw_names.len(),
                golden.len()
            ),
        });
    }

    for name in &artifacts.orw_names {
        if !golden.contains_key(name) {
            return Err(TokenParityMismatch {
                artifact: "prsorw.inc",
                detail: format!("missing golden equate {name}"),
            });
        }
    }

    Ok(())
}

/// Run all token-layer golden comparisons.
pub fn validate_against_golden(
    artifacts: &TokenArtifacts,
    prstab_text: &str,
    prsirw_text: &str,
    prsorw_text: &str,
    prsrwt_text: &str,
) -> Result<(), TokenParityMismatch> {
    compare_prstab_constants(artifacts, prstab_text)?;
    compare_irw_equates(artifacts, prsirw_text)?;
    compare_orw_inventory(artifacts, prsorw_text)?;
    compare_mp_tables(artifacts, prsrwt_text)?;
    Ok(())
}

/// Build per-letter `tRw` buckets and `ORW_*` offsets from token artifacts and lowering hints.
pub fn generate_reserved_word_tables(
    artifacts: &TokenArtifacts,
    lowering: &RwLoweringInput,
    codec: &RwSymbolCodec,
) -> ReservedWordTableArtifacts {
    let alpha_tokens = alpha_reserved_tokens(artifacts);
    let mut buckets = Vec::new();
    let mut flat_blob = Vec::new();
    let mut trw_pointers = Vec::with_capacity(RW_BUCKET_LETTERS.len());
    let mut orw_offsets = BTreeMap::new();

    for letter in RW_BUCKET_LETTERS.chars() {
        let entries = alpha_tokens
            .iter()
            .filter(|entry| {
                entry
                    .spelling
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.eq_ignore_ascii_case(&letter))
            })
            .collect::<Vec<_>>();

        let bucket_offset = u16::try_from(flat_blob.len()).expect("bucket offset fits in u16");
        trw_pointers.push(bucket_offset);

        let first_irw = entries
            .first()
            .map(|entry| artifacts.irw_ids[&entry.irw_name])
            .unwrap_or_else(|| first_irw_for_empty_bucket(letter, &alpha_tokens, artifacts));

        let mut bucket_bytes = Vec::new();
        bucket_bytes.extend_from_slice(&first_irw.to_le_bytes());

        for entry in &entries {
            let orw_name = orw_name_from_irw(&entry.irw_name);
            let entry_offset = u16::try_from(bucket_bytes.len()).expect("entry offset fits in u16");
            let bucket_selector = u16::try_from((u32::from(letter) - u32::from('A')) * 4)
                .expect("bucket selector fits in u16");
            orw_offsets.insert(orw_name.clone(), (bucket_selector << 8) | entry_offset);

            let token_flags = artifacts
                .token_rwf_flags
                .get(&entry.irw_name)
                .copied()
                .unwrap_or(0);
            let entry_lowering = lowering.entries.get(&orw_name).cloned().unwrap_or_default();
            let iop = artifacts
                .mp_irw_to_iop
                .get(artifacts.irw_ids[&entry.irw_name] as usize)
                .copied()
                .filter(|value| *value != 0xFF)
                .or_else(|| iop_for_reserved_spelling(&entry.spelling))
                .unwrap_or(0xFF);
            bucket_bytes.extend(encode_rw_entry(
                &entry.spelling,
                token_flags,
                artifacts.rwf,
                &entry_lowering,
                iop,
                codec,
            ));
        }

        bucket_bytes.push(0);
        flat_blob.extend_from_slice(&bucket_bytes);
        buckets.push(RwBucket {
            letter,
            first_irw,
            bytes: bucket_bytes,
        });
    }

    ReservedWordTableArtifacts {
        buckets,
        trw_pointers,
        flat_blob,
        orw_offsets,
    }
}

/// Collect `DW` symbol names from `prsrwt.asm` in first-appearance order.
pub fn build_rw_symbol_codec_from_prsrwt(prsrwt_text: &str) -> RwSymbolCodec {
    let mut codec = RwSymbolCodec::new();
    for line in prsrwt_text.lines() {
        let trimmed = strip_asm_comment(line).trim();
        let Some(rest) = trimmed
            .get(..2)
            .filter(|prefix| prefix.eq_ignore_ascii_case("dw"))
            .and_then(|_| trimmed.get(2..))
        else {
            continue;
        };
        for item in split_data_items(rest) {
            let item = item.trim();
            if parse_numeric_operand(item, 0).is_ok() {
                continue;
            }
            if parse_iop_symbol(item).is_some() {
                continue;
            }
            if parse_asm_char_literal(item).is_some() {
                continue;
            }
            if !item.is_empty() && !item.eq_ignore_ascii_case("offset") {
                codec.intern(item);
            }
        }
    }
    codec
}

/// Parse golden `t41Rw`…`t59Rw` bucket bytes from `prsrwt.asm`.
pub fn parse_golden_rw_buckets(
    prsrwt_text: &str,
    codec: &RwSymbolCodec,
) -> Result<Vec<RwBucket>, ParseDataError> {
    let mut buckets = Vec::new();
    for letter in RW_BUCKET_LETTERS.chars() {
        let label = format!("t{:02x}Rw", u32::from(letter));
        let bytes = parse_rw_bucket_bytes(prsrwt_text, &label, codec)?;
        let first_irw = if bytes.len() >= 2 {
            u16::from_le_bytes([bytes[0], bytes[1]])
        } else {
            IRW_ALPHA_FIRST
        };
        buckets.push(RwBucket {
            letter,
            first_irw,
            bytes,
        });
    }
    Ok(buckets)
}

/// Build lowering hints by parsing captured `prsrwt.asm` bucket bytes.
pub fn extract_rw_lowering_from_golden(
    prsrwt_text: &str,
    artifacts: &TokenArtifacts,
) -> Result<(RwLoweringInput, RwSymbolCodec), ParseDataError> {
    let codec = build_rw_symbol_codec_from_prsrwt(prsrwt_text);
    let buckets = parse_golden_rw_buckets(prsrwt_text, &codec)?;
    let lowering = extract_rw_lowering_from_buckets(&buckets, artifacts, &codec);
    Ok((lowering, codec))
}

/// Extract per-`ORW_*` lowering hints by decoding golden bucket bytes.
pub fn extract_rw_lowering_from_buckets(
    buckets: &[RwBucket],
    artifacts: &TokenArtifacts,
    codec: &RwSymbolCodec,
) -> RwLoweringInput {
    let mut entries = BTreeMap::new();
    for bucket in buckets {
        let mut offset = 2usize;
        while offset < bucket.bytes.len() && bucket.bytes[offset] != 0 {
            let (orw_name, lowering, consumed) =
                decode_rw_entry(bucket.letter, &bucket.bytes, offset, artifacts.rwf, codec);
            entries.insert(orw_name, lowering);
            offset += consumed;
        }
    }
    RwLoweringInput { entries }
}

/// Compare generated `ORW_*` offsets against `prsorw.inc`.
pub fn compare_orw_offsets(
    tables: &ReservedWordTableArtifacts,
    prsorw_text: &str,
) -> Result<(), TokenParityMismatch> {
    let golden = parse_orw_equates(prsorw_text).map_err(|error| TokenParityMismatch {
        artifact: "prsorw.inc",
        detail: format!("parse error: {error}"),
    })?;

    for (name, &expected) in &tables.orw_offsets {
        match golden.get(name) {
            Some(actual) if i64::from(expected) == *actual => {}
            Some(actual) => {
                return Err(TokenParityMismatch {
                    artifact: "prsorw.inc",
                    detail: format!("{name}: expected {expected}, golden {actual}"),
                });
            }
            None => {
                return Err(TokenParityMismatch {
                    artifact: "prsorw.inc",
                    detail: format!("missing golden equate {name}"),
                });
            }
        }
    }

    for name in golden.keys() {
        if !tables.orw_offsets.contains_key(name) {
            return Err(TokenParityMismatch {
                artifact: "prsorw.inc",
                detail: format!("missing generated equate {name}"),
            });
        }
    }

    Ok(())
}

/// Compare generated bucket bytes against golden `prsrwt.asm` buckets.
pub fn compare_rw_bucket_bytes(
    tables: &ReservedWordTableArtifacts,
    prsrwt_text: &str,
    codec: &RwSymbolCodec,
) -> Result<(), TokenParityMismatch> {
    let golden =
        parse_golden_rw_buckets(prsrwt_text, codec).map_err(|error| TokenParityMismatch {
            artifact: "prsrwt.asm",
            detail: format!("bucket parse error: {error}"),
        })?;

    if golden.len() != tables.buckets.len() {
        return Err(TokenParityMismatch {
            artifact: "prsrwt.asm",
            detail: format!(
                "bucket count mismatch: generated {} vs golden {}",
                tables.buckets.len(),
                golden.len()
            ),
        });
    }

    for (generated, expected) in tables.buckets.iter().zip(golden.iter()) {
        if generated.bytes != expected.bytes {
            return Err(TokenParityMismatch {
                artifact: "prsrwt.asm",
                detail: format!(
                    "bucket t{:02x}Rw mismatch: generated {} bytes, golden {} bytes (first diff at offset {})",
                    generated.letter as u32,
                    generated.bytes.len(),
                    expected.bytes.len(),
                    first_byte_diff(&generated.bytes, &expected.bytes)
                ),
            });
        }
    }

    Ok(())
}

/// Validate generated reserved-word tables against captured `prsorw.inc` / `prsrwt.asm`.
pub fn validate_rw_tables_against_golden(
    tables: &ReservedWordTableArtifacts,
    prsorw_text: &str,
    prsrwt_text: &str,
    codec: &RwSymbolCodec,
) -> Result<(), TokenParityMismatch> {
    compare_orw_offsets(tables, prsorw_text)?;
    compare_rw_bucket_bytes(tables, prsrwt_text, codec)?;
    Ok(())
}

const RW_BUCKET_LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXY";

#[derive(Debug, Clone)]
struct AlphaReservedToken {
    spelling: String,
    irw_name: String,
}

fn first_irw_for_empty_bucket(
    letter: char,
    alpha_tokens: &[AlphaReservedToken],
    artifacts: &TokenArtifacts,
) -> u16 {
    let letter = letter.to_ascii_uppercase();
    for entry in alpha_tokens {
        let Some(first) = entry.spelling.chars().next() else {
            continue;
        };
        if first.to_ascii_uppercase() >= letter {
            return artifacts.irw_ids[&entry.irw_name];
        }
    }
    u16::try_from(artifacts.token_count).expect("token count fits in u16")
}

fn alpha_reserved_tokens(artifacts: &TokenArtifacts) -> Vec<AlphaReservedToken> {
    let orw_names = artifacts
        .orw_names
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    artifacts
        .irw_order
        .iter()
        .filter_map(|irw| {
            let orw = orw_name_from_irw(irw);
            orw_names
                .contains(orw.as_str())
                .then(|| AlphaReservedToken {
                    spelling: orw_display_spelling(&orw),
                    irw_name: irw.clone(),
                })
        })
        .collect()
}

fn orw_display_spelling(orw: &str) -> String {
    let stem = orw.strip_prefix("ORW_").unwrap_or(orw);
    if stem.ends_with('_') {
        format!("{}$", stem.trim_end_matches('_'))
    } else {
        stem.to_string()
    }
}

fn encode_rw_entry(
    spelling: &str,
    token_flags: u8,
    rwf: RwfConstants,
    lowering: &RwEntryLowering,
    iop: u8,
    codec: &RwSymbolCodec,
) -> Vec<u8> {
    let upper = spelling.to_ascii_uppercase();
    let has_dollar = upper.ends_with('$');
    let stem = upper.trim_end_matches('$');
    let suffix = &stem[1..];
    let mut attr = Vec::new();

    if token_flags & rwf.operator != 0 {
        attr.push(rwf.operator);
        if iop != 0xFF {
            attr.push(iop);
        }
    } else {
        let mut flags = token_flags & rwf.no_direct;
        let nstmts = lowering.stmt_entries.len() as u8;
        if nstmts > 0 {
            flags |= nstmts & rwf.nstmts_mask;
        }
        let stmt_cg = nstmts > 1
            || lowering
                .stmt_entries
                .iter()
                .any(|entry| entry.cg_fn.as_ref().is_some_and(|name| name != "0"));
        if stmt_cg {
            flags |= rwf.stmt_cg;
        }
        let func_has_cg = lowering.func_cg_fn.as_ref().is_some_and(|name| name != "0");
        if func_has_cg {
            flags |= rwf.func_cg;
        }
        if lowering.func_offset.is_some() {
            flags |= rwf.func;
        }
        if has_dollar {
            flags |= rwf.str_suffix;
        }
        attr.push(flags);

        if nstmts > 1 {
            if let Some(func_offset) = lowering.func_offset {
                attr.extend_from_slice(&func_offset.to_le_bytes());
            }
            if let Some(am) = &lowering.am_resolver {
                attr.extend_from_slice(&encode_dw_operand(am, codec));
            }
            for stmt in &lowering.stmt_entries {
                attr.extend_from_slice(&stmt.stmt_offset.to_le_bytes());
                attr.extend_from_slice(&encode_dw_operand(
                    stmt.cg_fn.as_deref().unwrap_or("0"),
                    codec,
                ));
                attr.extend_from_slice(&encode_dw_operand(
                    stmt.cg_arg.as_deref().unwrap_or("0"),
                    codec,
                ));
            }
        } else {
            if let Some(func_offset) = lowering.func_offset {
                attr.extend_from_slice(&func_offset.to_le_bytes());
                if func_has_cg {
                    attr.extend_from_slice(&encode_dw_operand(
                        lowering.func_cg_fn.as_deref().unwrap_or("0"),
                        codec,
                    ));
                    attr.extend_from_slice(&encode_dw_operand(
                        lowering.func_cg_arg.as_deref().unwrap_or("0"),
                        codec,
                    ));
                }
            }
            if let Some(stmt) = lowering.stmt_entries.first() {
                attr.extend_from_slice(&stmt.stmt_offset.to_le_bytes());
                if flags & rwf.stmt_cg != 0 {
                    attr.extend_from_slice(&encode_dw_operand(
                        stmt.cg_fn.as_deref().unwrap_or("0"),
                        codec,
                    ));
                    attr.extend_from_slice(&encode_dw_operand(
                        stmt.cg_arg.as_deref().unwrap_or("0"),
                        codec,
                    ));
                }
            }
        }
    }

    let mut out = Vec::new();
    if suffix.len() > 15 || attr.len() > 15 {
        out.push(0xFF);
        out.push(u8::try_from(attr.len()).expect("attr length fits in u8"));
        out.push(u8::try_from(suffix.len()).expect("suffix length fits in u8"));
    } else {
        out.push(((suffix.len() as u8) << 4) | (attr.len() as u8));
    }
    out.extend(suffix.bytes());
    out.extend(attr);
    out
}

fn encode_dw_operand(operand: &str, codec: &RwSymbolCodec) -> [u8; 2] {
    if let Ok(value) = operand.parse::<u16>() {
        return value.to_le_bytes();
    }
    codec
        .lookup(operand)
        .unwrap_or_else(|| {
            panic!("symbol {operand} missing from codec; build codec from golden fixture first")
        })
        .to_le_bytes()
}

fn rw_entry_sizes(bytes: &[u8], start: usize) -> (usize, usize, usize) {
    if bytes[start] == 0xFF {
        let attr_len = usize::from(bytes[start + 1]);
        let suffix_len = usize::from(bytes[start + 2]);
        (suffix_len, attr_len, 3)
    } else {
        let size_byte = bytes[start];
        (
            usize::from(size_byte >> 4),
            usize::from(size_byte & 0x0F),
            1,
        )
    }
}

fn decode_rw_entry(
    bucket_letter: char,
    bytes: &[u8],
    start: usize,
    rwf: RwfConstants,
    codec: &RwSymbolCodec,
) -> (String, RwEntryLowering, usize) {
    let (suffix_len, attr_len, header_len) = rw_entry_sizes(bytes, start);
    let suffix = &bytes[start + header_len..start + header_len + suffix_len];
    let attr = &bytes[start + header_len + suffix_len..start + header_len + suffix_len + attr_len];

    let mut spelling = String::new();
    spelling.push(bucket_letter);
    spelling.extend(suffix.iter().map(|byte| *byte as char));
    if attr
        .first()
        .is_some_and(|flags| flags & rwf.str_suffix != 0)
    {
        spelling.push('$');
    }

    let orw = orw_name_from_spelling(&spelling);
    let lowering = decode_rw_attr(attr, rwf, codec);
    let consumed = header_len + suffix_len + attr_len;
    (orw, lowering, consumed)
}

fn orw_name_from_spelling(spelling: &str) -> String {
    let upper = spelling.to_ascii_uppercase();
    if upper.ends_with('$') {
        format!("ORW_{}_", upper.trim_end_matches('$'))
    } else {
        format!("ORW_{upper}")
    }
}

fn decode_rw_attr(attr: &[u8], rwf: RwfConstants, codec: &RwSymbolCodec) -> RwEntryLowering {
    let mut lowering = RwEntryLowering::default();
    if attr.is_empty() {
        return lowering;
    }

    let flags = attr[0];
    if flags & rwf.operator != 0 {
        return lowering;
    }

    let nstmts = flags & rwf.nstmts_mask;
    let mut index = 1usize;

    if nstmts > 1 {
        if flags & rwf.func != 0 {
            if index + 2 <= attr.len() {
                lowering.func_offset = Some(u16::from_le_bytes([attr[index], attr[index + 1]]));
                index += 2;
            }
        }
        if index + 2 <= attr.len() {
            lowering.am_resolver = Some(decode_dw_operand(&attr[index..index + 2], codec));
            index += 2;
        }
        for _ in 0..nstmts {
            if index + 2 > attr.len() {
                break;
            }
            let stmt_offset = u16::from_le_bytes([attr[index], attr[index + 1]]);
            index += 2;
            let mut cg_fn = None;
            let mut cg_arg = None;
            if flags & rwf.stmt_cg != 0 {
                if index + 2 <= attr.len() {
                    cg_fn = Some(decode_dw_operand(&attr[index..index + 2], codec));
                    index += 2;
                }
                if index + 2 <= attr.len() {
                    cg_arg = Some(decode_dw_operand(&attr[index..index + 2], codec));
                    index += 2;
                }
            }
            lowering.stmt_entries.push(RwStmtLowering {
                stmt_offset,
                cg_fn,
                cg_arg,
            });
        }
        return lowering;
    }

    if flags & rwf.func != 0 {
        if index + 2 <= attr.len() {
            lowering.func_offset = Some(u16::from_le_bytes([attr[index], attr[index + 1]]));
            index += 2;
        }
        if flags & rwf.func_cg != 0 {
            if index + 2 <= attr.len() {
                lowering.func_cg_fn = Some(decode_dw_operand(&attr[index..index + 2], codec));
                index += 2;
            }
            if index + 2 <= attr.len() {
                lowering.func_cg_arg = Some(decode_dw_operand(&attr[index..index + 2], codec));
                index += 2;
            }
        }
    }

    if nstmts > 0 {
        if index + 2 <= attr.len() {
            let stmt_offset = u16::from_le_bytes([attr[index], attr[index + 1]]);
            index += 2;
            let mut cg_fn = None;
            let mut cg_arg = None;
            if flags & rwf.stmt_cg != 0 {
                if index + 2 <= attr.len() {
                    cg_fn = Some(decode_dw_operand(&attr[index..index + 2], codec));
                    index += 2;
                }
                if index + 2 <= attr.len() {
                    cg_arg = Some(decode_dw_operand(&attr[index..index + 2], codec));
                }
            }
            lowering.stmt_entries.push(RwStmtLowering {
                stmt_offset,
                cg_fn,
                cg_arg,
            });
        }
    }

    lowering
}

fn decode_dw_operand(bytes: &[u8], codec: &RwSymbolCodec) -> String {
    let value = u16::from_le_bytes([bytes[0], bytes[1]]);
    if let Some(name) = codec.symbol_name(value) {
        return name.to_string();
    }
    value.to_string()
}

fn parse_rw_bucket_bytes(
    text: &str,
    label: &str,
    codec: &RwSymbolCodec,
) -> Result<Vec<u8>, ParseDataError> {
    let mut bytes = Vec::new();
    let mut in_block = false;
    let mut line_no = 0;

    for line in text.lines() {
        line_no += 1;
        let trimmed = strip_asm_comment(line).trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }

        if !in_block {
            if label_declares(trimmed, label) {
                in_block = true;
            }
            continue;
        }

        if is_label_line(trimmed) {
            break;
        }

        if let Some(rest) = trimmed
            .get(..2)
            .filter(|prefix| prefix.eq_ignore_ascii_case("db"))
            .and_then(|_| trimmed.get(2..))
        {
            for item in split_data_items(rest) {
                bytes.push(parse_mp_operand(item, line_no)? as u8);
            }
            continue;
        }

        let Some(rest) = trimmed
            .get(..2)
            .filter(|prefix| prefix.eq_ignore_ascii_case("dw"))
            .and_then(|_| trimmed.get(2..))
        else {
            continue;
        };

        for item in split_data_items(rest) {
            let value = parse_dw_operand(item, line_no, codec)?;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }

    if !in_block {
        return Err(ParseDataError {
            line: 0,
            message: format!("label {label} not found"),
        });
    }

    Ok(bytes)
}

fn parse_dw_operand(
    item: &str,
    line_no: usize,
    codec: &RwSymbolCodec,
) -> Result<u16, ParseDataError> {
    let item = item.trim();
    if item.is_empty() {
        return Err(ParseDataError {
            line: line_no,
            message: "empty dw operand".to_string(),
        });
    }

    if let Ok(value) = parse_numeric_operand(item, line_no) {
        return u16::try_from(value).map_err(|_| ParseDataError {
            line: line_no,
            message: format!("dw value {value} is out of u16 range"),
        });
    }

    if let Some(value) = parse_iop_symbol(item) {
        return Ok(u16::from(value));
    }

    codec.lookup(item).ok_or_else(|| ParseDataError {
        line: line_no,
        message: format!("unknown dw symbol {item}"),
    })
}

fn first_byte_diff(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right.iter())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| left.len().min(right.len()))
}

fn is_alpha_reserved_word(token: &TokenDef) -> bool {
    token
        .spelling
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
}

fn rwf_flags_from_token(token: &TokenDef, rwf: RwfConstants) -> u8 {
    let mut flags = 0u8;
    for attr in &token.attributes {
        match attr.as_str() {
            "operator" => flags |= rwf.operator,
            "no_direct" => flags |= rwf.no_direct,
            _ => {}
        }
    }
    if token.spelling.ends_with('$') {
        flags |= rwf.str_suffix;
    }
    flags
}

fn spelling_to_byte(spelling: &str) -> u8 {
    let ch = spelling
        .chars()
        .next()
        .expect("token spelling is non-empty");
    u8::try_from(u32::from(ch)).expect("QBasic token spellings fit in u8")
}

fn iop_for_special_token(token: &TokenDef) -> Option<u8> {
    if !token.attributes.iter().any(|attr| attr == "operator") {
        return None;
    }

    match token.name.as_str() {
        "tkIdiv" => Some(17),
        "tkPwr" => Some(22),
        "tkLParen" => Some(23),
        "tkRParen" => Some(1),
        "tkMult" => Some(18),
        "tkAdd" => Some(14),
        "tkMinus" => Some(15),
        "tkDiv" => Some(19),
        "tkLT" => Some(9),
        "tkEQ" => Some(8),
        "tkGT" => Some(10),
        _ => None,
    }
}

fn iop_for_reserved_spelling(spelling: &str) -> Option<u8> {
    match spelling.to_ascii_uppercase().as_str() {
        "IMP" => Some(2),
        "EQV" => Some(3),
        "XOR" => Some(4),
        "OR" => Some(5),
        "AND" => Some(6),
        "NOT" => Some(7),
        "MOD" => Some(16),
        _ => None,
    }
}

/// Parse `ORW_*` equates from `prsorw.inc` text.
pub fn parse_orw_equates(
    text: &str,
) -> Result<BTreeMap<String, i64>, crate::buildprs_artifacts::ParseEquatesError> {
    let all = parse_equates(text)?;
    Ok(all
        .into_iter()
        .filter(|(name, _)| name.starts_with("ORW_"))
        .collect())
}

/// Parse a `label byte` table from `prsrwt.asm`, resolving `IOP_*` symbols.
pub fn parse_mp_byte_table(text: &str, label: &str) -> Result<Vec<u8>, ParseDataError> {
    let values = parse_initializer_values_with_iop(text, label)?;
    values
        .into_iter()
        .map(|value| {
            if (0..=255).contains(&value) {
                Ok(value as u8)
            } else {
                Err(ParseDataError {
                    line: 0,
                    message: format!("byte value {value} is out of range"),
                })
            }
        })
        .collect()
}

fn parse_initializer_values_with_iop(text: &str, label: &str) -> Result<Vec<i64>, ParseDataError> {
    let mut values = Vec::new();
    let mut in_block = false;
    let mut line_no = 0;

    for line in text.lines() {
        line_no += 1;
        let trimmed = strip_asm_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if !in_block {
            if label_declares(trimmed, label) {
                in_block = true;
            }
            continue;
        }

        if is_label_line(trimmed) {
            break;
        }

        let Some(rest) = trimmed
            .get(..2)
            .filter(|prefix| prefix.eq_ignore_ascii_case("db"))
            .and_then(|_| trimmed.get(2..))
        else {
            continue;
        };

        for item in split_data_items(rest) {
            values.push(parse_mp_operand(item, line_no)?);
        }
    }

    if !in_block {
        return Err(ParseDataError {
            line: 0,
            message: format!("label {label} not found"),
        });
    }

    Ok(values)
}

fn parse_mp_operand(item: &str, line_no: usize) -> Result<i64, ParseDataError> {
    let item = item.trim();
    if item.is_empty() {
        return Err(ParseDataError {
            line: line_no,
            message: "empty db operand".to_string(),
        });
    }

    if let Some(value) = parse_iop_symbol(item) {
        return Ok(i64::from(value));
    }

    if let Some(value) = parse_asm_char_literal(item) {
        return Ok(i64::from(value));
    }

    parse_numeric_operand(item, line_no)
}

fn parse_asm_char_literal(item: &str) -> Option<u8> {
    let item = item.trim();
    let quote = item.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    if item.len() < 3 || !item.ends_with(quote) {
        return None;
    }

    let inner = &item[1..item.len() - 1];
    if inner.len() == 1 {
        return u8::try_from(u32::from(inner.chars().next()?)).ok();
    }

    None
}

fn parse_iop_symbol(item: &str) -> Option<u8> {
    match item.to_ascii_uppercase().as_str() {
        "IOP_MARK" => Some(0),
        "IOP_RPAREN" => Some(1),
        "IOP_IMP" => Some(2),
        "IOP_EQV" => Some(3),
        "IOP_XOR" => Some(4),
        "IOP_OR" => Some(5),
        "IOP_AND" => Some(6),
        "IOP_NOT" => Some(7),
        "IOP_EQ" => Some(8),
        "IOP_LT" => Some(9),
        "IOP_GT" => Some(10),
        "IOP_LE" => Some(11),
        "IOP_GE" => Some(12),
        "IOP_NE" => Some(13),
        "IOP_ADD" => Some(14),
        "IOP_MINUS" => Some(15),
        "IOP_MOD" => Some(16),
        "IOP_IDIV" => Some(17),
        "IOP_MULT" => Some(18),
        "IOP_DIV" => Some(19),
        "IOP_PLUS" => Some(20),
        "IOP_UMINUS" => Some(21),
        "IOP_PWR" => Some(22),
        "IOP_LPAREN" => Some(23),
        _ => None,
    }
}

fn parse_numeric_operand(item: &str, line_no: usize) -> Result<i64, ParseDataError> {
    let lower = item.to_ascii_lowercase();
    if let Some(stem) = lower.strip_suffix('h') {
        let digits = if stem.is_empty() { "0" } else { stem };
        return i64::from_str_radix(digits, 16).map_err(|_| ParseDataError {
            line: line_no,
            message: format!("invalid hex literal {item}"),
        });
    }
    if let Some(stem) = lower.strip_suffix('b') {
        return i64::from_str_radix(stem, 2).map_err(|_| ParseDataError {
            line: line_no,
            message: format!("invalid binary literal {item}"),
        });
    }
    item.parse::<i64>().map_err(|_| ParseDataError {
        line: line_no,
        message: format!("invalid numeric literal {item}"),
    })
}

fn strip_asm_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] == b'\'' {
            idx += 1;
            while idx < bytes.len() {
                if bytes[idx] == b'\'' {
                    if idx + 1 < bytes.len() && bytes[idx + 1] == b'\'' {
                        idx += 2;
                        continue;
                    }
                    idx += 1;
                    break;
                }
                idx += 1;
            }
            continue;
        }

        if bytes[idx] == b';' {
            return &line[..idx];
        }

        idx += 1;
    }

    line
}

fn split_data_items(rest: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut idx = 0;
    let bytes = rest.as_bytes();

    while idx < bytes.len() {
        if bytes[idx] == b'\'' {
            idx += 1;
            while idx < bytes.len() {
                if bytes[idx] == b'\'' {
                    if idx + 1 < bytes.len() && bytes[idx + 1] == b'\'' {
                        idx += 2;
                        continue;
                    }
                    idx += 1;
                    break;
                }
                idx += 1;
            }
            continue;
        }

        if bytes[idx] == b',' {
            let item = rest[start..idx].trim();
            if !item.is_empty() {
                items.push(item);
            }
            idx += 1;
            start = idx;
            continue;
        }

        idx += 1;
    }

    let item = rest[start..].trim();
    if !item.is_empty() {
        items.push(item);
    }

    items
}

fn label_declares(line: &str, label: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(name) = parts.next() else {
        return false;
    };
    if !name.eq_ignore_ascii_case(label) {
        return false;
    }
    let Some(keyword) = parts.next() else {
        return false;
    };
    let Some(kind) = parts.next() else {
        return false;
    };
    keyword.eq_ignore_ascii_case("label")
        && (kind.eq_ignore_ascii_case("byte")
            || kind.eq_ignore_ascii_case("word")
            || kind.eq_ignore_ascii_case("dword"))
}

fn is_label_line(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    let _name = fields.next();
    matches!(
        (fields.next(), fields.next()),
        (Some(label), Some(kind))
            if label.eq_ignore_ascii_case("label")
                && (kind.eq_ignore_ascii_case("byte")
                    || kind.eq_ignore_ascii_case("word")
                    || kind.eq_ignore_ascii_case("dword"))
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::buildprs_grammar::{parse_token_decls, parse_token_decls_file};

    const FIXTURE_DIR: &str = "../../frontends/qb/fixtures/buildprs/qbasic-1.1";
    const GRAMMAR_PATH: &str = "../../src/frontend/qb/grammar/qbasbnf.prs";

    fn read_fixture(name: &str) -> String {
        fs::read_to_string(Path::new(FIXTURE_DIR).join(name))
            .unwrap_or_else(|error| panic!("read {FIXTURE_DIR}/{name}: {error}"))
    }

    fn qbasic_11_tokens() -> Vec<TokenDef> {
        parse_token_decls_file(Path::new(GRAMMAR_PATH))
            .expect("vendored qbasbnf.prs tokens should parse")
    }

    #[test]
    fn irw_name_mapping_strips_tk_prefix() {
        assert_eq!(irw_name_from_tk("tkPRINT"), "IRW_PRINT");
        assert_eq!(irw_name_from_tk("tkCHR_"), "IRW_CHR_");
        assert_eq!(irw_name_from_tk("tkEtInteger"), "IRW_EtInteger");
    }

    #[test]
    fn irw_name_keeps_environ_pair_in_plain_first_order() {
        let tokens = vec![
            TokenDef {
                name: "tkENVIRON".to_string(),
                spelling: "ENVIRON".to_string(),
                attributes: Vec::new(),
            },
            TokenDef {
                name: "tkENVIRON_".to_string(),
                spelling: "ENVIRON$".to_string(),
                attributes: Vec::new(),
            },
        ];

        assert_eq!(irw_name_for_token(&tokens[0], &tokens), "IRW_ENVIRON");
        assert_eq!(irw_name_for_token(&tokens[1], &tokens), "IRW_ENVIRON_");
    }

    #[test]
    fn irw_name_uses_plain_and_dollar_suffix_symbols() {
        let tokens = vec![
            TokenDef {
                name: "tkSTRING_".to_string(),
                spelling: "STRING$".to_string(),
                attributes: Vec::new(),
            },
            TokenDef {
                name: "tkSTRING".to_string(),
                spelling: "STRING".to_string(),
                attributes: Vec::new(),
            },
        ];

        assert_eq!(irw_name_for_token(&tokens[0], &tokens), "IRW_STRING_");
        assert_eq!(irw_name_for_token(&tokens[1], &tokens), "IRW_STRING");
    }

    #[test]
    fn generate_token_artifacts_assigns_dense_ids_in_declaration_order() {
        let source = r#"TOKENS:
   tkA ("%"),
   tkB ("+"),
   tkPRINT ("PRINT"),
"#;
        let tokens = parse_token_decls(source).expect("inline tokens");
        let artifacts = generate_token_artifacts(&tokens);

        assert_eq!(artifacts.token_count, 3);
        assert_eq!(artifacts.irw_ids["IRW_A"], 0);
        assert_eq!(artifacts.irw_ids["IRW_B"], 1);
        assert_eq!(artifacts.irw_ids["IRW_PRINT"], 2);
        assert_eq!(artifacts.irw_order, vec!["IRW_A", "IRW_B", "IRW_PRINT"]);
        assert_eq!(
            artifacts.mp_irw_to_char,
            vec![b'%', b'+', b'P'],
            "prefix table includes first min(25, n) grammar tokens"
        );
        assert_eq!(artifacts.mp_irw_to_iop, vec![0xFF, 0xFF, 0xFF]);
        assert_eq!(artifacts.orw_names, vec!["ORW_PRINT"]);
    }

    #[test]
    fn operator_and_no_direct_flags_follow_token_attributes() {
        let source = r#"TOKENS:
   tkAND ("AND", operator),
   tkCOMMON ("COMMON", no_direct),
   tkCHR_ ("CHR$"),
"#;
        let tokens = parse_token_decls(source).expect("inline tokens");
        let artifacts = generate_token_artifacts(&tokens);
        let rwf = RwfConstants::QBASIC_11;

        assert_eq!(artifacts.token_rwf_flags["IRW_AND"], rwf.operator);
        assert_eq!(artifacts.token_rwf_flags["IRW_COMMON"], rwf.no_direct);
        assert_eq!(artifacts.token_rwf_flags["IRW_CHR_"], rwf.str_suffix);
    }

    #[test]
    fn qbasic_11_token_count_matches_ntokens_fixture() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);
        let prstab = read_fixture("prstab.inc");
        let equates = parse_equates(&prstab).expect("prstab equates");

        assert_eq!(artifacts.token_count, 246);
        assert_eq!(equates["NTOKENS"], 246);
        assert_eq!(artifacts.irw_order.len(), 246);
    }

    #[test]
    fn qbasic_11_irw_ids_match_prsirw_fixture() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);
        let prsirw = read_fixture("prsirw.inc");

        compare_irw_equates(&artifacts, &prsirw).expect("IRW ids should match golden fixture");
    }

    #[test]
    fn qbasic_11_selected_irw_mappings_match_fixture() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);

        assert_eq!(artifacts.irw_ids["IRW_NewLine"], 7);
        assert_eq!(artifacts.irw_ids["IRW_PRINT"], 0xAD);
        assert_eq!(artifacts.irw_ids["IRW_ABS"], 25);
        assert_eq!(artifacts.irw_ids["IRW_XOR"], 0xF5);
        assert_eq!(artifacts.tk_to_irw["tkCOMMAND_"], "IRW_COMMAND_");
    }

    #[test]
    fn qbasic_11_rwf_constants_match_prstab_fixture() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);
        let prstab = read_fixture("prstab.inc");

        compare_prstab_constants(&artifacts, &prstab)
            .expect("RWF constants should match golden fixture");
    }

    #[test]
    fn qbasic_11_mp_tables_match_prsrwt_fixture() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);
        let prsrwt = read_fixture("prsrwt.asm");

        compare_mp_tables(&artifacts, &prsrwt).expect("mp tables should match golden fixture");

        assert_eq!(artifacts.mp_irw_to_char[7], 0x0A);
        assert_eq!(artifacts.mp_irw_to_char[8], 0x09);
        assert_eq!(artifacts.mp_irw_to_iop[6], 17);
        assert_eq!(artifacts.mp_irw_to_iop[9], 22);
        assert_eq!(artifacts.mp_irw_to_iop[15], 14);
    }

    #[test]
    fn qbasic_11_orw_inventory_matches_prsorw_fixture() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);
        let prsorw = read_fixture("prsorw.inc");

        compare_orw_inventory(&artifacts, &prsorw).expect("ORW inventory should match fixture");
        assert_eq!(artifacts.orw_names.len(), 221);
        assert_eq!(artifacts.orw_names[0], "ORW_ABS");
        assert_eq!(
            artifacts.orw_names.last().map(String::as_str),
            Some("ORW_XOR")
        );
    }

    #[test]
    fn qbasic_11_full_token_artifact_validation_passes() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);

        validate_against_golden(
            &artifacts,
            &read_fixture("prstab.inc"),
            &read_fixture("prsirw.inc"),
            &read_fixture("prsorw.inc"),
            &read_fixture("prsrwt.asm"),
        )
        .expect("token artifacts should match all golden fixtures");
    }

    #[test]
    fn parse_mp_byte_table_reads_iop_symbols_from_fixture() {
        let prsrwt = read_fixture("prsrwt.asm");
        let iop = parse_mp_byte_table(&prsrwt, "mpIRWtoIOP").expect("mpIRWtoIOP");
        let ch = parse_mp_byte_table(&prsrwt, "mpIRWtoChar").expect("mpIRWtoChar");

        assert_eq!(iop.len(), 25);
        assert_eq!(ch.len(), 25);
        assert_eq!(iop[6], 17);
        assert_eq!(ch[0], b'%');
        assert_eq!(ch[5], b'"');
    }

    #[test]
    fn operator_entries_encode_without_lowering_hints() {
        let source = r#"TOKENS:
   tkAND ("AND", operator),
"#;
        let tokens = parse_token_decls(source).expect("inline tokens");
        let artifacts = generate_token_artifacts(&tokens);
        let tables = generate_reserved_word_tables(
            &artifacts,
            &RwLoweringInput::default(),
            &RwSymbolCodec::new(),
        );

        let bucket = &tables.buckets[0];
        assert_eq!(bucket.letter, 'A');
        assert_eq!(bucket.bytes[2], 0x22);
        assert_eq!(bucket.bytes[3], b'N');
        assert_eq!(bucket.bytes[4], b'D');
        assert_eq!(bucket.bytes[5], 0x80);
        assert_eq!(bucket.bytes[6], 6);
        assert_eq!(tables.orw_offsets["ORW_AND"], 2);
    }

    #[test]
    fn plain_keyword_entries_encode_without_lowering_hints() {
        let source = r#"TOKENS:
   tkACCESS ("ACCESS"),
   tkAS ("AS"),
"#;
        let tokens = parse_token_decls(source).expect("inline tokens");
        let artifacts = generate_token_artifacts(&tokens);
        let tables = generate_reserved_word_tables(
            &artifacts,
            &RwLoweringInput::default(),
            &RwSymbolCodec::new(),
        );

        let bucket = &tables.buckets[0];
        assert_eq!(bucket.bytes[2], 0x51);
        assert_eq!(bucket.bytes[9], 0x11);
        assert_eq!(tables.orw_offsets["ORW_ACCESS"], 2);
        assert_eq!(tables.orw_offsets["ORW_AS"], 9);
    }

    #[test]
    fn golden_lowering_round_trips_rw_bucket_bytes() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);
        let prsrwt = read_fixture("prsrwt.asm");
        let (lowering, codec) = extract_rw_lowering_from_golden(&prsrwt, &artifacts)
            .expect("golden lowering should parse");

        assert!(lowering.entries.contains_key("ORW_PRINT"));
        assert!(lowering.entries["ORW_BLOAD"].stmt_entries[0]
            .cg_fn
            .as_deref()
            .is_some_and(|name| name == "Cg1or2Args"));

        let regenerated = generate_reserved_word_tables(&artifacts, &lowering, &codec);
        compare_rw_bucket_bytes(&regenerated, &prsrwt, &codec)
            .expect("regenerated buckets should match golden bytes");
    }

    #[test]
    fn qbasic_11_orw_offsets_match_prsorw_fixture() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);
        let prsrwt = read_fixture("prsrwt.asm");
        let prsorw = read_fixture("prsorw.inc");
        let (lowering, codec) = extract_rw_lowering_from_golden(&prsrwt, &artifacts)
            .expect("golden lowering should parse");
        let tables = generate_reserved_word_tables(&artifacts, &lowering, &codec);

        compare_orw_offsets(&tables, &prsorw).expect("ORW offsets should match golden fixture");
        assert_eq!(tables.orw_offsets["ORW_ABS"], 0x02);
        assert_eq!(tables.orw_offsets["ORW_PRINT"], 0x3c6f);
        assert_eq!(tables.orw_offsets["ORW_XOR"], 0x5c02);
    }

    #[test]
    fn qbasic_11_rw_buckets_match_prsrwt_fixture() {
        let tokens = qbasic_11_tokens();
        let artifacts = generate_token_artifacts(&tokens);
        let prsrwt = read_fixture("prsrwt.asm");
        let (lowering, codec) = extract_rw_lowering_from_golden(&prsrwt, &artifacts)
            .expect("golden lowering should parse");
        let tables = generate_reserved_word_tables(&artifacts, &lowering, &codec);

        validate_rw_tables_against_golden(&tables, &read_fixture("prsorw.inc"), &prsrwt, &codec)
            .expect("reserved-word tables should match golden fixtures");
        assert_eq!(tables.buckets.len(), 25);
        assert_eq!(tables.trw_pointers.len(), 25);
        assert!(!tables.flat_blob.is_empty());
    }
}

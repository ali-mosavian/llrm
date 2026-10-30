//! Reads the text of `X86GenDAGISel.inc`: the byte tables and the predicate
//! lists.
//!
//! tblgen writes the tables as C++ initialisers whose entries name enum values,
//! so the bytes come from the symbol values in [`Ids`]
//! (`tools/dagisel/regen.sh` takes them from the pin's headers). The number of
//! bytes is checked against the total tblgen states.

use std::collections::BTreeMap;

/// `name value` pairs: every symbol the tables name.
#[derive(Default, Debug)]
pub struct Ids(pub BTreeMap<String, i64>);

impl Ids {
    pub fn parse(text: &str) -> Result<Ids, String> {
        let mut map = BTreeMap::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (name, value) = line.rsplit_once(' ').ok_or_else(|| format!("bad ids line `{line}`"))?;
            let value = value.parse().map_err(|_| format!("bad ids value in `{line}`"))?;
            map.insert(name.to_string(), value);
        }
        Ok(Ids(map))
    }

    pub fn get(
        &self,
        name: &str,
    ) -> Result<i64, String> {
        self.0.get(name).copied().ok_or_else(|| format!("symbol `{name}` has no value in the ids file"))
    }
}

/// The two byte tables and the C++ the generated switches carry.
pub struct Inc {
    pub matcher_table: Vec<u8>,
    pub operand_lists: Vec<u8>,
    pub pattern_predicates: Vec<String>,
    /// The `// Predicate_X` names of each `CheckNodePredicate` case, by number.
    pub node_predicates: Vec<Vec<String>>,
    /// Each `CheckComplexPattern` case: the selector function it calls and how
    /// many results.
    pub complex_patterns: Vec<ComplexPattern>,
    /// The `return` line of each `RunSDNodeXForm` case.
    pub xforms: Vec<String>,
}

pub struct ComplexPattern {
    pub function: String,
    pub results: usize,
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        match rest[at..].find("*/") {
            Some(end) => rest = &rest[at + end + 2..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The text between `marker` and the closing `};`, and what follows the closing
/// line.
fn initializer<'a>(
    text: &'a str,
    marker: &str,
) -> Result<(&'a str, &'a str), String> {
    let at = text.find(marker).ok_or_else(|| format!("no `{marker}` in the .inc"))?;
    let body = &text[at + marker.len()..];
    let end = body.find("};").ok_or_else(|| format!("`{marker}` is not closed"))?;
    let after = &body[end + 2..];
    Ok((&body[..end], after))
}

/// One initialiser entry as bytes: a number, an OR of numbers and symbols, or
/// `TARGET_VAL(sym)`.
fn entry_bytes(
    entry: &str,
    ids: &Ids,
) -> Result<Vec<u8>, String> {
    let entry = entry.trim();
    if let Some(inner) = entry.strip_prefix("TARGET_VAL(") {
        let symbol = inner.strip_suffix(')').ok_or_else(|| format!("bad entry `{entry}`"))?;
        let value = term(symbol.trim(), ids)?;
        if !(0..=0xffff).contains(&value) {
            return Err(format!("`{entry}` does not fit two bytes"));
        }
        return Ok(vec![value as u8, (value >> 8) as u8]);
    }
    let mut value = 0i64;
    for part in entry.split('|') {
        value |= term(part.trim(), ids)?;
    }
    if !(0..=255).contains(&value) {
        return Err(format!("`{entry}` does not fit a byte"));
    }
    Ok(vec![value as u8])
}

fn term(
    term: &str,
    ids: &Ids,
) -> Result<i64, String> {
    if term.bytes().all(|b| b.is_ascii_digit()) && !term.is_empty() {
        return term.parse().map_err(|_| format!("bad number `{term}`"));
    }
    ids.get(term)
}

fn bytes_of(
    body: &str,
    ids: &Ids,
) -> Result<Vec<u8>, String> {
    let body = strip_comments(body);
    let mut out = Vec::new();
    for entry in body.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        out.extend(entry_bytes(entry, ids)?);
    }
    Ok(out)
}

pub fn parse(
    text: &str,
    ids: &Ids,
) -> Result<Inc, String> {
    let (table_body, after) = initializer(text, "MatcherTable[] = {")?;
    let matcher_table = bytes_of(table_body, ids)?;
    let stated = after
        .lines()
        .next()
        .and_then(|l| l.split("Total Array size is ").nth(1))
        .and_then(|l| l.split(' ').next())
        .and_then(|n| n.parse::<usize>().ok());
    if let Some(stated) = stated {
        if stated != matcher_table.len() {
            return Err(format!("tblgen states {stated} table bytes, read {}", matcher_table.len()));
        }
    }
    let (lists_body, after) = initializer(after, "OperandLists[] = {")?;
    let operand_lists = bytes_of(lists_body, ids)?;
    let pattern_predicates = pattern_predicates(after)?;
    let node_predicates = node_predicates(after)?;
    let complex_patterns = complex_patterns(after)?;
    let xforms = xforms(after)?;
    Ok(Inc { matcher_table, operand_lists, pattern_predicates, node_predicates, complex_patterns, xforms })
}

/// The expression of each `case N: return <expr>;` in `CheckPatternPredicate`,
/// by `N`.
fn pattern_predicates(after: &str) -> Result<Vec<String>, String> {
    let at = after.find("CheckPatternPredicate(unsigned PredNo) const\n").ok_or("no CheckPatternPredicate")?;
    let body = &after[at..];
    let end = body.find("\n}\n").ok_or("CheckPatternPredicate is not closed")?;
    let mut out = Vec::new();
    for line in body[..end].lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("case ") else { continue };
        let (number, expr) = rest.split_once(": return ").ok_or_else(|| format!("bad predicate line `{line}`"))?;
        let number: usize = number.parse().map_err(|_| format!("bad predicate number in `{line}`"))?;
        if number != out.len() {
            return Err(format!("predicate {number} out of order"));
        }
        out.push(expr.strip_suffix(';').ok_or_else(|| format!("bad predicate line `{line}`"))?.to_string());
    }
    Ok(out)
}

/// The body of the generated function that starts at `signature`, up to its
/// closing brace.
fn function_body<'a>(
    after: &'a str,
    signature: &str,
) -> Result<&'a str, String> {
    let at = after.find(signature).ok_or_else(|| format!("no `{signature}` in the .inc"))?;
    let body = &after[at..];
    let end = body.find("\n}\n").ok_or_else(|| format!("`{signature}` is not closed"))?;
    Ok(&body[..end])
}

/// The numbers of the `  case N:` lines of a generated switch, with the lines
/// of each case.
fn cases(body: &str) -> Vec<(usize, Vec<&str>)> {
    let mut out: Vec<(usize, Vec<&str>)> = Vec::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("  case ") {
            if let Some(number) = rest.split(':').next().and_then(|n| n.parse().ok()) {
                out.push((number, vec![line]));
                continue;
            }
        }
        if let Some((_, lines)) = out.last_mut() {
            lines.push(line);
        }
    }
    out
}

fn node_predicates(after: &str) -> Result<Vec<Vec<String>>, String> {
    let body = function_body(after, "CheckNodePredicate(SDValue Op, unsigned PredNo) const\n")?;
    let mut out = Vec::new();
    for (number, lines) in cases(body) {
        if number != out.len() {
            return Err(format!("node predicate {number} out of order"));
        }
        out.push(lines.iter().filter_map(|l| l.trim().strip_prefix("// Predicate_").map(str::to_string)).collect());
    }
    Ok(out)
}

fn complex_patterns(after: &str) -> Result<Vec<ComplexPattern>, String> {
    let body = function_body(
        after,
        "CheckComplexPattern(SDNode *Root, SDNode *Parent,\n      SDValue N, unsigned PatternNo,\n      SmallVectorImpl<std::pair<SDValue, SDNode *>> &Result)\n",
    )?;
    let mut out = Vec::new();
    for (number, lines) in cases(body) {
        if number != out.len() {
            return Err(format!("complex pattern {number} out of order"));
        }
        let results = lines
            .iter()
            .find_map(|l| l.trim().strip_prefix("Result.resize(NextRes+"))
            .and_then(|r| r.split(')').next())
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| format!("complex pattern {number}: no result count"))?;
        let function = lines
            .iter()
            .find_map(|l| l.trim().strip_prefix("return "))
            .and_then(|r| r.split('(').next())
            .ok_or_else(|| format!("complex pattern {number}: no call"))?
            .to_string();
        out.push(ComplexPattern { function, results });
    }
    Ok(out)
}

fn xforms(after: &str) -> Result<Vec<String>, String> {
    let body = function_body(after, "RunSDNodeXForm(SDValue V, unsigned XFormNo)\n")?;
    let mut out = Vec::new();
    for (number, lines) in cases(body) {
        if number != out.len() {
            return Err(format!("xform {number} out of order"));
        }
        let call = lines.iter().find_map(|l| l.trim().strip_prefix("return ")).unwrap_or("");
        out.push(call.trim_end_matches(';').to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Ids {
        Ids::parse("OPC_Scope 0\nOPC_RecordNode 1\nOPFL_Chain 1\nOPFL_MemRefs 8\nISD::STORE 317\nMVT::i32 7\n").unwrap()
    }

    #[test]
    fn reads_vbr_pieces_symbols_and_two_byte_values() {
        let text = "static const uint8_t MatcherTable[] = {\n OPC_Scope, 34|128,104, /*13346*/ TARGET_VAL(ISD::STORE), MVT::i32, OPFL_Chain|OPFL_MemRefs,\n }; // Total Array size is 7 bytes\n\n static const uint8_t OperandLists[] = {\n /* 0 */ 0, 0,\n };\n";
        let err = parse(text, &ids()).err().unwrap();
        assert!(err.contains("CheckPatternPredicate"), "{err}");
        let (body, _) = initializer(text, "MatcherTable[] = {").unwrap();
        assert_eq!(bytes_of(body, &ids()).unwrap(), [0, 34 | 128, 104, 61, 1, 7, 9]);
    }

    #[test]
    fn a_wrong_total_is_refused() {
        let text = "static const uint8_t MatcherTable[] = {\n OPC_Scope,\n }; // Total Array size is 5 bytes\n static const uint8_t OperandLists[] = {\n };\n";
        let err = parse(text, &ids()).err().unwrap();
        assert!(err.contains("states 5"), "{err}");
    }

    #[test]
    fn an_unknown_symbol_is_refused() {
        let text = "static const uint8_t MatcherTable[] = {\n MVT::nope,\n };\n";
        assert!(parse(text, &ids()).err().unwrap().contains("MVT::nope"));
    }
}

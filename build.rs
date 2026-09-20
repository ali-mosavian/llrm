use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::PathBuf;

use buildprs::buildprs_generator::{
    generate_tables_from_grammar, parse_opcode_equates_from_peropcod,
};
use buildprs::buildprs_grammar::parse_grammar_file;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MappingKind {
    Emit,
    External,
    Dispatch,
}

#[derive(Clone, Debug)]
struct ActionMapping {
    kind: MappingKind,
    identity: String,
    action: String,
    operand: Option<String>,
    shape: Option<String>,
    retains: Vec<String>,
}

/// A dialect addition that is intentionally outside the recovered QBasic
/// grammar.  These live in their own checked input so the base table stays a
/// faithful artifact of qbasic-port while later dialects can add syntax.
#[derive(Clone, Debug)]
struct DialectExtension {
    identity: String,
    action: String,
    pattern: Vec<ExtensionPattern>,
    tail: String,
}

#[derive(Clone, Debug)]
enum ExtensionPattern {
    GrammarToken(String),
    Keyword(String),
    Label,
    Identifier,
    StringLiteral,
}

/// Source facts that the hand-written syntax AST or its statement builder can
/// actually preserve today.  The action schema is a checked contract, not an
/// AST-field generator: adding a claim requires adding the syntax field first.
const AST_SOURCE_FACTS: &[&str] = &[
    "array",
    "bounds",
    "by_value",
    "condition",
    "declaration",
    "dynamic",
    "else_branch",
    "expression",
    "fixed_length",
    "indices",
    "member_path",
    "name",
    "name_or_number",
    "operands",
    "operator",
    "procedure_kind",
    "ranges",
    "result_type",
    "segmented",
    "separator",
    "shared",
    "span",
    "storage",
    "target",
    "then_branch",
    "type_name",
    "value",
];

fn main() -> io::Result<()> {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let grammar = root.join("src/frontend/qb/grammar/qbasbnf.prs");
    let peropcod = root.join("src/frontend/qb/grammar/peropcod.txt");
    let action_schema = root.join("src/frontend/qb/grammar/ast-actions.toml");
    let extension_schema = root.join("src/frontend/qb/grammar/dialect-extensions.toml");
    println!("cargo:rerun-if-changed={}", grammar.display());
    println!("cargo:rerun-if-changed={}", peropcod.display());
    println!("cargo:rerun-if-changed={}", action_schema.display());
    println!("cargo:rerun-if-changed={}", extension_schema.display());

    let grammar_file = parse_grammar_file(&grammar)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", grammar.display()));
    let opcode_source = fs::read_to_string(&peropcod)?;
    let tables = generate_tables_from_grammar(&grammar_file, &opcode_source)
        .unwrap_or_else(|error| panic!("failed to generate QB parser tables: {error}"));
    let tokens = buildprs::buildprs_tokens::generate_token_artifacts(&grammar_file.tokens);
    let opcodes = parse_opcode_equates_from_peropcod(&opcode_source);
    let mappings = parse_action_schema(&fs::read_to_string(&action_schema)?)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", action_schema.display()));
    let extensions = parse_dialect_extensions(&fs::read_to_string(&extension_schema)?)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", extension_schema.display()));
    validate_action_schema(&mappings, &tables, &tokens, &opcodes)
        .unwrap_or_else(|error| panic!("invalid {}: {error}", action_schema.display()));
    validate_dialect_extensions(&extensions, &tokens)
        .unwrap_or_else(|error| panic!("invalid {}: {error}", extension_schema.display()));

    let output = render(
        &grammar_file,
        &tables,
        &tokens,
        &opcodes,
        &mappings,
        &extensions,
    );
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out.join("qbasic_parser_tables.rs"), output)
}

fn render(
    grammar: &buildprs::buildprs_grammar::GrammarFile,
    tables: &buildprs::buildprs_generator::GeneratedParserTables,
    tokens: &buildprs::buildprs_tokens::TokenArtifacts,
    opcodes: &std::collections::BTreeMap<String, u16>,
    mappings: &[ActionMapping],
    extensions: &[DialectExtension],
) -> String {
    let mut out = String::from("// Generated from the vendored QBasic 1.1 qbasbnf.prs.\n");
    out.push_str("pub const T_STATE: &[u8] = &[\n");
    for row in tables.state.chunks(16) {
        out.push_str("    ");
        for value in row {
            write!(out, "0x{value:02X}, ").unwrap();
        }
        out.push('\n');
    }
    out.push_str("];\n\npub const T_INT_NT_DISP: &[u16] = &[\n    ");
    for (index, value) in tables.dispatch.int_nt_disp.iter().enumerate() {
        if index != 0 && index % 12 == 0 {
            out.push_str("\n    ");
        }
        write!(out, "{value}, ").unwrap();
    }
    out.push_str("\n];\n\n");
    render_strings(&mut out, "T_EXT_NT_DISP", &tables.dispatch.ext_nt_disp);
    render_strings(&mut out, "T_EXT_NT_HELP", &tables.dispatch.ext_nt_help);

    let mut statement_dispatch = tables
        .statement_offset_list
        .iter()
        .filter_map(|(name, offset)| {
            let irw_name = tokens.tk_to_irw.get(name)?;
            Some((*tokens.irw_ids.get(irw_name)?, *offset))
        })
        .collect::<Vec<_>>();
    statement_dispatch.sort_by_key(|&(irw, _)| irw);
    render_pairs(&mut out, "T_STMT_DISPATCH", &statement_dispatch);

    let mut function_dispatch = tables
        .function_offsets
        .iter()
        .filter_map(|(name, offset)| {
            let irw_name = tokens.tk_to_irw.get(name)?;
            Some((*tokens.irw_ids.get(irw_name)?, *offset))
        })
        .collect::<Vec<_>>();
    function_dispatch.sort_by_key(|&(irw, _)| irw);
    render_pairs(&mut out, "T_FUNC_DISPATCH", &function_dispatch);

    out.push_str("pub const TOKENS: &[(&str, &str, u16, u8)] = &[\n");
    let by_name = grammar
        .tokens
        .iter()
        .map(|token| (token.name.as_str(), token))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut ordered = tokens
        .tk_to_irw
        .iter()
        .filter_map(|(tk, irw)| {
            let token = by_name.get(tk.as_str())?;
            let id = *tokens.irw_ids.get(irw)?;
            let flags = *tokens.token_rwf_flags.get(irw).unwrap_or(&0);
            Some((id, tk, token.spelling.as_str(), flags))
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|&(id, _, _, _)| id);
    for (id, name, spelling, flags) in ordered {
        writeln!(
            out,
            "    ({:?}, {:?}, {}, 0x{:02X}),",
            name, spelling, id, flags
        )
        .unwrap();
    }
    out.push_str("];\n\n");

    render_ast_actions(&mut out, mappings, tokens, opcodes);
    render_dialect_extensions(&mut out, extensions, tokens);
    out
}

fn parse_dialect_extensions(source: &str) -> Result<Vec<DialectExtension>, String> {
    let mut extensions = Vec::new();
    let mut fields = std::collections::BTreeMap::<String, String>::new();
    let finish = |line_number: usize,
                  fields: &mut std::collections::BTreeMap<String, String>,
                  extensions: &mut Vec<DialectExtension>|
     -> Result<(), String> {
        if fields.is_empty() {
            return Ok(());
        }
        let mut required = |name: &str| {
            fields
                .remove(name)
                .ok_or_else(|| format!("extension ending at line {line_number} requires {name}"))
        };
        let identity = required("identity")?;
        let action = required("action")?;
        let pattern = required("pattern")?
            .split(',')
            .map(str::trim)
            .map(|item| {
                item.strip_prefix("token:")
                    .map(|name| ExtensionPattern::GrammarToken(name.to_string()))
                    .or_else(|| {
                        item.strip_prefix("keyword:")
                            .map(|word| ExtensionPattern::Keyword(word.to_ascii_uppercase()))
                    })
                    .or_else(|| (item == "capture:label").then_some(ExtensionPattern::Label))
                    .or_else(|| {
                        (item == "capture:identifier").then_some(ExtensionPattern::Identifier)
                    })
                    .or_else(|| {
                        (item == "capture:string").then_some(ExtensionPattern::StringLiteral)
                    })
                    .ok_or_else(|| format!("extension {identity} has invalid pattern item {item}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tail = fields.remove("tail").unwrap_or_else(|| "none".into());
        if !fields.is_empty() {
            let unknown = fields.keys().next().expect("not empty");
            return Err(format!("extension {identity} has unknown field {unknown}"));
        }
        extensions.push(DialectExtension {
            identity,
            action,
            pattern,
            tail,
        });
        Ok(())
    };

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[extension]]" {
            finish(line_number, &mut fields, &mut extensions)?;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {line_number}: expected key = value"));
        };
        let value = value.trim();
        if !(value.starts_with('"') && value.ends_with('"')) {
            return Err(format!("line {line_number}: values must be quoted"));
        }
        let key = key.trim();
        if !matches!(key, "identity" | "action" | "pattern" | "tail") {
            return Err(format!("line {line_number}: unknown field {key}"));
        }
        if fields
            .insert(key.to_string(), value[1..value.len() - 1].to_string())
            .is_some()
        {
            return Err(format!("line {line_number}: duplicate field {key}"));
        }
    }
    finish(source.lines().count(), &mut fields, &mut extensions)?;
    Ok(extensions)
}

fn validate_dialect_extensions(
    extensions: &[DialectExtension],
    tokens: &buildprs::buildprs_tokens::TokenArtifacts,
) -> Result<(), String> {
    let mut identities = std::collections::BTreeSet::new();
    let mut actions = std::collections::BTreeSet::new();
    for extension in extensions {
        if !identities.insert(extension.identity.as_str()) {
            return Err(format!(
                "duplicate extension identity {}",
                extension.identity
            ));
        }
        if !actions.insert(extension.action.as_str()) {
            return Err(format!("duplicate extension action {}", extension.action));
        }
        if extension.pattern.is_empty() {
            return Err(format!(
                "extension {} has an empty pattern",
                extension.identity
            ));
        }
        if !extension.action.chars().enumerate().all(|(index, ch)| {
            ch.is_ascii_alphanumeric() && (index != 0 || ch.is_ascii_uppercase())
        }) {
            return Err(format!(
                "extension action {} is not a Rust enum variant",
                extension.action
            ));
        }
        for item in &extension.pattern {
            match item {
                ExtensionPattern::GrammarToken(name) if !tokens.tk_to_irw.contains_key(name) => {
                    return Err(format!(
                        "extension {} names unknown grammar token {name}",
                        extension.identity
                    ));
                }
                ExtensionPattern::Keyword(keyword)
                    if keyword.is_empty()
                        || !keyword.bytes().all(|byte| byte.is_ascii_uppercase()) =>
                {
                    return Err(format!(
                        "extension {} has invalid keyword {keyword}",
                        extension.identity
                    ));
                }
                ExtensionPattern::Label => {}
                ExtensionPattern::Identifier | ExtensionPattern::StringLiteral => {}
                _ => {}
            }
        }
        if !matches!(extension.tail.as_str(), "none" | "function_signature") {
            return Err(format!(
                "extension {} has unknown tail {}",
                extension.identity, extension.tail
            ));
        }
    }
    Ok(())
}

fn parse_action_schema(source: &str) -> Result<Vec<ActionMapping>, String> {
    let mut mappings = Vec::new();
    let mut current: Option<ActionMapping> = None;
    for (line_number, source_line) in source.lines().enumerate() {
        let line = source_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("[[") && line.ends_with("]]") {
            if let Some(mapping) = current.take() {
                finish_mapping(mapping, &mut mappings, line_number)?;
            }
            let section = &line[2..line.len() - 2];
            let kind = match section {
                "emit" => MappingKind::Emit,
                "external" => MappingKind::External,
                "dispatch" => MappingKind::Dispatch,
                _ => {
                    return Err(format!(
                        "line {}: unknown section {section}",
                        line_number + 1
                    ));
                }
            };
            current = Some(ActionMapping {
                kind,
                identity: String::new(),
                action: String::new(),
                operand: None,
                shape: None,
                retains: Vec::new(),
            });
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("line {}: expected key = value", line_number + 1));
        };
        let value = value.trim();
        if !(value.starts_with('"') && value.ends_with('"')) {
            return Err(format!("line {}: values must be quoted", line_number + 1));
        }
        let value = &value[1..value.len() - 1];
        let Some(mapping) = current.as_mut() else {
            return Err(format!("line {}: field outside a mapping", line_number + 1));
        };
        match key.trim() {
            "identity" => mapping.identity = value.into(),
            "action" => mapping.action = value.into(),
            "operand" => mapping.operand = Some(value.into()),
            "shape" => mapping.shape = Some(value.into()),
            "retains" => {
                mapping.retains = value
                    .split(',')
                    .map(str::trim)
                    .filter(|fact| !fact.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            other => return Err(format!("line {}: unknown field {other}", line_number + 1)),
        }
    }
    if let Some(mapping) = current {
        finish_mapping(mapping, &mut mappings, source.lines().count())?;
    }
    Ok(mappings)
}

fn finish_mapping(
    mapping: ActionMapping,
    mappings: &mut Vec<ActionMapping>,
    line_number: usize,
) -> Result<(), String> {
    if mapping.identity.is_empty() || mapping.action.is_empty() {
        return Err(format!(
            "mapping ending at line {line_number} requires identity and action"
        ));
    }
    mappings.push(mapping);
    Ok(())
}

fn validate_action_schema(
    mappings: &[ActionMapping],
    tables: &buildprs::buildprs_generator::GeneratedParserTables,
    tokens: &buildprs::buildprs_tokens::TokenArtifacts,
    opcodes: &std::collections::BTreeMap<String, u16>,
) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for mapping in mappings {
        if !seen.insert((mapping.kind as u8, mapping.identity.as_str())) {
            return Err(format!(
                "duplicate {:?} identity {}",
                mapping.kind, mapping.identity
            ));
        }
        if !mapping.action.chars().enumerate().all(|(index, ch)| {
            ch.is_ascii_alphanumeric() && (index != 0 || ch.is_ascii_uppercase())
        }) {
            return Err(format!(
                "action {} is not a Rust enum variant",
                mapping.action
            ));
        }
        match mapping.kind {
            MappingKind::Emit if !opcodes.contains_key(&mapping.identity) => {
                return Err(format!("unknown EMIT identity {}", mapping.identity));
            }
            MappingKind::External if !tables.dispatch.ext_nt_disp.contains(&mapping.identity) => {
                return Err(format!("unknown external NT {}", mapping.identity));
            }
            MappingKind::Dispatch if !tokens.tk_to_irw.contains_key(&mapping.identity) => {
                return Err(format!("unknown dispatch token {}", mapping.identity));
            }
            _ => {}
        }
        if mapping.kind == MappingKind::External && mapping.retains.is_empty() {
            return Err(format!(
                "external action {} must declare retained source facts",
                mapping.identity
            ));
        }
        for fact in &mapping.retains {
            if !AST_SOURCE_FACTS.contains(&fact.as_str()) {
                return Err(format!(
                    "external action {} claims source fact {fact}, but the hand-written AST does not model it",
                    mapping.identity
                ));
            }
        }
    }
    Ok(())
}

fn render_ast_actions(
    out: &mut String,
    mappings: &[ActionMapping],
    tokens: &buildprs::buildprs_tokens::TokenArtifacts,
    opcodes: &std::collections::BTreeMap<String, u16>,
) {
    let emit_variants = mappings
        .iter()
        .filter(|mapping| matches!(mapping.kind, MappingKind::Emit | MappingKind::Dispatch))
        .map(|mapping| mapping.action.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let external_variants = mappings
        .iter()
        .filter(|mapping| mapping.kind == MappingKind::External)
        .map(|mapping| mapping.action.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let shapes = mappings
        .iter()
        .filter_map(|mapping| mapping.shape.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    let operand_actions = mappings
        .iter()
        .filter(|mapping| mapping.operand.is_some())
        .map(|mapping| mapping.action.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let operand_shapes = mappings
        .iter()
        .filter(|mapping| mapping.operand.is_some())
        .filter_map(|mapping| mapping.shape.as_deref())
        .collect::<std::collections::BTreeSet<_>>();

    out.push_str("#[derive(Clone, Debug, Eq, PartialEq)]\npub enum AstAction {\n");
    out.push_str("    Mark { slot: u8, token: usize },\n    OperandPlaceholder,\n    OperandValue,\n    Unsupported(&'static str),\n");
    for variant in emit_variants {
        if operand_actions.contains(variant) {
            writeln!(out, "    {variant}(&'static str),").unwrap();
        } else {
            writeln!(out, "    {variant},").unwrap();
        }
    }
    out.push_str("}\n\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum StatementShape {\n");
    for shape in shapes {
        if operand_shapes.contains(shape) {
            writeln!(out, "    {shape}(&'static str),").unwrap();
        } else {
            writeln!(out, "    {shape},").unwrap();
        }
    }
    out.push_str("}\n\n");
    out.push_str("impl AstAction {\n    pub fn statement_shape(&self) -> Option<StatementShape> {\n        match self {\n");
    let mut rendered_actions = std::collections::BTreeSet::new();
    for mapping in mappings.iter().filter(|mapping| mapping.shape.is_some()) {
        if !rendered_actions.insert(mapping.action.as_str()) {
            continue;
        }
        let action = render_action_pattern(mapping);
        let shape = render_shape(mapping);
        writeln!(out, "            {action} => Some({shape}),").unwrap();
    }
    out.push_str("            _ => None,\n        }\n    }\n}\n\n");

    out.push_str("fn one_emit_action(id: u16) -> AstAction {\n    match id {\n");
    for mapping in mappings
        .iter()
        .filter(|mapping| mapping.kind == MappingKind::Emit)
    {
        let id = opcodes[&mapping.identity];
        let action = render_action_value(mapping);
        writeln!(out, "        {id} => {action},").unwrap();
    }
    let mut opcode_identities = std::collections::BTreeMap::<u16, &str>::new();
    for (identity, id) in opcodes {
        opcode_identities.entry(*id).or_insert(identity.as_str());
    }
    out.push_str("        other => unsupported_emit_action(other),\n    }\n}\n\n");
    out.push_str("fn unsupported_emit_action(id: u16) -> AstAction {\n    match id {\n");
    for (id, identity) in opcode_identities {
        writeln!(out, "        {id} => AstAction::Unsupported({identity:?}),").unwrap();
    }
    out.push_str("        _ => AstAction::OperandValue,\n    }\n}\n\n");
    out.push_str("pub fn emit_actions(word: u16) -> [Option<AstAction>; 2] {\n    if word == u16::MAX {\n        return [Some(AstAction::OperandPlaceholder), None];\n    }\n    let primary = one_emit_action(word & 0x03ff);\n    let secondary_id = word >> 10;\n    let secondary = (secondary_id != 0).then(|| one_emit_action(secondary_id));\n    [Some(primary), secondary]\n}\n\n");

    out.push_str("pub fn dispatch_action(token: u16) -> Option<AstAction> {\n    match token {\n");
    for mapping in mappings
        .iter()
        .filter(|mapping| mapping.kind == MappingKind::Dispatch)
    {
        let irw = &tokens.tk_to_irw[&mapping.identity];
        let id = tokens.irw_ids[irw];
        let action = render_action_value(mapping);
        writeln!(out, "        {id} => Some({action}),").unwrap();
    }
    out.push_str("        _ => None,\n    }\n}\n\n");

    out.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum ExternalAction {\n");
    for variant in external_variants {
        writeln!(out, "    {variant},").unwrap();
    }
    out.push_str(
        "}\n\npub fn external_action(name: &str) -> Option<ExternalAction> {\n    match name {\n",
    );
    for mapping in mappings
        .iter()
        .filter(|mapping| mapping.kind == MappingKind::External)
    {
        writeln!(
            out,
            "        {:?} => Some(ExternalAction::{}),",
            mapping.identity, mapping.action
        )
        .unwrap();
    }
    out.push_str("        _ => None,\n    }\n}\n\n");
    out.push_str("pub const EXTERNAL_RETAINED_FACTS: &[(&str, &[&str])] = &[\n");
    for mapping in mappings
        .iter()
        .filter(|mapping| mapping.kind == MappingKind::External)
    {
        write!(out, "    ({:?}, &[", mapping.identity).unwrap();
        for fact in &mapping.retains {
            write!(out, "{:?}, ", fact).unwrap();
        }
        out.push_str("]),\n");
    }
    out.push_str("];\n\npub const SOURCE_FACT_VOCABULARY: &[&str] = &[\n");
    for fact in AST_SOURCE_FACTS {
        writeln!(out, "    {fact:?},").unwrap();
    }
    out.push_str("];\n");
}

fn render_dialect_extensions(
    out: &mut String,
    extensions: &[DialectExtension],
    tokens: &buildprs::buildprs_tokens::TokenArtifacts,
) {
    out.push_str(
        "\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum GeneratedExtensionAction {\n",
    );
    for extension in extensions {
        writeln!(out, "    {},", extension.action).unwrap();
    }
    out.push_str("}\n\n");
    out.push_str(
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum ExtensionPatternToken {\n    Grammar(u16),\n    Keyword(&'static str),\n    Label,\n    Identifier,\n    StringLiteral,\n}\n\n",
    );
    out.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum ExtensionTail {\n    None,\n    FunctionSignature,\n}\n\n");
    out.push_str("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub struct ExtensionSpec {\n    pub identity: &'static str,\n    pub action: GeneratedExtensionAction,\n    pub pattern: &'static [ExtensionPatternToken],\n    pub tail: ExtensionTail,\n}\n\n");
    for (index, extension) in extensions.iter().enumerate() {
        writeln!(
            out,
            "const EXTENSION_PATTERN_{index}: &[ExtensionPatternToken] = &["
        )
        .unwrap();
        for item in &extension.pattern {
            match item {
                ExtensionPattern::GrammarToken(name) => {
                    let irw = &tokens.tk_to_irw[name];
                    let id = tokens.irw_ids[irw];
                    writeln!(out, "    ExtensionPatternToken::Grammar({id}),").unwrap();
                }
                ExtensionPattern::Keyword(keyword) => {
                    writeln!(out, "    ExtensionPatternToken::Keyword({keyword:?}),").unwrap();
                }
                ExtensionPattern::Label => {
                    out.push_str("    ExtensionPatternToken::Label,\n");
                }
                ExtensionPattern::Identifier => {
                    out.push_str("    ExtensionPatternToken::Identifier,\n");
                }
                ExtensionPattern::StringLiteral => {
                    out.push_str("    ExtensionPatternToken::StringLiteral,\n");
                }
            }
        }
        out.push_str("];\n\n");
    }
    out.push_str("pub const DIALECT_EXTENSIONS: &[ExtensionSpec] = &[\n");
    for (index, extension) in extensions.iter().enumerate() {
        writeln!(
            out,
            "    ExtensionSpec {{ identity: {:?}, action: GeneratedExtensionAction::{}, pattern: EXTENSION_PATTERN_{index}, tail: ExtensionTail::{} }},",
            extension.identity,
            extension.action,
            match extension.tail.as_str() {
                "none" => "None",
                "function_signature" => "FunctionSignature",
                _ => unreachable!("validated tail"),
            }
        )
        .unwrap();
    }
    out.push_str("];\n\n");
    out.push_str("pub fn extension_keyword(spelling: &str) -> Option<&'static str> {\n    match spelling {\n");
    let keywords = extensions
        .iter()
        .flat_map(|extension| extension.pattern.iter())
        .filter_map(|item| match item {
            ExtensionPattern::Keyword(keyword) => Some(keyword.as_str()),
            ExtensionPattern::GrammarToken(_) => None,
            ExtensionPattern::Label => None,
            ExtensionPattern::Identifier | ExtensionPattern::StringLiteral => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    for keyword in keywords {
        writeln!(out, "        {keyword:?} => Some({keyword:?}),").unwrap();
    }
    out.push_str("        _ => None,\n    }\n}\n");
}

fn render_action_value(mapping: &ActionMapping) -> String {
    if let Some(operand) = mapping.operand.as_deref() {
        format!("AstAction::{}({operand:?})", mapping.action,)
    } else {
        format!("AstAction::{}", mapping.action)
    }
}

fn render_action_pattern(mapping: &ActionMapping) -> String {
    if mapping.operand.is_some() {
        format!("AstAction::{}(operand)", mapping.action)
    } else {
        format!("AstAction::{}", mapping.action)
    }
}

fn render_shape(mapping: &ActionMapping) -> String {
    match mapping.shape.as_deref().expect("shape") {
        shape if mapping.operand.is_some() => format!("StatementShape::{shape}(operand)"),
        shape => format!("StatementShape::{shape}"),
    }
}

fn render_strings(out: &mut String, name: &str, values: &[String]) {
    writeln!(out, "pub const {name}: &[&str] = &[").unwrap();
    for value in values {
        writeln!(out, "    {:?},", value).unwrap();
    }
    out.push_str("];\n\n");
}

fn render_pairs(out: &mut String, name: &str, values: &[(u16, u16)]) {
    writeln!(out, "pub const {name}: &[(u16, u16)] = &[").unwrap();
    for (left, right) in values {
        writeln!(out, "    ({left}, {right}),").unwrap();
    }
    out.push_str("];\n\n");
}

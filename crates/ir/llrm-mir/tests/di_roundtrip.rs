//! clang's `-g` output stopped the parser at its first `!DISubprogram(...)`
//! flag set (`unexpected character |`): no `!DIxxx(field: value)` node, no
//! `#dbg_declare(ptr %x, !var, !DIExpression(), !loc)`, no `!dbg` on a
//! `define`. `llrm-llc` refused every C program built with `-g`.

use llrm_mir::module::{GlobalKind, MetadataOperand};
use llrm_mir::{parse, print};

const CLANG: &str = include_str!("clang/dbg.ll");

fn kinds(module: &llrm_mir::module::Module) -> Vec<&str> {
    module.special.values().map(|one| one.kind.as_str()).collect()
}

#[test]
fn test_clang_debug_metadata_parses_and_keeps_its_fields() {
    let module = parse::module(CLANG).expect("clang's -g output parses");
    let kinds = kinds(&module);
    for wanted in [
        "DICompileUnit",
        "DIFile",
        "DISubprogram",
        "DISubroutineType",
        "DIBasicType",
        "DIDerivedType",
        "DICompositeType",
        "DILocalVariable",
        "DILexicalBlock",
        "DILocation",
        "DIExpression",
    ] {
        assert!(kinds.contains(&wanted), "{wanted} missing from {kinds:?}");
    }
    let (id, subprogram) = module.special.iter().find(|(_, one)| one.kind == "DISubprogram").expect("a DISubprogram");
    let node = &module.metadata[id.0 as usize];
    let at = subprogram.fields.iter().position(|name| name == "name").expect("a name field");
    assert_eq!(node.operands[at], MetadataOperand::String("dist".to_owned()));
    let flags = subprogram.fields.iter().position(|name| name == "flags").expect("a flags field");
    assert_eq!(node.operands[flags], MetadataOperand::Ident("DIFlagPrototyped".to_owned()));
}

#[test]
fn test_a_define_keeps_its_dbg_attachment_and_a_declare_its_expression_and_location() {
    let module = parse::module(CLANG).expect("parses");
    let dist = module
        .globals
        .iter()
        .find_map(|one| match &one.kind {
            GlobalKind::Function(function) if one.name.as_deref() == Some("dist") => Some(function),
            _ => None,
        })
        .expect("dist");
    assert!(dist.metadata.iter().any(|(kind, _)| kind == "dbg"), "{:?}", dist.metadata);
    let records = dist.debug_records();
    assert_eq!(records.len(), 4);
    assert!(records.iter().all(|one| one.expression.is_some() && one.location.is_some()), "{records:?}");
}

#[test]
fn test_printed_debug_metadata_reads_back_the_same() {
    let module = parse::module(CLANG).expect("parses");
    let once = print::module(&module);
    let again = parse::module(&once).expect("what is printed parses");
    assert_eq!(print::module(&again), once);
    for text in ["!DISubprogram(name: \"dist\"", "DISPFlagLocalToUnit | DISPFlagDefinition", "#dbg_declare(ptr %"] {
        assert!(once.contains(text), "`{text}` not printed:\n{once}");
    }
}

#[test]
fn test_enumerators_subranges_and_file_scope_variables_parse_and_read_back() {
    let module = parse::module(include_str!("clang/dbg_types.ll")).expect("parses");
    let kinds = kinds(&module);
    for wanted in ["DIEnumerator", "DISubrange", "DIGlobalVariable", "DIGlobalVariableExpression"] {
        assert!(kinds.contains(&wanted), "{wanted} missing from {kinds:?}");
    }
    let once = print::module(&module);
    assert_eq!(print::module(&parse::module(&once).expect("what is printed parses")), once);
}

/// clang puts a declaration's attachment before its linkage and type
/// (`declare !dbg !1 dso_local i32 @sink(...)`): the parser stopped there with
/// `expected a type, found !dbg`, so no `-O2 -g` C program that calls an
/// external function reached the backend.
#[test]
fn a_declaration_keeps_the_dbg_attachment_clang_puts_after_declare() {
    let text = "declare !dbg !1 i32 @sink(i64, i32)\n\n!1 = !{}\n";
    let module = parse::module(text).expect("parses");
    let once = print::module(&module);
    assert!(once.contains("declare !dbg !"), "{once}");
    let again = parse::module(&once).expect("what is printed parses");
    assert_eq!(print::module(&again), once);
}

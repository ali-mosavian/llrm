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

const ARGLIST: &str = include_str!("clang/arglist.ll");

/// clang -O2 -g writes `#dbg_value(!DIArgList(i32 %a, i32 %b), !var,
/// !DIExpression(DW_OP_LLVM_arg, 0, ...))` for a variable that is a sum of
/// values. The parser stopped at it ("expected a type, found DIArgList") and
/// ten of the benchmark programs could not be built with `-g` at all.
#[test]
fn a_debug_value_of_an_argument_list_parses_keeps_its_values_and_prints_back() {
    let module = parse::module(ARGLIST).expect("the argument list parses");
    let function = module
        .functions()
        .map(|(.., function)| function)
        .find(|function| function.debug_records().iter().any(|one| matches!(one.what, llrm_mir::DebugWhat::Args(_))))
        .expect("a function with a variadic record");
    let record = function
        .debug_records()
        .iter()
        .find(|one| matches!(one.what, llrm_mir::DebugWhat::Args(_)))
        .expect("the record");
    let llrm_mir::DebugWhat::Args(list) = record.what else { unreachable!() };
    assert_eq!(function.debug_args(list).len(), 3);
    let again = parse::module(&print::module(&module)).expect("what is printed parses");
    let kept = |module: &llrm_mir::module::Module| -> usize {
        module
            .functions()
            .flat_map(|(.., function)| {
                function.debug_records().iter().map(|one| matches!(one.what, llrm_mir::DebugWhat::Args(_)))
            })
            .filter(|one| *one)
            .count()
    };
    assert_eq!((kept(&module), kept(&again)), (1, 1));
}

/// clang's assignment tracking: `#dbg_assign` records and `!DIAssignID`
/// attachments. The parser stopped at the first record, so every `-O2 -g`
/// file clang wrote without `-fexperimental-assignment-tracking=disabled`
/// (12 of 70 bench programs) was refused.
#[test]
fn test_dbg_assign_records_parse_print_and_parse_again() {
    let module = parse::module(include_str!("clang/assign.ll")).expect("assignment tracking parses");
    let (_, function) =
        module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).map(|(i, _, f)| (i, f)).unwrap();
    let assigns = function
        .debug_records()
        .iter()
        .filter(|one| matches!(one.what, llrm_mir::module::DebugWhat::Assign { .. }))
        .count();
    assert_eq!(assigns, 2);
    let text = print::module(&module);
    assert_eq!(text.matches("#dbg_assign").count(), 2, "{text}");
    assert_eq!(text.matches(", !DIAssignID !").count(), 2, "two attachments: {text}");
    assert_eq!(text.matches("= distinct !DIAssignID()").count(), 2, "{text}");
    let again = parse::module(&text).expect("the printed form parses");
    assert_eq!(print::module(&again), text);
}

/// clang writes a bit-field member's storage unit as `extraData: i64 928`.
/// The parser took `i64` for a flag word and stopped at `928` (`expected
/// ,`), so `llrm-arm64-llc` refused every `-g` file that declares a
/// bit-field struct (basic32.c, dump.c of QuickrBasic).
#[test]
fn a_bit_field_members_typed_extra_data_parses_and_reads_back() {
    let text = "!1 = !DIDerivedType(tag: DW_TAG_member, name: \"f\", size: 24, offset: 928, flags: DIFlagBitField, extraData: i64 928)\n";
    let module = parse::module(text).expect("a typed extraData parses");
    let once = print::module(&module);
    assert_eq!(print::module(&parse::module(&once).expect("what is printed parses")), once);
    assert!(once.contains("928"), "{once}");
}

/// clang -g -O1 writes `#dbg_label(!label, !loc)` for a C label; the parser
/// refused it (`#dbg_label is not a debug record`), so `exec.c` of
/// QuickrBasic (computed-goto labels) was refused at every level with `-g`.
#[test]
fn a_debug_label_record_parses() {
    let text = "define void @f() !dbg !4 {\nentry:\n  #dbg_label(!2, !3)\n  ret void\n}\n\
        !2 = !DILabel(scope: !4, name: \"again\", line: 3)\n!3 = !DILocation(line: 3, scope: !4)\n\
        !4 = distinct !DISubprogram(name: \"f\")\n";
    let module = parse::module(text).expect("a label record parses");
    let once = print::module(&module);
    parse::module(&once).expect("what is printed parses");
}

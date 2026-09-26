use crate::{parse, print};

const DATALAYOUT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16\"\n";

/// Every `.ll` under `tests/`: hand-written mappings, clang's output, and
/// with `invalid`, the ones LLVM's verifier refuses instead.
fn fixtures(invalid: bool) -> Vec<(std::path::PathBuf, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut out = Vec::new();
    for dir in std::fs::read_dir(&root).expect("tests/").flatten() {
        if (dir.file_name() == "invalid") != invalid {
            continue;
        }
        for file in std::fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            if file.path().extension().is_some_and(|one| one == "ll") {
                out.push((file.path(), std::fs::read_to_string(file.path()).expect("readable")));
            }
        }
    }
    out
}

/// `text` read and written back.
fn round(text: &str) -> String {
    print::module(&parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}")))
}

fn refusal(text: &str) -> String {
    parse::module(text).expect_err("refused").to_string()
}

#[test]
fn the_crate_depends_on_nothing() {
    // MIR may name no decoder, object file or target; the dependency graph enforces it.
    let manifest = include_str!("../Cargo.toml");
    let dependencies = manifest.split("[dependencies]").nth(1).expect("a [dependencies] table");
    let entries: Vec<&str> =
        dependencies.lines().map(str::trim).take_while(|line| !line.starts_with('[')).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
    assert!(entries.is_empty(), "{entries:?}");
}

#[test]
fn every_fixture_prints_to_a_fixed_point() {
    let fixtures = fixtures(false);
    assert!(fixtures.len() >= 17, "{} fixtures", fixtures.len());
    for (path, text) in fixtures {
        let module = parse::module(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(crate::verify::verify(&module), Vec::<String>::new(), "{}", path.display());
        let once = round(&text);
        assert_eq!(round(&once), once, "{}", path.display());
    }
}

#[test]
fn a_value_may_be_used_before_its_definition() {
    let text = format!(
        "{DATALAYOUT}\ndefine i16 @count(i16 %n) {{\nentry:\n  br label %head\n\nhead:\n  %i = phi i16 [ 0, %entry ], [ %next, %head ]\n  %next = add nsw i16 %i, 1\n  %more = icmp slt i16 %next, %n\n  br i1 %more, label %head, label %done\n\ndone:\n  ret i16 %next\n}}\n"
    );
    assert_eq!(round(&text), text);
}

#[test]
fn a_forward_use_must_agree_with_its_definition() {
    let text = "define i16 @f() {\nentry:\n  br label %b\n\nb:\n  %x = phi i16 [ 0, %entry ], [ %y, %b ]\n  %y = zext i16 %x to i32\n  br label %b\n}\n";
    assert_eq!(refusal(text), "line 7: %y is used as i16 but defined as i32");
}

#[test]
fn a_value_never_defined_is_refused() {
    let text = "define i16 @f() {\n  ret i16 %ghost\n}\n";
    assert_eq!(refusal(text), "line 2: %ghost is used but never defined");
}

#[test]
fn unnamed_values_count_the_entry_block() {
    let text = "define i16 @f(i16 %0) {\n  %2 = add i16 %0, 1\n  ret i16 %2\n}\n";
    assert_eq!(round(text), text);
    assert_eq!(refusal(&text.replace("%2", "%3")), "line 2: expected to be numbered %2");
}

#[test]
fn constructs_outside_the_subset_are_refused() {
    assert_eq!(refusal("define i16 @f() {\n  ret i16 undef\n}\n"), "line 2: `undef` is outside MIR's subset of LLVM");
    assert_eq!(refusal("@x = global x86_fp80 zeroinitializer\n"), "line 1: `x86_fp80` is outside MIR's subset of LLVM");
    assert_eq!(refusal("target triple = \"i386\"\n"), "line 1: `triple` is outside MIR's subset of LLVM");
}

#[test]
fn attribute_groups_may_follow_their_use() {
    let text = "declare void @stop(i16) #0\n\nattributes #0 = { noreturn memory(argmem: read) \"kind\"=\"error\" }\n";
    assert_eq!(round(text), "declare void @stop(i16) noreturn memory(argmem: read) \"kind\"=\"error\"\n");
}

#[test]
fn floating_constants_print_as_llvm_does() {
    // As LLVM 20's llvm-dis writes them.
    let text = "@a = global double 1.5\n@b = global double 0.1\n@c = global double 0x3FD5555555555555\n@d = global float 0x3FB99999A0000000\n";
    assert_eq!(round(text), "@a = global double 1.500000e+00\n@b = global double 1.000000e-01\n@c = global double 0x3FD5555555555555\n@d = global float 0x3FB99999A0000000\n");
    assert_eq!(refusal("@d = global float 0.1\n"), "line 1: 0.1 is not exactly a float");
}

#[test]
fn integer_constants_hold_their_bits_and_print_signed() {
    assert_eq!(round("@a = global i16 65535\n"), "@a = global i16 -1\n");
    assert_eq!(refusal("@a = global i16 65536\n"), "line 1: 65536 does not fit i16");
}

#[test]
fn globals_keep_their_definition_order_whatever_uses_them_first() {
    let text = "@first = global ptr @second\n@second = global i16 7\n";
    assert_eq!(round(text), text);
    assert_eq!(refusal("@first = global ptr addrspace(1) @second\n@second = global i16 7\n"), "line 1: a global in address space 0 used as ptr addrspace(1)");
}

#[test]
fn metadata_keeps_its_numbers_whatever_mentions_it_first() {
    // Numbered by first mention, an attachment's node swapped with the one
    // it refers to, and clang's loop metadata never printed the same twice.
    let text = "define void @f() {\n  ret void, !x !1\n}\n\n!0 = !{!\"first\"}\n!1 = !{!0}\n";
    assert_eq!(round(text), text);
}

#[test]
fn every_invalid_fixture_is_refused_for_its_reason() {
    let fixtures = fixtures(true);
    assert!(fixtures.len() >= 9, "{} fixtures", fixtures.len());
    for (path, text) in fixtures {
        let reason = text.lines().find_map(|line| line.strip_prefix("; invalid: ")).expect("a `; invalid:` line");
        let found = match parse::module(&text) {
            Err(error) => vec![error.to_string()],
            Ok(module) => crate::verify::verify(&module),
        };
        assert!(found.iter().any(|one| one.contains(reason)), "{}: {found:?}", path.display());
    }
}

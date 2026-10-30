//! The constructs of ValueTracking.cpp that MIR cannot say, each as the fact it
//! rests on: a test fails the day MIR gains the construct, so its translation
//! is owed then and is not forgotten.

use crate::parse;

fn rejects(text: &str) -> bool {
    parse::module(text).is_err()
}

/// MIR types vectors and verifies element-wise operations on them, but has no
/// lane operation, and no front end or lowering of llrm produces or selects a
/// vector value. ValueTracking's `DemandedElts` is therefore not translated:
/// a vector value claims nothing (see `known::compute_known_bits`).
#[test]
fn mir_has_no_lane_operations() {
    for lane in [
        "%x = extractelement <4 x i32> %a, i32 0",
        "%x = insertelement <4 x i32> %a, i32 1, i32 0",
        "%x = shufflevector <4 x i32> %a, <4 x i32> %a, <4 x i32> zeroinitializer",
    ] {
        assert!(rejects(&format!("define void @f(<4 x i32> %a) {{\n  {lane}\n  ret void\n}}\n")), "{lane}");
    }
}

#[test]
fn mir_has_no_scalable_vectors_and_no_vscale_range() {
    assert!(rejects("define i32 @f(<vscale x 4 x i32> %a) {\n  ret i32 0\n}\n"));
    assert!(rejects("define i32 @f() vscale_range(1,16) {\n  ret i32 0\n}\n"));
}

#[test]
fn mir_has_no_operand_bundles() {
    assert!(rejects("declare void @g()\ndefine void @f() {\n  call void @g() [ \"deopt\"(i32 1) ]\n  ret void\n}\n"));
}

#[test]
fn mir_reads_ptrtoaddr_as_ptrtoint() {
    let module = parse::module("define i64 @f(ptr %p) {\n  %x = ptrtoaddr ptr %p to i64\n  ret i64 %x\n}\n")
        .expect("ptrtoaddr is read");
    let text = crate::print::module(&module);
    assert!(text.contains("ptrtoint ptr %p to i64") && !text.contains("ptrtoaddr"), "{text}");
}

#[test]
fn mir_has_no_target_triple() {
    assert!(rejects("target triple = \"riscv64\"\ndefine void @f() {\n  ret void\n}\n"));
}

#[test]
fn mir_has_no_half() {
    assert!(rejects("define half @f(half %a) {\n  ret half %a\n}\n"));
}

/// No front end, pass or lowering of llrm builds a vector type, so
/// `DemandedElts` is not translated. Fails the day one does: ValueTracking's
/// vector cases are then owed.
#[test]
fn nothing_outside_mir_produces_a_vector_type() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    let mut directories = vec![crates];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory).expect("a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !matches!(
                    path.file_name().and_then(|one| one.to_str()),
                    Some("target" | "tests" | "fixtures")
                ) {
                    directories.push(path);
                }
            } else if path.extension().is_some_and(|one| one == "rs")
                && !path.components().any(|part| part.as_os_str() == "llrm-mir")
                && std::fs::read_to_string(&path).is_ok_and(|text| {
                    text.split("#[cfg(test)]").next().unwrap_or_default().contains("intern(Type::Vector")
                })
            {
                found.push(path.display().to_string());
            }
        }
    }
    assert!(found.is_empty(), "these build a vector type: {found:?}");
}

/// The answers we assert were taken from LLVM, and an oracle that can only say
/// one thing proves nothing: in each directory of `tests/vt`, every column that
/// appears says at least two different things (a fact and "unknown").
#[test]
fn the_pinned_llvm_answers_we_assert_say_more_than_one_thing() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vt");
    let mut checked = 0;
    for area in std::fs::read_dir(&root).expect("tests/vt").flatten() {
        let mut answers = std::collections::BTreeMap::<String, std::collections::BTreeSet<String>>::new();
        for file in std::fs::read_dir(area.path()).expect("an area").flatten() {
            let path = file.path();
            if !matches!(
                path.extension().and_then(|one| one.to_str()),
                Some("expected" | "llvm")
            ) {
                continue;
            }
            for line in std::fs::read_to_string(&path).expect("a file").lines() {
                for word in line.split_whitespace() {
                    if let Some((key, value)) = word.split_once('=') {
                        answers.entry(key.to_owned()).or_default().insert(value.to_owned());
                    }
                }
            }
        }
        for (key, values) in &answers {
            assert!(values.len() >= 2, "{}: column `{key}` always says {values:?}", area.path().display());
            checked += 1;
        }
    }
    assert!(checked >= 10, "{checked} columns checked");
}

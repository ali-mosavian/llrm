//! `indirectbr` and `blockaddress`: each address-taken block is a symbol the
//! object defines, and no pass removes the block.

use llrm_arm64::driver::{object, platform_or_default};
use llrm_arm64::machine::Platform;
use llrm_object::Definition;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn linux() -> Platform {
    platform_or_default("aarch64-unknown-linux-gnu").expect("a platform")
}

/// The offsets in the object of the symbols that name blocks.
fn block_symbols(body: &str) -> Result<Vec<usize>, String> {
    let image = object(&format!("{LAYOUT}{body}"), linux(), "t", true)?;
    let mut at: Vec<usize> = image
        .symbols
        .iter()
        .filter(|symbol| symbol.name.starts_with(".LBA"))
        .map(|symbol| match symbol.definition {
            Definition::Defined { offset, .. } => offset,
            _ => panic!("{} is undefined", symbol.name),
        })
        .collect();
    at.sort_unstable();
    Ok(at)
}

/// The legalizer refused `indirectbr` ("custom lowering of BrInd"), and a
/// block reached only through it (nothing branches to it, so the branch folding
/// dropped it) left no label for the table entry to name.
#[test]
fn an_indirectbr_reaches_blocks_nothing_branches_to() {
    let at = block_symbols(
        "@t = internal constant [2 x ptr] [ptr blockaddress(@f, %one), ptr blockaddress(@f, %two)]
define i32 @f(i32 %x) {
entry:
  %p = getelementptr [2 x ptr], ptr @t, i64 0, i32 %x
  %a = load ptr, ptr %p
  indirectbr ptr %a, [label %one, label %two]
one:
  ret i32 1
two:
  ret i32 2
}
",
    )
    .expect("an object");
    assert_eq!(at.len(), 2, "{at:?}");
    assert_ne!(at[0], at[1]);
}

/// Early if-conversion folded the arms of a diamond whose addresses a global
/// holds into its head: the symbols named blocks that were gone ("no block").
#[test]
fn an_address_taken_arm_is_not_if_converted_away() {
    let at = block_symbols(
        "@k = global [2 x ptr] [ptr blockaddress(@f, %one), ptr blockaddress(@f, %two)]
define i32 @f(i32 %w, i32 %x) {
entry:
  %c = icmp ne i32 %w, 0
  br i1 %c, label %one, label %two
one:
  %a = add i32 %x, 1
  br label %join
two:
  %b = mul i32 %x, 2
  br label %join
join:
  %r = phi i32 [ %a, %one ], [ %b, %two ]
  ret i32 %r
}
",
    )
    .expect("an object");
    assert_eq!(at.len(), 2, "{at:?}");
}

/// A block's address read by a function that comes before the one that holds
/// the block named a symbol the object then defined twice: the first function's
/// relocation held the undefined copy.
#[test]
fn a_block_address_read_before_its_function_is_defined_resolves() {
    let image = object(
        &format!(
            "{LAYOUT}define ptr @g() {{
  ret ptr blockaddress(@f, %two)
}}
define i32 @f(ptr %a) {{
entry:
  indirectbr ptr %a, [label %one, label %two]
one:
  ret i32 1
two:
  ret i32 2
}}
"
        ),
        linux(),
        "t",
        true,
    )
    .expect("an object");
    let named: Vec<_> = image.symbols.iter().filter(|symbol| symbol.name.starts_with(".LBA")).collect();
    assert!(named.iter().all(|symbol| matches!(symbol.definition, Definition::Defined { .. })), "{named:?}");
    let mut names: Vec<_> = named.iter().map(|symbol| symbol.name.clone()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), named.len(), "a block symbol is defined twice: {named:?}");
}

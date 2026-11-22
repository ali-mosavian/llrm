//! `GlobalMerge` with `MergeExternalByDefault` (GlobalMerge.cpp, `!MachO`):
//! defined external globals one function stores together share one base, each
//! name an alias at its place in the merged struct. QuickrBasic qbrt gfx.c -Os:
//! 182 `adrp` against gcc's 65 (section anchors), `B_VIEW` one per global.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;
use llrm_object::{Binding, Definition};

const SOURCE: &str = include_str!("data/global_merge_external.ll");

/// The `adrp` words of the object's code.
fn pages(object: &llrm_object::Object) -> usize {
    object
        .sections
        .iter()
        .filter(|section| section.name.contains("text"))
        .flat_map(|section| section.image.chunks_exact(4))
        .filter(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]) & 0x9f00_0000 == 0x9000_0000)
        .count()
}

/// Four external globals were four `adrp`; they are one page address and the
/// names stay where other objects find them: public, four bytes each, in one
/// section at offsets 0, 4, 8 and 12 of the struct.
#[test]
fn external_globals_used_together_take_one_page_address_and_keep_their_names() {
    let object = driver::object(SOURCE, Platform::Linux, "t", Level::Os).expect("lowers");
    assert_eq!(pages(&object), 1);
    let mut places = Vec::new();
    for name in ["a", "b", "c", "d"] {
        let symbol = object.symbols.iter().find(|symbol| symbol.name == name).expect("the name is still defined");
        assert_eq!((symbol.binding, symbol.size), (Binding::Public, 4), "{name}");
        let Definition::Defined { section, offset } = symbol.definition else { panic!("{name} is defined") };
        places.push((section, offset));
    }
    let (section, first) = places[0];
    let expected: Vec<_> = (0..4).map(|n| (section, first + 4 * n)).collect();
    assert_eq!(places, expected, "one struct, the four in order");
}

/// Control: assembly text carries no alias, so nothing external merges there.
#[test]
fn assembly_text_leaves_external_globals_alone() {
    let text = driver::assembly(SOURCE, Platform::Linux, Level::Os).expect("lowers");
    assert_eq!(text.lines().filter(|line| line.contains("adrp")).count(), 4, "{text}");
}

/// `-fPIC` shared-library code may have its default-visibility externals
/// interposed by another module: they keep their own symbols and pages.
#[test]
fn an_interposable_external_global_is_not_merged() {
    let source = format!("{SOURCE}!llvm.module.flags = !{{!0}}\n!0 = !{{i32 8, !\"PIC Level\", i32 2}}\n");
    let object = driver::object(&source, Platform::Linux, "t", Level::Os).expect("lowers");
    assert_eq!(pages(&object), 4);
}

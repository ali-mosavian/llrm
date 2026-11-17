//! A constant the function loads from memory sat in `.rodata` with the
//! program's own constants. The pin puts it in a mergeable section of its size
//! (`.rodata.cst16`, `TargetLoweringObjectFileELF::getSectionForConstant`), and
//! the bench's byte count reads those: `dot` -Os counted 292 B with 224 B of
//! vector constants unseen, `nbody` 364 with 56 unseen.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

fn sections() -> Vec<(String, usize)> {
    let done = driver::object_with(include_str!("data/pool_sections.ll"), Platform::Linux, "t", Level::O2, false)
        .expect("lowers");
    done.object.sections.iter().map(|section| (section.name.clone(), section.image.len())).collect()
}

#[test]
fn a_pool_constant_sits_in_the_mergeable_section_of_its_size() {
    let sections = sections();
    let size = |name: &str| sections.iter().find(|(found, _)| found == name).map(|(_, size)| *size);
    assert_eq!(size(".rodata.cst16"), Some(16), "{sections:?}");
    assert_eq!(size(".rodata.cst8"), Some(8), "{sections:?}");
}

#[test]
fn a_global_constant_stays_in_rodata() {
    let sections = sections();
    assert_eq!(sections.iter().find(|(name, _)| name == ".rodata").map(|(_, size)| *size), Some(12), "{sections:?}");
}

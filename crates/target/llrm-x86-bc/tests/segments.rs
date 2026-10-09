//! DGROUP is the program's: what LINK groups, by name, not one module's
//! GRPDEF.

use llrm_x86_bc::machine::Facts;
use llrm_x86_bc::objects::Carving;
use llrm_x86_bcmachine::objectfile::omf;

fn loaded(fixture: &str) -> llrm_omf::module::Module {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/inputs/omf").join(fixture);
    llrm_omf::module::load(&path).expect("reads").expect("an object")
}

/// The raise carved DGROUP by the module's own GRPDEF, so a segment
/// another module groups was carved as far data, and bcdriver refused the
/// program.
#[test]
fn the_carving_follows_the_program_s_data_group() {
    let found = loaded("arith-q-o.obj");
    let facts = Facts::new(&found, &llrm_x86_m16::machine::BUILT_IN).expect("facts");
    let named = omf::segments(&found.records);
    let index =
        named.iter().position(|one| one.as_ref().is_some_and(|(name, _)| name == "BC_CN")).expect("BC_CN") as i64;
    let mut layout = llrm_x86_bc::segments(&llrm_x86_m16::machine::BUILT_IN, [&found]);
    assert!(Carving::of(&facts, &layout).dgroup.contains(&index));
    layout.data_group.members.retain(|one| one != "BC_CN");
    assert!(!Carving::of(&facts, &layout).dgroup.contains(&index));
}

/// Every module's GRPDEF adds to the program's DGROUP.
#[test]
fn the_data_group_is_every_module_s() {
    let (quick, visual) = (loaded("arith-q-o.obj"), loaded("arith-v-g3.obj"));
    let both = llrm_x86_bc::segments(&llrm_x86_m16::machine::BUILT_IN, [&quick, &visual]);
    for one in [
        llrm_x86_bc::segments(&llrm_x86_m16::machine::BUILT_IN, [&quick]),
        llrm_x86_bc::segments(&llrm_x86_m16::machine::BUILT_IN, [&visual]),
    ] {
        assert!(one.data_group.members.iter().all(|member| both.data_group.members.contains(member)));
    }
}

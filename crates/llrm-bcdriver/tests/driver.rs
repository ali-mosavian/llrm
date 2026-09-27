//! The rich route, checked on what it writes.

use std::path::PathBuf;
use std::rc::Rc;

use llrm_omf::omf::{self, Record};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/omf").join(name)
}

fn records(name: &str) -> Vec<Rc<Record>> {
    omf::parse(&std::fs::read(fixture(name)).expect("reads")).expect("an object")
}

fn recompiled(name: &str) -> Vec<Rc<Record>> {
    let data = std::fs::read(fixture(name)).expect("reads");
    let written = llrm_bcdriver::compiled(&data, "386", name).unwrap_or_else(|why| panic!("{name}: {why}"));
    omf::parse(&written).expect("parses")
}

/// A segment's index and image, by name.
fn segment(records: &[Rc<Record>], name: &str) -> (i64, Vec<u8>) {
    let segments = omf::segments(records);
    let index = segments.iter().position(|one| one.as_ref().is_some_and(|(named, _)| named == name)).unwrap_or_else(|| panic!("no {name}"));
    let size = segments[index].as_ref().expect("named").1;
    (index as i64, omf::segment_image(records, index as i64, size))
}

/// The runtime enters the module at its header, which keeps every byte
/// but the words LINK fills in.
#[test]
fn a_recompiled_module_keeps_its_header() {
    let relocated = [10, 12, 14, 16, 24, 32, 0x22];
    for name in ["cmpord-q-o.obj", "cmpord-p-g2.obj", "cmpord-v-g3.obj"] {
        let (before, after) = (records(name), recompiled(name));
        let (_, code_name, _) = omf::code_segment(&before).expect("code");
        let (old, new) = (segment(&before, &code_name).1, segment(&after, &code_name).1);
        let kept = |image: &[u8]| -> Vec<u8> { (0..0x30).filter(|&at| !relocated.iter().any(|&word| word == at || word + 1 == at)).map(|at| image[at]).collect() };
        assert_eq!(kept(&new), kept(&old), "{name}");
    }
}

/// Each header word LINK fixes up keeps its offset into its segment: OF_DS
/// is BC_DS + 2, and READ read the first row's key as its text while it
/// was BC_DS + 0 -- lngmxx printed "S= 0".
#[test]
fn a_header_word_keeps_its_offset_into_its_segment() {
    for name in ["lngmxx-q-o.obj", "lngmxx-p-noo.obj", "lngmxx-v-g3.obj"] {
        let named = |records: &[Rc<Record>]| -> Vec<(i64, String, i64)> {
            let (code, code_name, _) = omf::code_segment(records).expect("code");
            let (_, image) = segment(records, &code_name);
            let segments = omf::segments(records);
            let mut words: Vec<(i64, String, i64)> = omf::fixups(records)
                .into_iter()
                .filter(|one| one.seg == Some(code) && one.offset < 0x30 && one.target == "segment" && one.index != code)
                .map(|one| {
                    let at = one.offset as usize;
                    let segment = segments[one.index as usize].as_ref().expect("named").0.clone();
                    (one.offset, segment, one.disp + i64::from(u16::from_le_bytes([image[at], image[at + 1]])))
                })
                .collect();
            words.sort();
            words
        };
        assert_eq!(named(&recompiled(name)), named(&records(name)), "{name}");
    }
}

/// RESTORE finds a DATA row by the code offset BC keyed it with. The
/// recompile moves that code, so the keys stay the original offsets, as
/// literals. Refused before: "BC_DS names code at ..., which the recompile
/// moves".
#[test]
fn a_data_row_keeps_its_original_key() {
    for name in ["lngmxx-p-noo.obj", "lngmxx-v-g3.obj"] {
        let (before, after) = (records(name), recompiled(name));
        let (code, _, _) = omf::code_segment(&before).expect("code");
        let (index, old) = segment(&before, "BC_DS");
        let word = |image: &[u8], at: i64| i64::from(u16::from_le_bytes([image[at as usize], image[at as usize + 1]]));
        // The field holds the addend LINK adds the target's offset to.
        let keys: Vec<(i64, i64)> = omf::fixups(&before).into_iter().filter(|one| one.seg == Some(index) && one.target == "segment" && one.index == code).map(|one| (one.offset, one.disp + word(&old, one.offset))).collect();
        assert!(!keys.is_empty(), "{name} keys no DATA row");
        let (written, image) = segment(&after, "BC_DS");
        for (at, key) in keys {
            assert_eq!(word(&image, at), key, "{name} at {at:#x}");
            assert!(!omf::fixups(&after).iter().any(|one| one.seg == Some(written) && one.offset == at), "{name} relocates the key at {at:#x}");
        }
    }
}

/// A constant isel keeps in memory goes with BC's own, in BC_CN. Refused
/// before: "a constant pool entry, which the BASIC data segments do not
/// place yet".
#[test]
fn a_pooled_constant_joins_bcs_constants() {
    let name = "fpcse-p-noo.obj";
    let (before, after) = (records(name), recompiled(name));
    assert!(segment(&after, "BC_CN").1.len() > segment(&before, "BC_CN").1.len());
}

/// One refused module fails its program, and nothing is written.
#[test]
fn a_refusal_writes_nothing() {
    let out = std::env::temp_dir().join(format!("bcdriver-refusal-{}", std::process::id()));
    std::fs::create_dir_all(&out).expect("made");
    // Refused: INTO is unmodelled.
    let refused = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/regressions/arridx-bounds-p-g2.obj");
    assert!(refused.exists());
    let argv: Vec<String> = [fixture("cmpord-p-g2.obj"), refused]
        .iter()
        .map(|one| one.display().to_string())
        .chain(["-o".to_owned(), out.display().to_string()])
        .collect();
    assert_eq!(llrm_bcdriver::main(&argv), 1);
    assert_eq!(std::fs::read_dir(&out).expect("listed").count(), 0);
    std::fs::remove_dir_all(&out).expect("removed");
}

/// A far pointer to DGROUP data, as an array descriptor holds, is DGROUP's
/// selector and the group-relative offset. Written as one POINTER fixup it
/// took its own segment's selector, which BASIC's code pairs with the
/// group-relative offset: arrprm's SUB filled the wrong cells and printed
/// " 0  0".
#[test]
fn a_far_pointer_to_dgroup_names_dgroup() {
    for name in ["nestud-q-o.obj"] {
        let far = |records: &[Rc<Record>]| -> (usize, usize) {
            let found = llrm_omf::module::of(records).expect("a module");
            let (code, _, _) = omf::code_segment(records).expect("code");
            let segments = omf::segments(records);
            // CodeView's segments are not written again.
            let debug = |index: i64| segments[index as usize].as_ref().is_some_and(|(name, _)| name.starts_with("$$"));
            let data: Vec<_> = omf::fixups(records).into_iter().filter(|one| one.seg.is_some_and(|seg| seg != code && !debug(seg))).collect();
            let pointers = data.iter().filter(|one| one.loc == omf::LOC_PTR32 && one.target == "segment" && found.dgroup.members.contains(&one.index)).count();
            let selectors = data.iter().filter(|one| one.loc == omf::LOC_BASE && one.target == "group").count();
            (pointers, selectors)
        };
        let (before, after) = (far(&records(name)), far(&recompiled(name)));
        assert!(before.0 > 0, "{name} holds no far pointer to DGROUP");
        assert_eq!(after, (0, before.0), "{name}");
    }
}

/// FPDEEP's main once the pipeline has run, linked against its runtime, as
/// MIR text.
fn fpdeep() -> String {
    let found = llrm_omf::module::load(&fixture("fpdeep-q-o.obj")).expect("reads").expect("an object");
    let raised = llrm_bc::raise(&found, &llrm_core::abi::machine::BUILT_IN).unwrap_or_else(|refusal| panic!("{refusal}"));
    let mut program = llrm_mir::program::Program::new(vec![raised.module], Rc::new(llrm_cycles::target::Dos::default())).and_then(|one| one.with_runtime(raised.runtime)).unwrap();
    llrm_transforms::pipeline::applied(&mut program, &llrm_transforms::pipeline::Applied::default()).unwrap();
    let text = llrm_mir::print::module(&program.modules[0]);
    text[text.find("define void @main").expect("main")..].to_owned()
}

/// FPDEEP's `FOR i = 1 TO 3` around its PRINTs is copied out, each trip
/// printing its own `i`; the old route's unroll test. Refused before as
/// "contains call and code would grow".
#[test]
fn fpdeep_copies_out_its_print_loop() {
    let main = fpdeep();
    assert!(!main.contains(" phi "), "{main}");
    for i in 1..=3 {
        assert!(main.contains(&format!("@llrm.qb.B$PSI2(i16 {i})")), "{main}");
    }
}

/// Every FPDEEP number is exact, so it prints constants and computes no
/// float arithmetic; the old route's floatbounds, floatfold and literal
/// tests.
#[test]
#[ignore = "p(i) reads through @BC_DATA.0002+4i miss the stores to the separately carved @BC_DATA.0006.., and loads of the never-written BC_CN literals do not fold"]
fn fpdeep_prints_constants_and_computes_no_float() {
    let main = fpdeep();
    for n in [144, 6, 512, 784, 14, 768, 3600, 30, 896] {
        assert!(main.contains(&format!("@llrm.qb.B$PEI4(i16 0, i16 {n})")), "{n}\n{main}");
    }
    assert!(!["fmul", "fdiv", "fadd", "fsub"].iter().any(|op| main.contains(&format!(" {op} "))), "{main}");
}

/// ON ERROR's landing stub, its inline helper and ERR's word are the
/// module's own: `__LANDING` was left an external LINK could not resolve.
#[test]
fn a_handled_module_defines_its_landing() {
    for name in ["onerr-q-o.obj", "onerr-v-g3.obj"] {
        let externals = omf::externals(&recompiled(name));
        assert!(externals.iter().any(|one| one == "B$OEGA"), "{name}: {externals:?}");
        assert!(!externals.iter().any(|one| one == "__LANDING" || one.starts_with("$QB$")), "{name}: {externals:?}");
    }
}

/// The runtime reports an error's line from the last statement-table row
/// before it: with the landing pad's row alone, laid last, an unhandled
/// error before it reported "No line number" where BC's reports line 0.
#[test]
fn every_procedure_starts_a_statement_table_row() {
    let records = recompiled("onerr-q-o.obj");
    let module = llrm_omf::module::of(&records).expect("a module");
    let word = |at: i64| i64::from(u16::from_le_bytes([module.code[at as usize], module.code[at as usize + 1]]));
    let start = word(0x0A);
    let rows: Vec<i64> = (0..).map(|row| start + 4 * row).take_while(|&at| module.operands.contains_key(&at)).map(word).collect();
    assert!(rows.len() >= 2 && rows.iter().min() < rows.iter().max(), "{rows:x?}");
}

/// Data no frontend lays out -- ON ERROR's ERL table, the ERR its landing
/// keeps -- goes where BC keeps its constants: erlnum's table was left a
/// reference to nothing defined.
#[test]
fn data_emission_adds_is_laid_out() {
    for name in ["erlnum-q-o.obj", "erlnum-v-g3.obj"] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/regressions").join(name);
        let data = std::fs::read(path).expect("reads");
        llrm_bcdriver::compiled(&data, "386", name).unwrap_or_else(|why| panic!("{name}: {why}"));
    }
}

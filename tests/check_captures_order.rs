//! `LLRM_CHECK_CAPTURES` with `LLRM_CHECK_ORDER` panicked ("frame objects
//! captured by a use that the facts say never escape ... Some(\"place\")") on
//! bench/{bintree,queens,quicksort}.nib: a re-run of the function passes
//! (`pipeline::rerun`) audited its facts against the module copy an earlier
//! pass took, not against the callees' bodies as they stood. It failed
//! pgo.yml's identity check on 6 of 1363 objects.

use std::path::Path;
use std::process::Command;

#[test]
fn the_capture_audit_reads_callees_as_they_stand_in_a_rerun() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["bintree", "queens", "quicksort"] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("bench/{name}/{name}.nib"));
        let run = Command::new(env!("CARGO_BIN_EXE_llrm-nib"))
            .env("LLRM_CHECK_CAPTURES", "1")
            .env("LLRM_CHECK_ORDER", "1")
            .arg("-O2")
            .arg("-o")
            .arg(directory.path().join(format!("{name}.obj")))
            .arg(&source)
            .output()
            .unwrap();
        assert!(run.status.success(), "{name}: {}", String::from_utf8_lossy(&run.stderr));
    }
}

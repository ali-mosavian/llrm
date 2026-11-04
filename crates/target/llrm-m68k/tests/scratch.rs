//! A test's scratch file stayed in the temp directory after it: 3,600
//! `llrm-m68k-*` entries and 21,000 inodes had piled up when the landing runner
//! stopped on a full temp directory. A scratch path now deletes its file when
//! it goes out of scope.

mod support;

#[test]
fn a_scratch_file_is_gone_after_its_test() {
    let kept;
    {
        let path = support::unique_path("scratch-test");
        std::fs::write(&path, b"x").unwrap();
        assert!(path.exists());
        kept = path.to_path_buf();
    }
    assert!(!kept.exists(), "{} is still there", kept.display());
}

#[test]
fn a_scratch_file_is_gone_after_a_panic() {
    let kept = std::sync::Mutex::new(None);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let path = support::unique_path("scratch-panic");
        std::fs::write(&path, b"x").unwrap();
        *kept.lock().unwrap() = Some(path.to_path_buf());
        panic!("the test failed");
    }));
    assert!(outcome.is_err());
    let kept = kept.lock().unwrap().take().unwrap();
    assert!(!kept.exists(), "{} is still there", kept.display());
}

#[path = "../examples/tempdir/mod.rs"]
mod example_scratch;

#[test]
fn an_examples_scratch_directory_is_gone_when_its_run_ends() {
    let kept;
    {
        let dir = example_scratch::Scratch::new("scratch-example");
        std::fs::write(dir.join("f"), b"x").unwrap();
        kept = dir.to_path_buf();
        assert!(kept.exists());
    }
    assert!(!kept.exists(), "{} is still there", kept.display());
}

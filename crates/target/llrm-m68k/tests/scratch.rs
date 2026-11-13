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

/// Runs the built example `name` with `TMPDIR` a fresh directory and the tools
/// it asks for set to nothing real, so it stops with a panic after it has made
/// its scratch directory.
fn run_example(
    name: &str,
    args: &[&str],
    envs: &[(&str, &str)],
) -> (String, Vec<String>) {
    let tmp = support::unique_path("scratch-tmpdir");
    std::fs::create_dir_all(&*tmp).unwrap();
    let profile_dir = std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    let exe = profile_dir.join("examples").join(name);
    if !exe.exists() {
        // `cargo test --test scratch` builds no examples; only a full `cargo
        // test` does.
        let profile = profile_dir.file_name().unwrap().to_string_lossy().into_owned();
        let mut build = std::process::Command::new(env!("CARGO"));
        build.args(["build", "-p", "llrm-m68k", "--example", name]);
        if profile != "debug" {
            build.args(["--profile", &profile]);
        }
        assert!(build.status().unwrap().success(), "cargo build -p llrm-m68k --example {name} failed");
    }
    let out =
        std::process::Command::new(&exe).args(args).envs(envs.iter().copied()).env("TMPDIR", &*tmp).output().unwrap();
    let left = std::fs::read_dir(&*tmp)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    (String::from_utf8_lossy(&out.stderr).into_owned(), left)
}

/// Symptom: `matrix` and `cycles` made `llrm-m68k-<kind>-<pid>` in `$TMPDIR`
/// and removed it on one path only; a run that panicked or was killed left it:
/// 11,900 directories by 2026-11-10, and the temp directory out of inodes.
#[test]
fn matrix_leaves_no_scratch_directory_when_it_panics() {
    let (stderr, left) = run_example(
        "matrix",
        &["kernels"],
        &[
            ("LLRM_PIN_CLANG", "/nonexistent"),
            ("LLRM_CC", "/nonexistent"),
            ("LLVM_M68K", "/nonexistent"),
            ("GCC_M68K", "/nonexistent"),
            ("MATRIX_PROGS", "no_such_program"),
        ],
    );
    assert!(stderr.contains("MATRIX_PROGS"), "the run did not get past its scratch directory:\n{stderr}");
    assert_eq!(left, Vec::<String>::new(), "left in $TMPDIR");
}

#[test]
fn cycles_leaves_no_scratch_directory_when_it_panics() {
    let (stderr, left) = run_example("cycles", &["--exe", "/nonexistent/exe"], &[("GCC_M68K", "/nonexistent")]);
    assert!(!stderr.is_empty(), "the run stopped silently:\n{stderr}");
    assert_eq!(left, Vec::<String>::new(), "left in $TMPDIR");
}

/// Symptom: `MATRIX_CPUS=68010,68030` named columns the table did not have and
/// the run printed nothing and exited 0, so a sweep of two CPUs read as "no
/// worse cells".
#[test]
fn matrix_stops_on_a_cpu_it_has_no_column_for() {
    let (stderr, _) = run_example(
        "matrix",
        &["kernels"],
        &[
            ("LLRM_PIN_CLANG", "/nonexistent"),
            ("LLRM_CC", "/nonexistent"),
            ("LLVM_M68K", "/nonexistent"),
            ("GCC_M68K", "/nonexistent"),
            ("MATRIX_CPUS", "68099"),
        ],
    );
    assert!(stderr.contains("MATRIX_CPUS: no column"), "the run did not name the bad column:\n{stderr}");
}

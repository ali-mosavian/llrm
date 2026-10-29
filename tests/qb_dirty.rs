//! The dirty-line protocol of the host video (runtime/qb/dev/host/video.h)
//! under a writer and a presenter thread, tests/qbhost/dirty_stress.c: plain
//! and under ThreadSanitizer. The weak-memory case is arm64 hardware (the Mac
//! session runs this file; qemu on an x86 host orders memory strongly and
//! cannot show a tear).

use std::path::Path;
use std::process::Command;

/// `program` with the soft data limit raised to the hard one: `prlimit --data=unlimited` raises the hard limit too, which a
/// runner with a finite hard limit refuses ("Operation not permitted") before the program starts.
fn uncapped(program: &Path) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg("ulimit -Sd \"$(ulimit -Hd)\"; exec \"$0\"").arg(program);
    command
}

fn stress(sanitize: Option<&str>) {
    stress_under(sanitize, None);
}

/// With `hard`, the whole run sits under a finite hard data limit of that many bytes, as the gate's runners are.
fn stress_under(sanitize: Option<&str>, hard: Option<&str>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = std::env::temp_dir().join(format!("dirty_stress_{}_{}", sanitize.unwrap_or("plain"), std::process::id()));
    let mut build = Command::new("cc");
    build.args(["-std=gnu11", "-w", "-g", "-pthread", "-D__cdecl="]);
    if let Some(kind) = sanitize {
        build.arg(format!("-fsanitize={kind}"));
    }
    for include in [
        "runtime/qb/dev",
        "runtime/qb/dev/sdl",
        "runtime/qb/dev/host",
        "runtime/shared/posix",
        "runtime/qb",
        "runtime/qb/posix",
    ] {
        build.arg("-I").arg(root.join(include));
    }
    for source in [
        "tests/qbhost/dirty_stress.c",
        "runtime/qb/dev/host/video.c",
        "runtime/qb/dev/host/vgadata.c",
        "runtime/qb/dev/host/tick.c",
        "runtime/shared/posix/os.c",
    ] {
        build.arg(root.join(source));
    }
    build.arg("-o").arg(&out);
    let built = build.output().expect("cc runs");
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let mut run = match hard {
        Some(bytes) => {
            let mut wrapped = Command::new("prlimit");
            wrapped.arg(format!("--data={bytes}:{bytes}")).arg("--").arg("sh").arg("-c");
            wrapped.arg("ulimit -Sd \"$(ulimit -Hd)\"; exec \"$0\"").arg(&out);
            wrapped
        }
        None => uncapped(&out),
    };
    let done = run.env("TSAN_OPTIONS", "halt_on_error=1").output().expect("the program runs");
    std::fs::remove_file(&out).ok();
    let printed = String::from_utf8_lossy(&done.stdout).into_owned();
    assert!(done.status.success(), "{printed}{}", String::from_utf8_lossy(&done.stderr));
    assert_eq!(printed.trim(), "ok");
}

#[test]
fn the_presenter_never_sees_a_torn_line() {
    stress(None);
}

#[test]
fn the_dirty_line_protocol_is_race_free_under_thread_sanitizer() {
    stress(Some("thread"));
}

/// The old `prlimit --data=unlimited` wrapper died with "failed to set the DATA resource limit: Operation not permitted" under
/// any finite hard data limit, so qb_dirty was red on every runner that has one.
#[test]
fn the_presenter_never_sees_a_torn_line_under_a_finite_hard_data_limit() {
    stress_under(None, Some("8000000000"));
}

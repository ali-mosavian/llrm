//! What a function nothing outside the program reaches leaves different is told to its callers
//! (`backend/calleefacts.rs`): a caller keeps a value in any register the callee did not write, and a function that is
//! not private tells nothing.

use std::process::Command;

mod common;

/// The listing of C `source` and the facts the compiler found (`LLRM_DEBUG=facts`), under `flags`.
fn compiled(
    flags: &[&str],
    source: &str,
) -> (String, String) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("t.c");
    std::fs::write(&path, source).unwrap();
    let out = directory.path().join("t.asm");
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .env("LLRM_DEBUG", "facts")
        .args(flags)
        .args(["-O2", "-fno-inline-functions", "-fno-inline-functions-called-once", "-S"])
        .arg(&path)
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    (std::fs::read_to_string(out).unwrap(), String::from_utf8_lossy(&done.stderr).into_owned())
}

fn procedure(
    listing: &str,
    name: &str,
) -> Vec<String> {
    let from = listing.find(&format!("{name} proc")).unwrap_or_else(|| panic!("{name} in:\n{listing}"));
    listing[from..]
        .lines()
        .skip(1)
        .take_while(|line| !line.ends_with("endp"))
        .map(|line| line.trim().to_owned())
        .collect()
}

const SYSV: &[&str] = &["-m32", "-mabi=sysv"];

/// `twice` writes AX alone, and cdecl16 says a call loses AX, BX, CX, DX and ES; `g` kept `k` in SI across the call,
/// which pushed and popped SI for its caller: with the fact it keeps `k` in BX, which `twice` leaves, and saves
/// nothing.
#[test]
fn a_caller_keeps_a_value_in_a_register_a_private_callee_does_not_write() {
    let (text, facts) = compiled(
        &[],
        "static int twice(int x) { return x + x; }\nint g(int a, int b)\n{\n    int k = a * b;\n    int t = twice(a);\n    return t + k;\n}\n",
    );
    assert!(facts.contains(&format!("{} writes {{EAX}}", common::symbol("twice"))), "{facts}");
    let lines = procedure(&text, &common::symbol("g"));
    assert!(!lines.iter().any(|one| one == "push si" || one == "pop si"), "{lines:?}");
}

/// A callee that saves a register and restores it has not written it for its caller: `step` uses ESI and EDI and pushes
/// them.
#[test]
fn what_a_prologue_saves_and_an_epilogue_restores_is_not_written() {
    let (_, facts) = compiled(
        SYSV,
        "static int step(int *p, int n, int k)\n{\n    int i, s = 0;\n    for (i = 0; i < n; i++) s += p[i] * k + (s >> 3);\n    return s;\n}\nint pub(int *p, int n) { return step(p, n, 3) + step(p, n, 5); }\n",
    );
    let line = facts.lines().find(|one| one.contains("_step writes")).unwrap_or_else(|| panic!("{facts}"));
    assert!(line.contains("EAX") && !line.contains("ESI") && !line.contains("EDI"), "{line}");
}

/// An exported function, one whose address is taken and one that calls itself tell nothing: their callers keep the
/// convention.
#[test]
fn a_function_that_may_be_reached_another_way_tells_nothing() {
    let (_, facts) = compiled(
        SYSV,
        "int exported(int x) { return x + 1; }\nstatic int addressed(int x) { return x + 2; }\nint (*keep)(int) = addressed;\nstatic int again(int x) { return x < 2 ? x : again(x - 1) + again(x - 2); }\nint use(int x) { return exported(x) + addressed(x) + again(x); }\n",
    );
    for name in ["exported", "addressed", "again"] {
        assert!(!facts.contains(&format!("_{name} writes")), "{name}: {facts}");
    }
}

/// A callee that loads a far pointer writes ES, which its caller must not hold a value in; one that reads near memory
/// leaves ES.
#[test]
fn a_segment_register_is_written_where_the_callee_loads_it() {
    let (_, facts) = compiled(
        &[],
        "char __far *far_cursor;\nchar *near_cursor;\nstatic int far_get(void) { return *far_cursor; }\nstatic int near_get(void) { return *near_cursor; }\nint h(void) { return far_get() + near_get() + far_get(); }\n",
    );
    let far = facts
        .lines()
        .find(|one| one.contains(&format!("{} writes", common::symbol("far_get"))))
        .unwrap_or_else(|| panic!("{facts}"));
    let near = facts
        .lines()
        .find(|one| one.contains(&format!("{} writes", common::symbol("near_get"))))
        .unwrap_or_else(|| panic!("{facts}"));
    assert!(far.contains("ES") && !near.contains("ES"), "{far} / {near}");
}

/// A far pointer's selector held in ES across a call survives it where the callee does not write ES: `h` loaded it
/// again after the call (`mov es, [bp+8]`), the convention saying ES is lost.
#[test]
fn a_caller_keeps_es_across_a_call_that_does_not_write_it() {
    let (text, _) = compiled(
        &[],
        "char *near_cursor;\nstatic int near_get(void) { return *near_cursor; }\nint h(char __far *a) { int x = *a; int y = near_get(); return x + y + a[1]; }\n",
    );
    let lines = procedure(&text, &common::symbol("h"));
    assert!(!lines.iter().any(|one| one.starts_with("mov es,")), "{lines:?}");
}

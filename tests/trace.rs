//! `LLRM_TRACE=path` appends what a compile did, a JSON object a line with the
//! file it was made for, and off writes nothing.

use std::path::Path;
use std::process::Command;

fn compile(
    directory: &Path,
    source: &str,
    trace: Option<&Path>,
) {
    let file = directory.join(source);
    std::fs::write(
        &file,
        "int f(int a, int b) { int s = 0; int i; for (i = 0; i < a; i++) s += i * b + 3; return s; }\n",
    )
    .expect("writes the source");
    let mut command = Command::new(Path::new(env!("CARGO_BIN_EXE_llrm-c")));
    command.args(["-m32", "-O2", "-o", directory.join("t.obj").to_str().unwrap(), file.to_str().unwrap()]);
    command.env_remove("LLRM_TRACE");
    if let Some(trace) = trace {
        command.env("LLRM_TRACE", trace);
    }
    let done = command.output().expect("runs llrm-c");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

/// Compiles of two files side by side wrote lines that did not say which was
/// which (hoist-bounded's interleaving).
#[test]
fn test_each_compile_appends_its_lines_with_its_file_name_and_off_writes_nothing() {
    let directory = tempfile::tempdir().expect("a directory");
    let trace = directory.path().join("trace.jsonl");
    compile(directory.path(), "first.c", None);
    assert!(!trace.exists(), "a trace was written with LLRM_TRACE unset");
    compile(directory.path(), "first.c", Some(&trace));
    compile(directory.path(), "second.c", Some(&trace));
    let text = std::fs::read_to_string(&trace).expect("the trace");
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.iter().all(|one| one.starts_with("{\"file\":\"") && one.ends_with('}')), "{text}");
    for name in ["first.c", "second.c"] {
        assert!(
            lines.iter().any(|one| one.contains(&format!("/{name}@O2\",")) && one.contains("\"ev\":\"run\"")),
            "no run line for {name}"
        );
    }
    for event in ["run", "ask", "edit", "clone"] {
        assert!(lines.iter().any(|one| one.contains(&format!("\"ev\":\"{event}\""))), "no {event} line");
    }
    // An ask says how it was answered, and what a compute cost; the lines are
    // numbered, and an ask is numbered when it starts.
    let asks: Vec<&&str> = lines.iter().filter(|one| one.contains("\"ev\":\"ask\"")).collect();
    for how in ["hit", "recomputed"] {
        assert!(asks.iter().any(|one| one.contains(&format!("\"how\":\"{how}\""))), "no ask answered {how}");
    }
    assert!(
        asks.iter().any(|one| one.contains("\"how\":\"recomputed\"") && !one.contains("\"instr\":0,")),
        "a compute cost nothing"
    );
    let seqs: Vec<u64> = lines
        .iter()
        .filter(|one| one.contains("first.c@O2"))
        .map(|one| one.split("\"seq\":").nth(1).unwrap().split(',').next().unwrap().parse().unwrap())
        .collect();
    assert!(
        seqs.windows(2).filter(|pair| pair[0] > pair[1]).count() < seqs.len() / 4,
        "the sequence numbers do not order the lines"
    );
    let run = lines.iter().find(|one| one.contains("\"ev\":\"run\"")).unwrap();
    assert!(run.contains("\"insts\":") && run.contains("\"blocks\":") && run.contains("\"stage\":"), "{run}");
}

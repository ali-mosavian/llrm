//! The pieces of the C -> ours pipeline the benchmarks and the differential tests share.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use llrm_6502::RUNTIME;

/// The 6502 description's datalayout: n8 and 16-bit pointers.
pub fn datalayout() -> String {
    llrm_6502::LAYOUT
        .lines()
        .find_map(|l| l.strip_prefix("datalayout = "))
        .expect("a datalayout")
        .trim_matches('"')
        .to_owned()
}

/// The runtime routines `text` reaches, as ca65 text. Routines are the
/// top-level labels of `mos_rt.s`; one is reached when another's text names it.
pub fn reachable_runtime(text: &str) -> String {
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut bss = String::new();
    let mut in_bss = false;
    let mut pending = String::new();
    for line in RUNTIME.lines() {
        if line.starts_with(".segment \"BSS\"") {
            in_bss = true;
        }
        if in_bss {
            bss.push_str(line);
            bss.push('\n');
            if line.starts_with(".segment \"CODE\"") {
                in_bss = false;
            }
            continue;
        }
        let label = line
            .split(':')
            .next()
            .filter(
                |l| line.contains(':')
                    && l.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                    && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            );
        if let Some(label) = label {
            blocks.push((label.to_owned(), std::mem::take(&mut pending)));
        }
        if let Some((_, body)) = blocks.last_mut().filter(|_| !line.starts_with(".export") || label.is_some()) {
            if label.is_some() || !line.starts_with(".export") {
                body.push_str(line);
                body.push('\n');
            }
        }
        if line.starts_with(".export") {
            pending.push_str(line);
            pending.push('\n');
        }
    }
    let by_name: HashMap<&str, usize> = blocks.iter().enumerate().map(|(i, (n, _))| (n.as_str(), i)).collect();
    let mut wanted = vec![false; blocks.len()];
    let mut queue: Vec<usize> = Vec::new();
    let tokens = |s: &str| -> Vec<usize> {
        s.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter_map(|t| by_name.get(t).copied()).collect()
    };
    for i in tokens(text) {
        queue.push(i);
    }
    while let Some(i) = queue.pop() {
        if !std::mem::replace(&mut wanted[i], true) {
            queue.extend(tokens(&blocks[i].1).into_iter().filter(|&j| j != i));
        }
    }
    let mut out = String::new();
    let mut any = false;
    for (i, (_, body)) in blocks.iter().enumerate().filter(|(i, _)| wanted[*i]) {
        let _ = i;
        out.push_str(body);
        any = true;
    }
    if any && (out.contains("__sa") || out.contains("__ra")) {
        out.push_str(&bss);
    }
    out
}

/// clang's IR in MIR's subset: our datalayout, no attributes, linkage or
/// metadata decorations.
pub fn sanitize(ir: &str) -> String {
    const DROP: [&str; 12] = [
        " dso_local",
        " local_unnamed_addr",
        " noundef",
        " zeroext",
        " signext",
        " inbounds",
        " nuw",
        " nsw",
        " tail",
        " unnamed_addr",
        " nonnull",
        " noalias",
    ];
    let mut out = Vec::new();
    for line in ir.lines() {
        if line.starts_with("target triple")
            || line.starts_with("attributes ")
            || line.starts_with('!')
            || line.starts_with("source_filename")
        {
            continue;
        }
        if line.starts_with("target datalayout") {
            out.push(format!("target datalayout = \"{}\"", datalayout()));
            continue;
        }
        let mut line = line.to_owned();
        for word in DROP {
            line = line.replace(word, "");
        }
        // trailing `, !tbaa !2`, `, !llvm.loop !5` and ` #0` on define
        if let Some(at) = line.find(", !") {
            line.truncate(at);
        }
        if let Some(at) = line.find(" #") {
            let rest = line[at + 2..].trim_start_matches(|c: char| c.is_ascii_digit()).to_owned();
            line.truncate(at);
            line.push_str(&rest);
        }
        out.push(line);
    }
    out.join("\n")
}

pub fn tool(
    dir: &Path,
    program: &str,
    args: &[&str],
) -> Result<String, String> {
    let out = Command::new(program).args(args).current_dir(dir).output().map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("{program}: {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("")))
    }
}

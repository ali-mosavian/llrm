//! Loop unroll and jam against the pinned LLVM's own tests:
//! `tests/loopunrollandjam/*.ll` are `llvm/test/Transforms/LoopUnrollAndJam`
//! loop-simplified, in LCSSA form and without their CHECK lines, and
//! `*.expected` what `opt -passes=loop-unroll-and-jam` made of each of their
//! RUN lines, one form (blocks in depth-first order, blocks and values renamed
//! by place, phi arms sorted): `tools/loopunrollandjam_oracle.py --refresh`
//! writes them. The functions `tools/loopunrollandjam_expected.toml` lists
//! hold llrm's own output.

use std::path::Path;
use std::sync::LazyLock;

use llrm_support::hash::HashMap;
use regex::{Captures, Regex};

/// A regex compiled once.
macro_rules! re {
    ($pattern:expr) => {{
        static ONCE: LazyLock<Regex> = LazyLock::new(|| Regex::new($pattern).expect("a regex"));
        &*ONCE
    }};
}

use crate::loopunrollandjam::{JamOptions, LoopUnrollAndJam};

/// The bits of a floating-point literal as `opt` or MIR prints it.
fn fp_bits(
    literal: &str,
    single: bool,
) -> u64 {
    let literal = literal.strip_prefix('f').unwrap_or(literal);
    if let Some(hex) = literal.strip_prefix("0x") {
        return u64::from_str_radix(hex, 16).expect("hex bits");
    }
    let value: f64 = literal.parse().expect("a decimal literal");
    if single { f64::from(value as f32).to_bits() } else { value.to_bits() }
}

/// The function `body` in one form: `tools/loopunrollandjam_oracle.py`'s
/// `canonical`.
fn canonical(
    body: &str,
    nodes: &HashMap<String, String>,
) -> String {
    let mut blocks: HashMap<String, Vec<String>> = HashMap::default();
    let mut order: Vec<String> = Vec::new();
    let mut name: Option<String> = None;
    let mut entry_open = false;
    let label = re!(r"^([\w.$-]+):$");
    for raw in body.lines() {
        let line = raw.split(';').next().expect("a split").trim_end();
        let s = line.trim();
        if s.is_empty()
            || ["define", "declare", "target", "source_filename", "!", "attributes"].iter().any(|p| s.starts_with(p))
        {
            if s.starts_with("define") {
                entry_open = true;
            }
            continue;
        }
        if s == "}" {
            break;
        }
        if let Some(found) = label.captures(s) {
            let block = found[1].to_owned();
            blocks.insert(block.clone(), Vec::new());
            order.push(block.clone());
            name = Some(block);
            entry_open = false;
            continue;
        }
        if entry_open && name.is_none() {
            blocks.insert("%entry".to_owned(), Vec::new());
            order.push("%entry".to_owned());
            name = Some("%entry".to_owned());
            entry_open = false;
        }
        blocks.get_mut(name.as_ref().expect("a block")).expect("a block").push(s.to_owned());
    }
    let successors = re!(r"label %([\w.$-]+)");
    let mut seen: Vec<String> = Vec::new();
    let mut dfs: Vec<String> = Vec::new();
    let mut stack = vec![order[0].clone()];
    while let Some(block) = stack.pop() {
        if seen.contains(&block) {
            continue;
        }
        seen.push(block.clone());
        dfs.push(block.clone());
        let end = blocks[&block].last().expect("a terminator");
        let mut targets: Vec<String> = Vec::new();
        for found in successors.captures_iter(end) {
            let target = found[1].to_owned();
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
        stack.extend(targets.into_iter().rev());
    }
    let names: HashMap<&String, String> = dfs.iter().enumerate().map(|(n, block)| (block, format!("B{n}"))).collect();
    let mut values: HashMap<String, String> = HashMap::default();
    let defined = re!(r"^%([\w.$-]+) = ");
    for block in &dfs {
        for s in &blocks[block] {
            if let Some(found) = defined.captures(s) {
                let next = format!("v{}", values.len());
                values.insert(found[1].to_owned(), next);
            }
        }
    }
    let by_name: HashMap<String, String> = names.iter().map(|(block, new)| ((*block).clone(), new.clone())).collect();
    let rename = |text: &str| -> String {
        let text = re!(r"label %([\w.$-]+)").replace_all(text, |m: &Captures| {
            format!("label %{}", by_name.get(&m[1]).cloned().unwrap_or_else(|| m[1].to_owned()))
        });
        let text = re!(r"\[ ([^,\]]+), %([\w.$-]+) \]").replace_all(&text, |m: &Captures| {
            format!("[ {}, %{} ]", &m[1], by_name.get(&m[2]).cloned().unwrap_or_else(|| m[2].to_owned()))
        });
        re!(r"%([\w.$-]+)")
            .replace_all(&text, |m: &Captures| {
                format!(
                    "%{}",
                    values.get(&m[1]).or_else(|| by_name.get(&m[1])).cloned().unwrap_or_else(|| m[1].to_owned())
                )
            })
            .into_owned()
    };
    let align = re!(r", align \d+");
    let quoted = re!(r#"@"([^"]*)""#);
    let cc = re!(r"\bcc (\d+)");
    let float = re!(r"(?:^|[^\w.%@])(f?0x[0-9A-Fa-f]{16}|-?\d+\.\d+e[+-]\d+)\b");
    let prof = re!(r"!prof !(\d+)");
    let looped = re!(r",? ?!llvm\.loop (distinct )?!(\d+|\{[^}]*\})");
    let attachments = re!(r", !(tbaa|dbg|range|nonnull|align|noundef) !\d+");
    let phi = re!(r"^(%\w+ = phi \S+ )(.*)$");
    let arm = re!(r"\[ [^\]]* \]");
    let mut out: Vec<String> = Vec::new();
    for block in &dfs {
        out.push(format!("{}:", names[block]));
        for s in &blocks[block] {
            let s = align.replace_all(s, "").into_owned();
            let s = quoted.replace_all(&s, "@$1").into_owned();
            let s = cc.replace_all(&s, "cc$1").into_owned();
            let single = s.contains(" float ") && !s.contains(" double ");
            let s = float
                .replace_all(&s, |m: &Captures| {
                    let whole = m[0].to_owned();
                    let literal = &m[1];
                    let prefix = &whole[..whole.len() - literal.len()];
                    format!("{prefix}fp({:016X})", fp_bits(literal, single))
                })
                .into_owned();
            let s = prof
                .replace_all(&s, |m: &Captures| format!("!prof {}", nodes.get(&m[1]).map_or("?", String::as_str)))
                .into_owned();
            // D2: no loop metadata is made.
            let s = looped.replace_all(&s, "").into_owned();
            let s = attachments.replace_all(&s, "").into_owned();
            let s = re!(r", !llvm\.loop (!\d+|distinct !\{[^}]*\}|![^ ]*)").replace_all(&s, "").into_owned();
            let mut s = rename(&s);
            if let Some(found) = phi.captures(&s) {
                let mut arms: Vec<&str> = arm.find_iter(&found[2]).map(|one| one.as_str()).collect();
                arms.sort_unstable();
                s = format!("{}{}", &found[1], arms.join(", "));
            }
            out.push(format!("  {s}"));
        }
    }
    out.join("\n")
}

/// Each function of `text` with a body: its name and its text.
fn functions(text: &str) -> Vec<(String, String)> {
    let define = re!(r#"(?ms)^define [^\n]*@"?([\w.$-]+)"?\(.*?^\}$"#);
    define.captures_iter(text).map(|found| (found[1].to_owned(), found[0].to_owned())).collect()
}

/// The pass over `text` with `flags`, printed.
fn jammed(
    text: &str,
    flags: &str,
) -> String {
    let mut arguments: Vec<String> = flags.split_whitespace().map(str::to_owned).collect();
    let options = JamOptions::take_flags(&mut arguments).expect("known flags");
    assert!(arguments.is_empty(), "{arguments:?}");
    let mut module = llrm_mir::parse::module(text).expect("the corpus parses");
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopUnrollAndJam { opt_level: 2, opt_size: false, options });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("the pass runs");
    llrm_mir::print::module(&module)
}

/// Every run of every file of the corpus.
#[test]
fn every_run_of_the_pins_tests_makes_what_the_pin_makes() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/loopunrollandjam");
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .expect("tests/loopunrollandjam")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
        .collect();
    files.sort();
    assert!(files.len() >= 15, "the corpus is there: {}", files.len());

    let metadata = re!(r"(?m)^!(\d+) = (.*)$");
    let mut wrong = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("a file");
        let expected = std::fs::read_to_string(path.with_extension("expected")).expect("an expected file");
        for run in format!("\n{expected}").split("\n== ").skip(1) {
            let (flags, body) = run.split_once('\n').expect("a run");
            let got_text = jammed(&text, flags);
            let nodes: HashMap<String, String> =
                metadata.captures_iter(&got_text).map(|found| (found[1].to_owned(), found[2].to_owned())).collect();
            let got: String = functions(&got_text)
                .iter()
                .map(|(name, body)| format!("@{name}\n{}\n", canonical(body, &nodes)))
                .collect();
            if got.trim_end() != body.trim_end() {
                let at = got.lines().zip(body.lines()).find(|(a, b)| a != b);
                wrong.push(format!("{} [{}] {:?}", path.file_name().expect("a name").to_string_lossy(), flags, at));
            }
        }
    }
    assert!(wrong.is_empty(), "differ from the pin: {wrong:?}");
}

//! Loop interchange against the pinned LLVM's own tests:
//! `tests/loopinterchange/*.ll` are `llvm/test/Transforms/LoopInterchange`
//! loop-simplified, in LCSSA form and without their CHECK lines, and
//! `*.expected` what `opt -passes=loop-interchange` made of each of their RUN
//! lines, one form (blocks in depth-first order, blocks and values renamed by
//! place, phi arms sorted): `tools/loopinterchange_oracle.py --refresh` writes
//! them.

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

use crate::loopinterchange::{Interchange, Options};

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

/// The function `body` in one form: `tools/loopinterchange_oracle.py`'s
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
    let looped = re!(r"!llvm.loop !(\d+)");
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
            let s = looped
                .replace_all(&s, |m: &Captures| format!("!llvm.loop {}", nodes.get(&m[1]).map_or("?", String::as_str)))
                .into_owned();
            let s = attachments.replace_all(&s, "").into_owned();
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
fn interchanged(
    text: &str,
    flags: &str,
) -> String {
    let mut arguments: Vec<String> = flags.split_whitespace().map(str::to_owned).collect();
    let options = Options::take_flags(&mut arguments).expect("known flags");
    assert!(arguments.is_empty(), "{arguments:?}");
    let mut module = llrm_mir::parse::module(text).expect("the corpus parses");
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(Interchange { options });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("the pass runs");
    llrm_mir::print::module(&module)
}

/// Every run of every file of the corpus. `reduction2mem.ll` was the first to
/// differ: LCSSA formed by the function-wide pass put a new phi after the old
/// one in the nest's exit, where `formLCSSARecursively` puts it first, so the
/// dead reduction phi read the other of two equal phis.
#[test]
fn every_run_of_the_pins_tests_makes_what_the_pin_makes() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/loopinterchange");
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .expect("tests/loopinterchange")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ll"))
        .collect();
    files.sort();
    assert!(files.len() > 90, "the corpus is there: {}", files.len());

    let metadata = re!(r"(?m)^!(\d+) = (.*)$");
    let mut wrong = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("a file");
        let expected = std::fs::read_to_string(path.with_extension("expected")).expect("an expected file");
        for run in format!("\n{expected}").split("\n== ").skip(1) {
            let (flags, body) = run.split_once('\n').expect("a run");
            let got_text = interchanged(&text, flags);
            let nodes: HashMap<String, String> =
                metadata.captures_iter(&got_text).map(|found| (found[1].to_owned(), found[2].to_owned())).collect();
            let got: String = functions(&got_text)
                .iter()
                .map(|(name, body)| format!("@{name}\n{}\n", canonical(body, &nodes)))
                .collect();
            if got.trim_end() != body.trim_end() {
                wrong.push(format!("{} [{}]", path.file_name().expect("a name").to_string_lossy(), flags));
            }
        }
    }
    assert!(wrong.is_empty(), "differ from the pin: {wrong:?}");
}

/// `A[a * i]` is written on one cell every iteration if `%a` is zero. DA
/// answers `[0]` for the store against itself under the runtime assumption
/// `%a != 0` and `[S *]` without. `depends` was asked with assumptions on, so
/// the dependence matrix took the first as a fact and read the nest as legal
/// to go on with; the pin asks without (`UnderRuntimeAssumptions = false`) and
/// gives up on the matrix.
#[test]
fn a_dependence_that_needs_a_runtime_assumption_gives_up_the_matrix() {
    use llrm_analysis::aggregate::AggregateAA;
    use llrm_analysis::basicaa::BasicAA;
    use llrm_analysis::depanalysis::DependenceInfo;
    use llrm_analysis::scev::{Cx, ScalarEvolution};
    use llrm_analysis::typebasedaa::TypeBasedAA;
    use llrm_mir::domtree::DomTree;
    use llrm_mir::loopinfo::LoopInfo;
    use llrm_mir::valuetracking::AssumptionCache;

    let text = r#"define void @f(ptr noalias %A, ptr noalias %B, i32 %a) {
entry:
  br label %outer

outer:
  %j = phi i32 [ 0, %entry ], [ %j.next, %outer.latch ]
  br label %inner

inner:
  %i = phi i32 [ 0, %outer ], [ %i.next, %inner ]
  %offset = phi i32 [ 0, %outer ], [ %offset.next, %inner ]
  %gep.1 = getelementptr inbounds i8, ptr %A, i32 %offset
  store i8 1, ptr %gep.1
  %row = shl nuw nsw i32 %i, 6
  %idx = add nuw nsw i32 %row, %j
  %gep.2 = getelementptr inbounds i8, ptr %B, i32 %idx
  store i8 2, ptr %gep.2
  %i.next = add nuw nsw i32 %i, 1
  %offset.next = add nuw nsw i32 %offset, %a
  %ec = icmp eq i32 %i.next, 5
  br i1 %ec, label %outer.latch, label %inner

outer.latch:
  %j.next = add nuw nsw i32 %j, 1
  %ec2 = icmp eq i32 %j.next, 8
  br i1 %ec2, label %exit, label %outer

exit:
  ret void
}
"#;
    let module = llrm_mir::parse::module(text).expect("parses");
    let layout = llrm_mir::datalayout::DataLayout::default();
    let (_, _, function) = module.functions().find(|(_, _, f)| !f.layout().is_empty()).expect("a body");
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let unit = llrm_analysis::memory::Unit::of(&module, &layout, function);
    let mut aa = AggregateAA::new();
    aa.add_aa_result(BasicAA::new(&unit));
    aa.add_aa_result(TypeBasedAA::new(&unit));
    let options = Options::default();
    let mut info = DependenceInfo::new(&mut aa, &unit, &cx, options.da);
    let mut se = ScalarEvolution::new();
    let outermost = loops.top_level_loops().next().expect("a nest");
    let mut matrix = Vec::new();
    let found = super::populate_dependency_matrix(&mut matrix, 2, outermost, &mut info, &mut se, &cx, &options);
    assert!(!found, "{matrix:?}");
}

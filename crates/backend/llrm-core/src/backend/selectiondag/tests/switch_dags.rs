use llrm_mir::datalayout::DataLayout;

use super::super::builder::{BlockDag, BuilderTarget, build_block};
use super::super::dump::normalized;
use super::super::function_lowering::FunctionLoweringInfo;

fn flat(
    done: &BlockDag,
    out: &mut Vec<String>,
) {
    out.push(normalized(&done.dag.dump()));
    for (_, extra) in &done.extra {
        flat(extra, out);
    }
}

/// The blocks of each function, ours and the pin's, as sorted dumps.
fn ours(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let text: String =
        text.lines().filter(|line| !line.starts_with("target triple")).map(|line| format!("{line}\n")).collect();
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let mut all = std::collections::BTreeMap::new();
    for (id, global, function) in module.functions() {
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        let mut dumps = Vec::new();
        for &block in function.layout() {
            let done = build_block(
                &module,
                function,
                &layout,
                &BuilderTarget::m32(),
                &llrm_x86_m32::M32,
                &convention,
                &mut info,
                block,
            )
            .unwrap_or_else(|why| panic!("{} {block:?}: {}", global.name.as_deref().unwrap(), why.0));
            flat(&done, &mut dumps);
        }
        dumps.sort();
        all.insert(global.name.as_deref().unwrap().to_string(), dumps);
    }
    all
}

fn pins(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut all: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut current: Option<(String, String)> = None;
    let finish = |current: &mut Option<(String, String)>, all: &mut std::collections::BTreeMap<String, Vec<String>>| {
        if let Some((name, body)) = current.take() {
            all.entry(name).or_default().push(normalized(&body));
        }
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Initial selection DAG: ") {
            finish(&mut current, &mut all);
            let name = rest.split('\'').nth(1).unwrap().split(':').next().unwrap().to_string();
            current = Some((name, String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    finish(&mut current, &mut all);
    all.values_mut().for_each(|dumps| dumps.sort());
    all
}

/// The compare-and-branch blocks a range-only `switch` becomes (the split
/// compares, the chains, the or-trick) are the pin's, block for block.
/// A `switch` is refused before this piece.
#[test]
fn jump_table_and_bit_test_blocks_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("../golden/jtbt.ll")), pins(include_str!("../golden/jtbt.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}

#[test]
fn switch_blocks_match_the_pins_initial_dags() {
    let (ours, pins) =
        (ours(include_str!("../golden/switchdag.ll")), pins(include_str!("../golden/switchdag.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}

//! The pin's `llvm/test/Analysis/HashRecognize` tests: each function's
//! `CHECK` lines say whether `print<hash-recognize>` found a CRC loop, its
//! endianness, width, trip count, polynomial and lookup table, or why not.

use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Module;
use llrm_mir::valuetracking::AssumptionCache;

use super::{PolynomialInfo, gen_sarwate_table, recognize_crc};

#[derive(Debug, Default)]
struct Expected {
    name: String,
    found: Option<(bool, u32, u32, u128)>,
    table: Vec<u128>,
    reason: Option<String>,
}

fn expectations(text: &str) -> Vec<Expected> {
    let mut all: Vec<Expected> = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        let Some(check) = line.strip_prefix("; CHECK-") else { continue };
        if let Some(label) = check.strip_prefix("LABEL: '") {
            all.push(Expected { name: label.trim_end_matches('\'').to_owned(), ..Default::default() });
            in_table = false;
            continue;
        }
        let Some(current) = all.last_mut() else { continue };
        let body = check.trim_start_matches("NEXT:").trim();
        if let Some(rest) = body.strip_prefix("Found ") {
            let big = rest.starts_with("big");
            let width: u32 = rest.split("CRC-").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
            let tc: u32 = rest.rsplit(' ').next().unwrap().parse().unwrap();
            current.found = Some((big, width, tc, 0));
        } else if let Some(poly) = body.strip_prefix("Generating polynomial: ") {
            if let Some(found) = current.found.as_mut() {
                found.3 = poly.trim().parse().unwrap();
            }
        } else if body.starts_with("Computed CRC lookup table:") {
            in_table = true;
        } else if body.starts_with("Computed CRC Barrett") {
            in_table = false;
        } else if let Some(reason) = body.strip_prefix("Reason: ") {
            current.reason = Some(reason.to_owned());
        } else if in_table {
            current.table.extend(body.split_whitespace().map(|word| word.parse::<u128>().unwrap()));
        }
    }
    all
}

/// The loop of `@name` whose header is `%loop`, recognized, after the LCSSA
/// formation `opt` runs ahead of a loop printer.
fn recognized(
    module: &mut Module,
    name: &str,
) -> Result<PolynomialInfo, &'static str> {
    let (_, function) = module.function_mut(name).expect("the function");
    crate::lcssa::formed(function).expect("lcssa");
    let module = &*module;
    let (_, _, function) =
        module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).expect("the function");
    let layout = DataLayout::default();
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(module, function);
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
    let mut se = ScalarEvolution::new();
    let mut result = Err("Loop not in canonical form");
    for top in loops.top_level_loops() {
        let mut inner = vec![top];
        top.get_inner_loops_in_preorder(&mut inner);
        for inner in inner {
            result = recognize_crc(&cx, &mut se, inner);
            if result.is_ok() {
                return result;
            }
        }
    }
    result
}

fn check_file(file: &str) {
    let path = format!("{}/tests/hashrecognize/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("the fixture");
    let mut module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{file}: {error}"));
    let wanted = expectations(&text);
    assert!(!wanted.is_empty());
    for want in wanted {
        let got = recognized(&mut module, &want.name);
        match (&want.found, got) {
            (Some((big, width, tc, poly)), Ok(info)) => {
                assert_eq!(
                    (info.is_big_endian, info.rhs_width, info.trip_count, info.rhs),
                    (*big, *width, *tc, *poly),
                    "{}",
                    want.name
                );
                let table = gen_sarwate_table(info.rhs, info.rhs_width, info.is_big_endian);
                assert_eq!(table.to_vec(), want.table, "{}: the lookup table", want.name);
            }
            (None, Err(why)) => {
                if let Some(reason) = &want.reason {
                    assert_eq!(why, reason, "{}", want.name);
                }
            }
            (found, got) => panic!("{}: the pin says {found:?} ({:?}); this says {got:?}", want.name, want.reason),
        }
    }
}

/// The pin's CRC tests, little- and big-endian, with and without a data
/// recurrence, and the loops it refuses.
#[test]
fn test_the_pins_crc_recognition_cases() {
    check_file("cyclic-redundancy-check.ll");
}

/// A loop whose instructions have many uses each must not be walked once per
/// use (the pin hung).
#[test]
fn test_def_use_chains_are_visited_once() {
    check_file("def-use-chains-visited.ll");
}

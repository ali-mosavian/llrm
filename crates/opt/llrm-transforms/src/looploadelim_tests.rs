//! `LoopLoadElim` against the pinned LLVM's `opt -passes=loop-load-elim -S` on
//! `tests/lle/*.ll` (`*.expected` is `tools/llvmport/lle_expected.sh` on the
//! file): the function's body, up to the numbering of values and blocks.

use super::LoopLoadElim;
use crate::testing::{managed, parsed};

fn body(module: &llrm_mir::module::Module) -> String {
    let (_, _, function) = module.functions().find(|(_, _, function)| function.entry().is_some()).expect("a function");
    canonical(&llrm_mir::print::body(&module.context, function))
}

/// `text`, a function body as `print::body` writes it, with the blocks in the
/// order a walk from the entry reaches them (the true branch first) and the
/// blocks and values named by where they first appear: two bodies that differ
/// in their names and the layout of their blocks (LLVM puts a new preheader
/// before the header, `loop.preheader`; here it is added last) print alike.
fn canonical(text: &str) -> String {
    let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        if !line.starts_with(' ') && line.ends_with(':') {
            blocks.push((line.trim_end_matches(':').to_owned(), Vec::new()));
        } else {
            blocks.last_mut().expect("a block first").1.push(line.to_owned());
        }
    }
    let labels: Vec<String> = blocks.iter().map(|(label, _)| label.clone()).collect();
    let successors = |lines: &[String]| -> Vec<String> {
        let last = lines.last().map_or("", String::as_str);
        last.split("label %").skip(1).map(|rest| token(rest).to_owned()).collect()
    };
    // The walk.
    let mut order: Vec<usize> = Vec::new();
    let mut stack = vec![0usize];
    while let Some(at) = stack.pop() {
        if order.contains(&at) {
            continue;
        }
        order.push(at);
        for next in successors(&blocks[at].1).into_iter().rev() {
            if let Some(index) = labels.iter().position(|label| *label == next) {
                stack.push(index);
            }
        }
    }
    // Names by first appearance.
    let mut names: Vec<String> = Vec::new();
    let mut rename = |name: &str| -> String {
        let at = names
            .iter()
            .position(|one| one == name)
            .unwrap_or_else(
                || {
                    names.push(name.to_owned());
                    names.len() - 1
                },
            );
        format!("%v{at}")
    };
    let mut out = String::new();
    for &at in &order {
        out += &format!("{}:\n", rename(&blocks[at].0));
        for line in &blocks[at].1 {
            let mut rest = line.as_str();
            let mut written = String::new();
            while let Some(found) = rest.find('%') {
                written += &rest[..found];
                let name = token(&rest[found + 1..]);
                written += &rename(name);
                rest = &rest[found + 1 + name.len()..];
            }
            out += &written;
            out += rest;
            out.push('\n');
        }
    }
    out
}

/// The name that starts `text`.
fn token(text: &str) -> &str {
    let end =
        text.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '$' | '-'))).unwrap_or(text.len());
    &text[..end]
}

fn check(
    source: &str,
    expected: &str,
) {
    let mut module = parsed(source);
    managed(&mut module, LoopLoadElim::default());
    assert_eq!(body(&module), body(&parsed(expected)));
}

macro_rules! corpus {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check(
                    include_str!(concat!("../tests/lle/", stringify!($name), ".ll")),
                    include_str!(concat!("../tests/lle/", stringify!($name), ".expected")),
                );
            }
        )*
    };
}

corpus!(
    conditional_load,
    descending,
    distance_two,
    float_store_int_load,
    forward_one,
    load_used_after_loop,
    needs_check,
    recurrence,
    two_candidates,
    two_stores_same_block,
);

/// `n` counted loops in a row, each reading and writing `@a`.
fn counted_loops(n: usize) -> String {
    let mut text = String::from(
        "@a = global [64 x i32] zeroinitializer\n\ndefine void @f(i32 %n) {\nb0:\n  %g = icmp sgt i32 %n, 0\n  br i1 %g, label %p0, label %end\n\n",
    );
    for k in 0..n {
        let next = if k + 1 == n { "end".to_owned() } else { format!("p{}", k + 1) };
        text += &format!(
            "p{k}:\n  br label %l{k}\n\nl{k}:\n  %i{k} = phi i32 [ 0, %p{k} ], [ %n{k}, %l{k} ]\n  %q{k} = getelementptr inbounds i32, ptr @a, i32 %i{k}\n  %v{k} = load i32, ptr %q{k}\n  store i32 %v{k}, ptr %q{k}\n  %n{k} = add nuw nsw i32 %i{k}, 1\n  %c{k} = icmp eq i32 %n{k}, %n\n  br i1 %c{k}, label %x{k}, label %l{k}\n\nx{k}:\n  br label %{next}\n\n"
        );
    }
    text + "end:\n  ret void\n}\n"
}

/// The trip counts worked out by `LoopDeletion` and then `LoopLoadElim` over
/// `n` loops neither changes.
fn computed_counts(n: usize) -> usize {
    let mut module = parsed(&counted_loops(n));
    let before = llrm_analysis::scev::trip::backedge_taken_computations();
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::loopdeletion::LoopDeletion);
    manager.add(LoopLoadElim { max_target_vector_width_in_bits: 0 });
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    llrm_analysis::scev::trip::backedge_taken_computations() - before
}

/// `LoopLoadElim` made its own expressions and worked out each loop's trip
/// count again: 128 loops cost it 96 M instructions, 85 M of them in
/// `LoopAccessInfo`, where the pin's pass reads the count its analysis manager
/// already holds. A loop nothing changed is counted once across passes.
#[test]
fn a_loop_an_earlier_pass_counted_is_not_counted_again() {
    assert_eq!(computed_counts(4), 4);
    assert_eq!(computed_counts(16), 16);
}

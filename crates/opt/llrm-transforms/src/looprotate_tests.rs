//! `rotate_loops` on loops the interpreter runs, before and after.

use llrm_mir::module::Module;
use llrm_x86::tti::X86;

use super::{Options, rotate_function};
use crate::testing::{parsed, printed, results};

/// The loops of @f rotated with the pass's own options (header limit 16,
/// header duplication on); whether any was.
fn rotated(module: &mut Module) -> bool {
    // The loop pass manager hands the pass loops in simplified, LCSSA form.
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::lcssa::closed(function).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options = Options { max_header_size: 16, rotation_only: false, is_util_mode: false, prepare_for_lto: false };
    rotate_function(module, "f", &tti, options).expect("rotates")
}

const INPUTS: &[&[i128]] = &[&[1, 2], &[0, 5], &[-3, 7], &[100, -1], &[0, 0]];

/// Test in the header, a body in its own block: `for (i = 0; i < n; i++)`.
const COUNTED: &str = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %m = mul i16 %acc, 3
  %s = add i16 %m, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = phi i16 [ %acc, %b1 ]
  ret i16 %r
}
";

#[test]
fn test_a_counted_loop_is_entered_behind_a_guard_and_computes_the_same() {
    let mut module = parsed(COUNTED);
    let before = results(&module, INPUTS);
    assert!(rotated(&mut module));
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), before, "{text}");
    // The entry ends in the guard, a copy of the test on the first values, and
    // the test is in the loop's block alone (the old header merged into the
    // latch).
    let entry = text.split("\n\n").next().unwrap();
    assert!(entry.contains("icmp slt i16 0, %n") && entry.contains("br i1 %0"), "{text}");
    assert_eq!(text.matches("icmp slt").count(), 2, "{text}");
}

/// What @f returns for each of `inputs`, or how it fails (a loop that does not
/// end for some input does not end after).
fn outcomes(
    module: &Module,
    inputs: &[&[i128]],
) -> Vec<Result<llrm_mir::interpret::Val, String>> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    let widths: Vec<u32> = function
        .parameters()
        .iter()
        .map(|&one| module.context.types.int_bits(function.value(one).ty).expect("an integer"))
        .collect();
    inputs
        .iter()
        .map(|input| {
            let arguments = input
                .iter()
                .zip(&widths)
                .map(|(&value, &width)| llrm_mir::interpret::Val::Int {
                    bits: llrm_mir::context::mask(width) & value as u128,
                    width,
                })
                .collect();
            llrm_mir::interpret::run(module, "f", arguments, 20_000).map_err(|trap| format!("{trap:?}"))
        })
        .collect()
}

fn fixture_inputs(module: &Module) -> Option<Vec<Vec<i128>>> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f"))?;
    let widths: Option<Vec<u32>> =
        function.parameters().iter().map(|&one| module.context.types.int_bits(function.value(one).ty)).collect();
    let widths = widths?;
    if widths.iter().any(|&width| width == 1) {
        return None;
    }
    Some(
        [0i128, 1, 2, 5, 11, -3]
            .iter()
            .flat_map(|&x| [0i128, 1, 4, 9, 20].iter().map(move |&n| (x, n)))
            .map(|(x, n)| (0..widths.len()).map(|at| if at == 0 { x } else { n + at as i128 }).collect())
            .collect(),
    )
}

/// Every fixture of the oracle corpus that takes integers computes the same
/// before and after, and verifies after: the loops the oracle rotates like
/// `opt` rotate here to the same results.
#[test]
fn test_every_oracle_fixture_computes_the_same_after_rotation() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../tests/fixtures/rotate");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("the fixtures") {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut module = parsed(&text);
        let Some(inputs) = fixture_inputs(&module) else { continue };
        let inputs: Vec<&[i128]> = inputs.iter().map(Vec::as_slice).collect();
        let before = outcomes(&module, &inputs);
        rotated(&mut module);
        let text = printed(&module);
        assert_eq!(outcomes(&module, &inputs), before, "{}\n{text}", path.display());
        checked += 1;
    }
    assert!(checked >= 10, "{checked} fixtures run");
}

/// `n` loops one after the other, each `for (i = 0; i < n; i++) acc += i`.
fn sequential_loops(n: usize) -> String {
    let mut text = String::from("define i16 @f(i16 %x, i16 %n) {\nb0:\n  br label %h0\n\n");
    for k in 0..n {
        let (this, next) = (format!("h{k}"), if k + 1 == n { "done".to_owned() } else { format!("h{}", k + 1) });
        let before = if k == 0 { "b0".to_owned() } else { format!("e{}", k - 1) };
        let acc_in = if k == 0 { "%x".to_owned() } else { format!("%r{}", k - 1) };
        text += &format!(
            "{this}:\n  %i{k} = phi i16 [ 0, %{before} ], [ %n{k}, %c{k} ]\n  %a{k} = phi i16 [ {acc_in}, %{before} ], [ %s{k}, %c{k} ]\n  %g{k} = icmp slt i16 %i{k}, %n\n  br i1 %g{k}, label %c{k}, label %e{k}\n\nc{k}:\n  %s{k} = add i16 %a{k}, %i{k}\n  %n{k} = add i16 %i{k}, 1\n  br label %{this}\n\ne{k}:\n  %r{k} = phi i16 [ %a{k}, %{this} ]\n  br label %{next}\n\n"
        );
    }
    text += &format!("done:\n  ret i16 %r{}\n}}\n", n - 1);
    text
}

/// Rotating `n` loops one after the other scans entries of the layout and of
/// use lists in proportion to `n`: a `LoopInfo` made again after each rotated
/// loop made it `n^2` (a count of entries looked over, not seconds: time fails
/// under load).
#[test]
fn test_rotating_loops_in_sequence_scans_in_proportion_to_their_number() {
    use llrm_mir::edit::layout_scanned;
    let scanned = |n: usize| {
        let mut module = parsed(&sequential_loops(n));
        let before = layout_scanned();
        assert!(rotated(&mut module));
        printed(&module);
        layout_scanned() - before
    };
    let (a, b, c) = (scanned(64), scanned(128), scanned(256));
    assert!(b * 100 <= a * 250 && c * 100 <= b * 250, "64: {a}, 128: {b}, 256: {c}");
}

/// A target with no cost model is priced by the generic profile, and the pass
/// names no target crate: a release build of the bins failed on `llrm_x86`
/// used outside the dev-dependencies (`cargo check --bins` is the test).
#[test]
fn test_the_slot_gets_its_cost_model_from_the_target() {
    let module = parsed("define i16 @f() {\nb:\n  ret i16 0\n}\n");
    let neutral = llrm_mir::passes::Outer::of(&module, None);
    // The generic profile: a phi is free, a branch costs one.
    let generic = super::target_tti(neutral.target());
    assert_eq!(generic.cf_instr_cost(true, llrm_mir::tti::CostKind::CodeSize), llrm_mir::tti::Cost::FREE);
    assert_eq!(generic.cf_instr_cost(false, llrm_mir::tti::CostKind::CodeSize), llrm_mir::tti::Cost::BASIC);
}

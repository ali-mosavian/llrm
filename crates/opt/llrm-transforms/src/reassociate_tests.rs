//! Each body is a test of LLVM's `test/Transforms/Reassociate` at d1106deb71cc,
//! run with `opt -passes=reassociate`: the expected body has the structure of
//! the file's CHECK lines (new instructions are unnamed here, so the printer
//! numbers them).

use llrm_mir::interpret::Val;
use llrm_mir::module::Module;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Outer;

use super::reassociate;
use crate::testing::{parsed, printed, results};

/// @f after Reassociate; whether anything changed.
fn reassociated(module: &mut Module) -> bool {
    let outer = Outer::of(module, None);
    let layout = llrm_analysis::testing::layout(module);
    let (context, function) = module.function_mut("f").expect("@f");
    reassociate(context, &layout, &outer, function)
}

/// Names of values and blocks by first appearance, so that two bodies compare
/// by structure.
fn canonical(text: &str) -> String {
    let mut names: Vec<String> = Vec::new();
    let mut id = |name: &str| {
        let name = name.to_lowercase();
        let at = names
            .iter()
            .position(|seen| *seen == name)
            .unwrap_or_else(
                || {
                    names.push(name);
                    names.len() - 1
                },
            );
        format!("%v{at}")
    };
    let word = |c: char| c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '-');
    let mut out = String::new();
    for line in text.lines() {
        // MIR has no `align` on a load or a store.
        let line = line.trim();
        let line = match line.rfind(", align ") {
            Some(at) if line[at + 8..].chars().all(|c| c.is_ascii_digit()) => &line[..at],
            _ => line,
        };
        if let Some(label) = line.strip_suffix(':').filter(|label| !label.is_empty() && label.chars().all(word)) {
            out += &format!("{}:\n", id(label));
            continue;
        }
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            let mut name = String::new();
            while let Some(&next) = chars.peek().filter(|&&next| word(next)) {
                name.push(next);
                chars.next();
            }
            out += &id(&name);
        }
        out.push('\n');
    }
    out
}

/// `@f` of `text`: from `define` to its closing brace.
fn function_text(text: &str) -> String {
    let from = text.find("define ").expect("a definition");
    let to = text[from..].find("\n}").expect("a closing brace");
    text[from..from + to + 2].to_owned()
}

/// `text` after Reassociate has the structure of `expected` (up to names),
/// and computes what it did on `inputs`, where there are any.
pub(super) fn check_lit(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    lit(text, expected, inputs, false);
}

/// `check_lit` after `dce` (the file's `-passes=reassociate,dce`).
pub(super) fn check_lit_dce(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    lit(text, expected, inputs, true);
}

fn lit(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
    dce: bool,
) {
    let mut module = parsed(text);
    let before = (!inputs.is_empty()).then(|| results(&module, inputs));
    reassociated(&mut module);
    if dce {
        let (_, function) = module.function_mut("f").expect("@f");
        loop {
            let dead: Vec<_> = function
                .walk()
                .map(|(_, inst)| inst)
                .filter(|&inst| {
                    let instruction = function.instruction(inst);
                    matches!(instruction.opcode, Opcode::Binary(_) | Opcode::Cast(_))
                        && instruction.result.is_some_and(|result| function.users(result).is_empty())
                })
                .collect();
            if dead.is_empty() {
                break;
            }
            for inst in dead {
                function.erase(inst).expect("unused");
            }
        }
    }
    let after = printed(&module);
    assert_eq!(canonical(&function_text(&after)), canonical(expected), "{after}");
    if let Some(before) = before {
        // The result may refine poison to a value; it may not change a value.
        for (after, before) in results(&module, inputs).into_iter().zip(before) {
            assert!(before == Val::Poison || after == before, "{after:?} for {before:?}\n{text}");
        }
    }
}

/// `text` after Reassociate is `expected`.
fn check_text(
    text: &str,
    expected: &str,
) {
    let mut module = parsed(text);
    let changed = reassociated(&mut module);
    assert_eq!(printed(&module), expected);
    assert_eq!(changed, text != expected);
}

/// `check_text`, and the body computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let before = results(&parsed(text), inputs);
    check_text(text, expected);
    assert_eq!(results(&parsed(expected), inputs), before);
}

/// `basictest.ll` @test2: the constants of `(x + -30) + y + 30` meet and go.
#[test]
fn test_constants_of_an_add_chain_meet_and_cancel() {
    check(
        "define i32 @f(i32 %reg109, i32 %reg1111) {
b0:
  %reg115 = add i32 %reg109, -30
  %reg116 = add i32 %reg115, %reg1111
  %reg117 = add i32 %reg116, 30
  ret i32 %reg117
}
",
        "define i32 @f(i32 %reg109, i32 %reg1111) {
b0:
  %reg117 = add i32 %reg1111, %reg109
  ret i32 %reg117
}
",
        &[&[1, 2], &[-5, 7]],
    );
}

/// `absorption.ll` @or_all_ones: constants that combine to the absorbing value
/// are the value.
#[test]
fn test_constants_that_or_to_all_ones_are_the_result() {
    check(
        "define i8 @f(i8 %x) {
b0:
  %tmp1 = or i8 %x, 127
  %tmp2 = or i8 %tmp1, -128
  ret i8 %tmp2
}
",
        "define i8 @f(i8 %x) {
b0:
  ret i8 -1
}
",
        &[&[0], &[5]],
    );
}

/// `binop-identity.ll` @add_0 .. @xor_0: constants that combine to the
/// identity add nothing, so no instruction is made.
#[test]
fn test_constants_that_combine_to_the_identity_leave_the_operand() {
    for (op, c1, c2) in
        [("add", "-30", "30"), ("mul", "-1", "-1"), ("and", "-1", "-1"), ("or", "0", "0"), ("xor", "42", "42")]
    {
        check(
            &format!(
                "define i8 @f(i8 %x) {{
b0:
  %a1 = {op} i8 %x, {c1}
  %a2 = {op} i8 %a1, {c2}
  ret i8 %a2
}}
"
            ),
            "define i8 @f(i8 %x) {
b0:
  ret i8 %x
}
",
            &[&[0], &[7], &[-3]],
        );
    }
}

/// `binop-identity.ll` @fmul_1: `x * 4.0 * 0.25` is `x`.
#[test]
fn test_float_constants_that_multiply_to_one_leave_the_operand() {
    check_text(
        "define float @f(float %x) {
b0:
  %a1 = fmul fast float %x, 4.0
  %a2 = fmul fast float %a1, 0.25
  ret float %a2
}
",
        "define float @f(float %x) {
b0:
  ret float %x
}
",
    );
}

/// `commute.ll` @test1: `x*y` and `y*x` get one operand order.
#[test]
fn test_commuted_products_get_one_operand_order() {
    check_text(
        "declare void @use(i32)

define void @f(i32 %x, i32 %y) {
b0:
  %0 = mul i32 %x, %y
  %1 = mul i32 %y, %x
  %2 = sub i32 %0, %1
  call void @use(i32 %0)
  call void @use(i32 %2)
  ret void
}
",
        "declare void @use(i32)

define void @f(i32 %x, i32 %y) {
b0:
  %0 = mul i32 %y, %x
  %1 = mul i32 %y, %x
  %2 = sub i32 %0, %1
  call void @use(i32 %0)
  call void @use(i32 %2)
  ret void
}
",
    );
}

/// `add-like-or.ll` @shl_add: an `or` of shifted copies of one value has no
/// common bit, so it is an add.
#[test]
fn test_an_or_of_disjoint_shifts_is_an_add() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %conv = zext i8 %x to i32
  %shl = shl nuw nsw i32 %conv, 8
  %shl2 = shl nuw nsw i32 %conv, 16
  %add = or i32 %shl, %shl2
  ret i32 %add
}
",
        "define i32 @f(i8 %x) {
b0:
  %conv = zext i8 %x to i32
  %shl = shl nuw nsw i32 %conv, 8
  %shl2 = shl nuw nsw i32 %conv, 16
  %add = add nuw nsw i32 %shl, %shl2
  ret i32 %add
}
",
        &[&[1], &[255]],
    );
}

/// `add-like-or.ll` @test1: an `or` whose operands may share a bit stays.
#[test]
fn test_an_or_that_may_share_bits_is_not_an_add() {
    check(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %c = or i32 %a, %b
  %c.plus.one = add i32 %c, 1
  ret i32 %c.plus.one
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %c = or i32 %b, %a
  %c.plus.one = add i32 %c, 1
  ret i32 %c.plus.one
}
",
        &[&[1, 2], &[3, 3]],
    );
}

/// `add-like-or.ll` @test4: a `disjoint` `or` reassociates as an add.
#[test]
fn test_a_disjoint_or_reassociates_as_an_add() {
    check(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %c = or disjoint i32 %a, %b
  %c.plus.one = add i32 %c, 1
  ret i32 %c.plus.one
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %c = add i32 %a, 1
  %c.plus.one = add i32 %c, %b
  ret i32 %c.plus.one
}
",
        &[&[1, 2], &[4, 8]],
    );
}

#[test]
fn test_mulfactor_test1() {
    check(
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %t2 = mul i32 %a, %a
  %t5 = shl i32 %a, 1
  %t6 = mul i32 %t5, %b
  %t8 = mul i32 %b, %b
  %t7 = add i32 %t6, %t2
  %t11 = add i32 %t7, %t8
  ret i32 %t11
}
",
        "define i32 @f(i32 %a, i32 %b) {
b0:
  %t2 = mul i32 %a, %a
  %t6 = mul i32 %a, 2
  %0 = add i32 %t6, %b
  %1 = mul i32 %0, %b
  %t11 = add i32 %1, %t2
  ret i32 %t11
}
",
        &[&[1, 2], &[3, -4], &[5, 5]],
    );
}

#[test]
fn test_mulfactor_test2() {
    check(
        "define i32 @f(i32 %t) {
b0:
  %a = mul i32 %t, 6
  %b = mul i32 %t, 36
  %c = add i32 %b, 15
  %d = add i32 %c, %a
  ret i32 %d
}
",
        "define i32 @f(i32 %t) {
b0:
  %0 = mul i32 %t, 42
  %d = add i32 %0, 15
  ret i32 %d
}
",
        &[&[1], &[2], &[-7]],
    );
}

#[test]
fn test_mulfactor_test3() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %a = mul i32 %x, %x
  %b = mul i32 %a, %x
  %c = mul i32 %b, %x
  %d = mul i32 %c, %x
  %e = mul i32 %d, %x
  %f = mul i32 %e, %x
  %g = mul i32 %f, %x
  ret i32 %g
}
",
        "define i32 @f(i32 %x) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %0
  %2 = mul i32 %1, %1
  ret i32 %2
}
",
        &[&[2], &[3]],
    );
}

#[test]
fn test_mulfactor_test4() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %a = mul i32 %x, %x
  %b = mul i32 %a, %x
  %c = mul i32 %b, %x
  %d = mul i32 %c, %x
  %e = mul i32 %d, %x
  %f = mul i32 %e, %x
  ret i32 %f
}
",
        "define i32 @f(i32 %x) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %x
  %2 = mul i32 %1, %x
  %f = mul i32 %2, %1
  ret i32 %f
}
",
        &[&[2], &[3]],
    );
}

#[test]
fn test_mulfactor_test5() {
    check(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %a = mul i32 %x, %y
  %b = mul i32 %a, %y
  %c = mul i32 %b, %x
  %d = mul i32 %c, %x
  %e = mul i32 %d, %x
  ret i32 %e
}
",
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %y
  %2 = mul i32 %1, %1
  ret i32 %2
}
",
        &[&[2, 3], &[3, 2]],
    );
}

#[test]
fn test_mulfactor_test6() {
    check(
        "define i32 @f(i32 %x, i32 %y, i32 %z) {
b0:
  %a = mul i32 %x, %y
  %b = mul i32 %a, %x
  %c = mul i32 %b, %y
  %d = mul i32 %c, %x
  %e = mul i32 %d, %y
  %f = mul i32 %e, %x
  %g = mul i32 %f, %z
  %h = mul i32 %g, %x
  ret i32 %h
}
",
        "define i32 @f(i32 %x, i32 %y, i32 %z) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %y
  %f = mul i32 %y, %x
  %g = mul i32 %f, %1
  %2 = mul i32 %g, %1
  %h = mul i32 %2, %z
  ret i32 %h
}
",
        &[&[2, 3, 5]],
    );
}

#[test]
fn test_mulfactor_test7() {
    check(
        "define i32 @f(i32 %x, i32 %y, i32 %z) {
b0:
  %a = mul i32 %y, %x
  %b = mul i32 %a, %z
  %c = mul i32 %b, %z
  %d = mul i32 %c, %x
  %e = mul i32 %d, %y
  %f = mul i32 %e, %y
  %g = mul i32 %f, %x
  %h = mul i32 %g, %x
  ret i32 %h
}
",
        "define i32 @f(i32 %x, i32 %y, i32 %z) {
b0:
  %0 = mul i32 %x, %x
  %1 = mul i32 %0, %y
  %2 = mul i32 %1, %z
  %3 = mul i32 %2, %y
  %h = mul i32 %3, %2
  ret i32 %h
}
",
        &[&[2, 3, 5]],
    );
}

#[test]
fn test_multistep1() {
    check(
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
  %t0 = mul i64 %a, %b
  %t1 = mul i64 %a, %t0
  %t2 = mul i64 %a, %c
  %t3 = mul i64 %a, %t2
  %t4 = add i64 %t1, %t3
  ret i64 %t4
}
",
        "define i64 @f(i64 %a, i64 %b, i64 %c) {
b0:
  %0 = add i64 %c, %b
  %1 = mul i64 %a, %a
  %2 = mul i64 %1, %0
  ret i64 %2
}
",
        &[&[2, 3, 5]],
    );
}

#[test]
fn test_multistep2() {
    check(
        "define i64 @f(i64 %a, i64 %b, i64 %c, i64 %d) {
b0:
  %t0 = mul i64 %a, %b
  %t1 = mul i64 %a, %c
  %t2 = add i64 %t1, %d
  %t3 = add i64 %t0, %t2
  ret i64 %t3
}
",
        "define i64 @f(i64 %a, i64 %b, i64 %c, i64 %d) {
b0:
  %0 = add i64 %c, %b
  %1 = mul i64 %0, %a
  %t3 = add i64 %1, %d
  ret i64 %t3
}
",
        &[&[2, 3, 5, 7]],
    );
}

/// The steps the pass takes on `text`'s `@f`.
fn work(text: &str) -> u64 {
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = llrm_analysis::testing::layout(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let env = llrm_mir::constantfolding::Env { globals: &outer.globals, layout: &layout };
    let mut run = super::Reassociator::new(context, env, &outer, function);
    run.run();
    run.work_done()
}

/// Four times the input takes at most 4.5 times the steps: a quadratic one
/// takes 16 times.
fn assert_linear(text: impl Fn(usize) -> String) {
    let (small, large) = (work(&text(100)), work(&text(400)));
    assert!(large * 10 <= small * 45, "{small} steps for 100, {large} for 400");
}

fn params(n: usize) -> String {
    (0..n).map(|k| format!("i32 %a{k}")).collect::<Vec<_>>().join(", ")
}

/// A sum of n arguments added one at a time: a tree of n nodes, linearized
/// once, not once per node.
#[test]
fn test_a_long_add_chain_takes_linear_steps() {
    assert_linear(|n| {
        let mut body = String::from("  %s1 = add i32 %a0, %a1\n");
        for k in 2..n {
            body += &format!("  %s{k} = add i32 %s{}, %a{k}\n", k - 1);
        }
        format!("define i32 @f({}) {{\nb0:\n{body}  ret i32 %s{}\n}}\n", params(n), n - 1)
    });
}

/// `x + x + ... + x`: the repeats are removed in one move, not one shift
/// each.
#[test]
fn test_a_sum_of_one_value_n_times_takes_linear_steps() {
    assert_linear(|n| {
        let mut body = String::from("  %s1 = add i32 %a0, %a0\n");
        for k in 2..n {
            body += &format!("  %s{k} = add i32 %s{}, %a0\n", k - 1);
        }
        format!("define i32 @f(i32 %a0) {{\nb0:\n{body}  ret i32 %s{}\n}}\n", n - 1)
    });
}

/// `((a0 - a1) - a2) - ...`: each subtract is broken up into an add of a
/// negate once.
#[test]
fn test_a_left_nested_subtract_chain_takes_linear_steps() {
    assert_linear(|n| {
        let mut body = String::from("  %s1 = sub i32 %a0, %a1\n");
        for k in 2..n {
            body += &format!("  %s{k} = sub i32 %s{}, %a{k}\n", k - 1);
        }
        format!("define i32 @f({}) {{\nb0:\n{body}  ret i32 %s{}\n}}\n", params(n), n - 1)
    });
}

/// `x * x * ... * x`: the factors are counted once.
#[test]
fn test_a_product_of_one_value_n_times_takes_linear_steps() {
    assert_linear(|n| {
        let mut body = String::from("  %s1 = mul i32 %a0, %a0\n");
        for k in 2..n {
            body += &format!("  %s{k} = mul i32 %s{}, %a0\n", k - 1);
        }
        format!("define i32 @f(i32 %a0) {{\nb0:\n{body}  ret i32 %s{}\n}}\n", n - 1)
    });
}

/// n independent small expressions in one block: each is visited once, and
/// the pair map is built once.
#[test]
fn test_many_small_expressions_take_linear_steps() {
    assert_linear(|n| {
        let mut body = String::new();
        for k in 0..n {
            body +=
                &format!("  %p{k} = add i32 %a0, 5\n  %q{k} = add i32 %p{k}, %a1\n  %r{k} = mul i32 %q{k}, %q{k}\n");
        }
        let sum = (0..n).fold(String::from("  %t0 = add i32 0, 0\n"), |text, k| {
            format!("{text}  %t{} = xor i32 %t{k}, %r{k}\n", k + 1)
        });
        format!("define i32 @f(i32 %a0, i32 %a1) {{\nb0:\n{body}{sum}  ret i32 %t{n}\n}}\n")
    });
}

/// bench row x_strlen: LLVM's `NegateValue` of `ptrtoint (ptr @g to i32)` is
/// the constant `sub (i32 0, ptrtoint ...)`; MIR has no constant `sub`, so it
/// is an instruction (DEVIATE 4), used by the add and not left over.
#[test]
fn test_the_negate_of_a_constant_expression_is_an_instruction_the_add_uses() {
    check_text(
        "@g = global i32 0

define i32 @f(i32 %x, i32 %y) {
b0:
  %a = sub i32 %x, ptrtoint (ptr @g to i32)
  %b = add i32 %a, %y
  ret i32 %b
}
",
        "@g = global i32 0

define i32 @f(i32 %x, i32 %y) {
b0:
  %0 = sub i32 0, ptrtoint (ptr @g to i32)
  %a = add i32 %x, %0
  %b = add i32 %a, %y
  ret i32 %b
}
",
    );
}

/// An add already in the order the pass wants is swapped by
/// `canonicalizeOperands` and swapped back by the rewrite: upstream's
/// `MadeChange` is true, the body is the same. The pipeline's rounds ran until
/// no pass changed a body, so the pass never settled (corpus entry
/// nib-examples-level: unbounded growth of the test process to 158 GB).
#[test]
fn test_a_body_the_pass_leaves_as_it_was_is_not_reported_changed() {
    let mut module = parsed(
        "define i32 @f(i32 %x, i32 %y) {
b0:
  %a = add i32 %y, %x
  ret i32 %a
}
",
    );
    assert!(!reassociated(&mut module));
    assert!(!reassociated(&mut module));
}

/// A block no path reaches may hold an instruction that reads itself (legal in
/// LLVM, `Only PHI nodes may reference their own value` is for reachable code).
/// The block order of the pass included it, the rank walk never ended, and the
/// test process grew to 158 GB (corpus entry nib-examples-level). Reassociate
/// walks the blocks a reverse post-order of the entry reaches, as upstream, so
/// the block is not ranked or visited.
#[test]
fn test_an_unreachable_block_that_reads_itself_is_left_alone() {
    let text = "define i32 @f(i32 %x) {
b0:
  ret i32 %x

dead:
  %a = add nuw nsw i32 %a, 1
  %b = add i32 %a, %x
  br label %dead
}
";
    let mut module = parsed(text);
    assert!(!reassociated(&mut module));
    assert_eq!(llrm_mir::print::module(&module), llrm_mir::print::module(&parsed(text)));
}

/// `digest` (what says whether the pass changed a body) tells two bodies
/// that differ only in an operand order apart, and the same body the same:
/// it was SipHash, 2.0 M of one run's 3.5 M Ir on d_faces -O2, and is Fx.
#[test]
fn test_the_digest_of_a_body_sees_an_operand_swap() {
    let body = |first: &str, second: &str| {
        let module = parsed(&format!(
            "define i32 @f(i32 %a, i32 %b) {{\nb0:\n  %x = sub i32 {first}, {second}\n  ret i32 %x\n}}\n"
        ));
        let (_, _, function) =
            module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
        super::digest(function)
    };
    assert_eq!(body("%a", "%b"), body("%a", "%b"));
    assert_ne!(body("%a", "%b"), body("%b", "%a"));
}

mod operand_map_footprint {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    use llrm_mir::module::{Operand, ValueId};

    use crate::reassociate::OperandMap;

    thread_local! {
        static LARGEST: Cell<usize> = const { Cell::new(0) };
    }

    struct Largest;

    // SAFETY: forwards to `System`, only noting the size.
    unsafe impl GlobalAlloc for Largest {
        unsafe fn alloc(
            &self,
            layout: Layout,
        ) -> *mut u8 {
            let _ = LARGEST.try_with(|largest| largest.set(largest.get().max(layout.size())));
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(
            &self,
            ptr: *mut u8,
            layout: Layout,
        ) {
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(
            &self,
            ptr: *mut u8,
            layout: Layout,
            new_size: usize,
        ) -> *mut u8 {
            let _ = LARGEST.try_with(|largest| largest.set(largest.get().max(new_size)));
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: Largest = Largest;

    /// A map made for one expression tree held a page table as long as the
    /// highest value id (about 580 Ir to drop each at 256 chained adds, the
    /// reassociate stage 1.9x the pin and growing faster).
    #[test]
    fn test_one_leaf_of_a_late_value_allocates_a_few_bytes() {
        LARGEST.with(|largest| largest.set(0));
        let mut leaves: OperandMap<u64> = OperandMap::new();
        leaves.insert(Operand::Value(ValueId(1 << 24)), 1);
        assert_eq!(leaves.get(Operand::Value(ValueId(1 << 24))), Some(&1));
        assert!(LARGEST.with(Cell::get) < 4096);
    }
}

//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let options = Options { speculate_blocks: false, sink_common_insts: true, ..Default::default() };
    let mut module = parsed(text);
    let before = (!inputs.is_empty()).then(|| results(&module, inputs));
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified_with(&options, context, &spec, &mut declared, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    if let Some(before) = before {
        assert_eq!(results(&module, inputs), before);
    }
}

/// Two arms end in `add x, 1; store` with different `x`: both run once in the
/// join, on a phi of the two (LLVM's `sink` of a column and its user); the
/// emptied arms then go and the phi is a select, as in `opt`.
#[test]
fn test_the_instructions_both_arms_end_with_run_once_in_the_join() {
    check(
        "define void @f(i16 %p, i16 %q, ptr %g, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %x = add i16 %p, 1
  store i16 %x, ptr %g
  br label %e

b2:
  %y = add i16 %q, 1
  store i16 %y, ptr %g
  br label %e

e:
  ret void
}
",
        "define void @f(i16 %p, i16 %q, ptr %g, i1 %c) {
b0:
  %0 = select i1 %c, i16 %p, i16 %q
  %y = add i16 %0, 1
  store i16 %y, ptr %g
  ret void
}
",
        &[],
    );
}

/// Different operations are not one.
#[test]
fn test_an_add_and_a_sub_are_not_sunk() {
    let text = "define i16 @f(i16 %p, i16 %q, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %x = add i16 %p, 1
  br label %e

b2:
  %y = sub i16 %q, 1
  br label %e

e:
  %z = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  ret i16 %z
}
";
    check(text, text, &[&[3, 4, 1], &[3, 4, 0]]);
}

/// A way into the join that is not a jump stays out of it: the jumps get a
/// block of their own to sink into, when what is sunk cannot run
/// speculatively (a store). The two ways then fold into one branch
/// (`SimplifyCondBranchToCondBranch`) and the sunk block's phi of one input is
/// gone, as `opt -passes='simplifycfg<sink-common-insts>'` has it: `%brmerge`
/// select, `%p.mux` select, `store i16 1, ptr %p.mux` in `end.sink.split`.
#[test]
fn test_a_conditional_way_in_makes_the_jumps_a_block_to_sink_into() {
    check(
        "define void @f(ptr %p, ptr %q, i1 %c, i1 %d) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr %p
  br label %end

b2:
  br i1 %d, label %b3, label %end

b3:
  store i16 1, ptr %q
  br label %end

end:
  ret void
}
",
        "define void @f(ptr %p, ptr %q, i1 %c, i1 %d) {
b0:
  %or.cond = select i1 %c, i1 true, i1 %d
  %mux = select i1 %c, ptr %p, ptr %q
  br i1 %or.cond, label %0, label %end

0:
  store i16 1, ptr %mux
  br label %end

end:
  ret void
}
",
        &[],
    );
}

/// Many jump-only predecessors that give the join the same value are one:
/// every duplicate is found by hashing what the phis take from it (a scan of
/// the phi's arms and of the kept ones for each was quadratic).
#[test]
fn test_hundreds_of_identical_jump_only_predecessors_are_one() {
    let n = 400;
    let mut text = String::from("define i16 @f(i16 %x) {\nb0:\n  switch i16 %x, label %j0 [\n");
    for i in 1..n {
        text += &format!("    i16 {i}, label %j{i}\n");
    }
    text += "  ]\n\n";
    for i in 0..n {
        text += &format!("j{i}:\n  br label %e\n\n");
    }
    text += "e:\n  %p = phi i16 [ 1, %j0 ]";
    for i in 1..n {
        text += &format!(", [ 1, %j{i} ]");
    }
    text += "\n  ret i16 %p\n}\n";
    let options = Options { speculate_blocks: false, sink_common_insts: true, ..Default::default() };
    let mut module = parsed(&text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(simplified_with(&options, context, &spec, &mut declared, function));
    let printed = printed(&module);
    assert!(printed.contains("ret i16"), "{printed}");
    assert!(printed.matches("br label").count() <= 1, "{printed}");
}

/// `store i16` and `store i8` are not the same operation: the operand types
/// differ (`isSameOperationAs`). Both stores were sunk on a phi of an `i16`
/// and an `i8`, which sccp's solver met at -O2 with `decide@832` wired.
#[test]
fn test_stores_of_different_widths_are_not_sunk_together() {
    const TEXT: &str = "define void @f(i16 %p, i8 %q, ptr %g, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %p, ptr %g
  br label %e

b2:
  store i8 %q, ptr %g
  br label %e

e:
  ret void
}
";
    check(TEXT, TEXT, &[]);
}

/// `sunk` over `text` on `tti`, printed.
fn sunk_on(
    tti: &dyn llrm_mir::tti::Tti,
    text: &str,
) -> String {
    let options = Options { speculate_blocks: false, sink_common_insts: true, ..Default::default() };
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified_with(&options, context, &spec, &mut declared, function);
    printed(&module)
}

/// Two arms load `[p+2]` and `[p+4]`. The pin sinks one load behind a phi of
/// the offsets; where the target keeps constant offsets as displacements
/// (`keep_constant_displacements`) it stays two loads. qbsp `r_point_leaf` at
/// m16 -O2 paid +6 bytes and +36 clocks for the sunk form (an add, a register).
#[test]
fn test_two_loads_at_constant_offsets_stay_two_loads_where_the_target_keeps_displacements() {
    let text = "define i16 @f(ptr %p, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = getelementptr inbounds nuw i8, ptr %p, i16 2
  %x = load i16, ptr %a
  br label %e

b2:
  %b = getelementptr inbounds nuw i8, ptr %p, i16 4
  %y = load i16, ptr %b
  br label %e

e:
  %r = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  ret i16 %r
}
";
    let pin = llrm_x86::tti::X86::of_cpu("i486").unwrap();
    let keep = pin
        .clone()
        .with_tuning(llrm_mir::target::Tuning { keep_constant_displacements: true, ..llrm_mir::target::Tuning::PIN });
    let loads = |text: &str| text.matches("= load i16").count();
    assert_eq!(loads(&sunk_on(&pin, text)), 1, "premise: the pin sinks the load");
    assert_eq!(loads(&sunk_on(&keep, text)), 2, "the displacements were turned into a phi");
}

/// The same two shapes with stores (`s.a = x` / `s.b = x` behind a branch) and
/// three arms: the constant offsets stay displacements, the pin's sink makes
/// one store at a phi of the offsets.
#[test]
fn test_stores_at_constant_offsets_stay_stores_where_the_target_keeps_displacements() {
    let text = "define void @f(ptr %p, i16 %v, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = getelementptr inbounds nuw i8, ptr %p, i16 2
  store i16 %v, ptr %a
  br label %e

b2:
  %b = getelementptr inbounds nuw i8, ptr %p, i16 6
  store i16 %v, ptr %b
  br label %e

e:
  ret void
}
";
    let pin = llrm_x86::tti::X86::of_cpu("i486").unwrap();
    let keep = pin
        .clone()
        .with_tuning(llrm_mir::target::Tuning { keep_constant_displacements: true, ..llrm_mir::target::Tuning::PIN });
    let stores = |text: &str| text.matches("store i16").count();
    assert_eq!(stores(&sunk_on(&pin, text)), 1, "premise: the pin sinks the store");
    assert_eq!(stores(&sunk_on(&keep, text)), 2, "the displacements were turned into a phi");
}

/// A different base in each arm is not a constant offset: it is still sunk
/// (a phi of pointers), so the lever only holds displacements.
#[test]
fn test_loads_through_different_bases_are_still_sunk_where_the_target_keeps_displacements() {
    let text = "define i16 @f(ptr %p, ptr %q, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = getelementptr inbounds nuw i8, ptr %p, i16 2
  %x = load i16, ptr %a
  br label %e

b2:
  %b = getelementptr inbounds nuw i8, ptr %q, i16 2
  %y = load i16, ptr %b
  br label %e

e:
  %r = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  ret i16 %r
}
";
    let keep = llrm_x86::tti::X86::of_cpu("i486")
        .unwrap()
        .with_tuning(llrm_mir::target::Tuning { keep_constant_displacements: true, ..llrm_mir::target::Tuning::PIN });
    assert_eq!(sunk_on(&keep, text).matches("= load i16").count(), 1);
}

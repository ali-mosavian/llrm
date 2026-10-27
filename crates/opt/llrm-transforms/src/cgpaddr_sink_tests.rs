//! A phi of one address computation, read as an address, is that computation
//! in the join; each case's fixture is the pass's input.

use super::CgpAddr as AddressSink;
use crate::testing::{managed, parsed, results};

const HEAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-f80:16-n8:16:32\"\n\n@g = global [64 x i16] zeroinitializer\n\n";

/// `text` through the pass, answering as before on `inputs`: the text after.
fn sunk(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let original = parsed(&format!("{HEAD}{text}"));
    let mut module = original.clone();
    let after = managed(&mut module, AddressSink);
    assert_eq!(results(&module, inputs), results(&original, inputs), "{after}");
    after
}

/// `a(i) = ...` under an `IF` computes its address in each arm; the join's
/// store read a phi of three equal addresses, held in a register, where one
/// addressing form would do: RGBLIGHTS' loop stored through
/// `mov si,bx; add si,offset FSIN4%; add si,1640` (#386).
#[test]
fn test_a_phi_of_one_address_in_every_arm_is_that_address_in_the_join() {
    let text = "define void @f(i16 %i, i16 %c, i16 %d) {
b0:
  %low = icmp slt i16 %c, 0
  br i1 %low, label %b1, label %b2

b1:
  %k1 = sub i16 %i, -3
  %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
  store i16 0, ptr %p1
  br label %b5

b2:
  %mid = icmp sgt i16 %c, %d
  br i1 %mid, label %b3, label %b4

b3:
  %k3 = sub i16 %i, -3
  %p3 = getelementptr inbounds i16, ptr @g, i16 %k3
  store i16 1, ptr %p3
  br label %b5

b4:
  %k4 = sub i16 %i, -3
  %p4 = getelementptr inbounds i16, ptr @g, i16 %k4
  store i16 2, ptr %p4
  br label %b5

b5:
  %p = phi ptr [ %p1, %b1 ], [ %p3, %b3 ], [ %p4, %b4 ]
  store i16 %c, ptr %p
  ret void
}
";
    let after = sunk(text, &[&[1, -1, 2], &[4, 5, 2], &[6, 1, 9]]);
    assert!(!after.contains("phi"), "{after}");
}

/// A phi whose value is more than an address, here also added to a number,
/// stays: gvn made it so that the arms' values are not computed twice.
#[test]
fn test_a_phi_read_as_a_value_stays() {
    let text = "define i16 @f(i16 %i, i16 %c) {
b0:
  %low = icmp slt i16 %c, 0
  br i1 %low, label %b1, label %b2

b1:
  %p1 = getelementptr inbounds i16, ptr @g, i16 %i
  br label %b3

b2:
  %p2 = getelementptr inbounds i16, ptr @g, i16 %i
  br label %b3

b3:
  %p = phi ptr [ %p1, %b1 ], [ %p2, %b2 ]
  %n = ptrtoint ptr %p to i16
  ret i16 %n
}
";
    assert!(sunk(text, &[&[1, 1], &[2, -1]]).contains("phi"));
}

/// Arms that compute different addresses stay a phi.
#[test]
fn test_a_phi_of_different_addresses_stays() {
    let text = "define void @f(i16 %i, i16 %c) {
b0:
  %low = icmp slt i16 %c, 0
  br i1 %low, label %b1, label %b2

b1:
  %p1 = getelementptr inbounds i16, ptr @g, i16 %i
  br label %b3

b2:
  %j = add i16 %i, 1
  %p2 = getelementptr inbounds i16, ptr @g, i16 %j
  br label %b3

b3:
  %p = phi ptr [ %p1, %b1 ], [ %p2, %b2 ]
  store i16 %c, ptr %p
  ret void
}
";
    assert!(sunk(text, &[&[1, 1], &[2, -1]]).contains("phi"));
}

/// The arms' copies the phi alone read are dead once it is gone: left, each
/// arm built the address again for nothing (`mov cx,bx; add cx,offset FSIN4%`
/// before every store in RGBLIGHTS).
#[test]
fn test_the_arms_copies_of_the_address_go() {
    let text = "define void @f(i16 %i, i16 %c) {
b0:
  %low = icmp slt i16 %c, 0
  br i1 %low, label %b1, label %b2

b1:
  %k1 = sub i16 %i, -3
  %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
  br label %b3

b2:
  %k2 = sub i16 %i, -3
  %p2 = getelementptr inbounds i16, ptr @g, i16 %k2
  br label %b3

b3:
  %p = phi ptr [ %p1, %b1 ], [ %p2, %b2 ]
  store i16 %c, ptr %p
  ret void
}
";
    let after = sunk(text, &[&[1, 1], &[2, -1]]);
    assert_eq!((after.matches("getelementptr").count(), after.matches("sub i16").count()), (1, 1), "{after}");
}

/// A test GVN'd with its copy ahead of a loop was one `i1` that the branch at
/// the loop's bottom read, held across the whole body (hanoi with its header
/// copied: `sete cl`, a store of the byte, `cmp cl, 0`, `jne` at each of eight
/// levels). It is made again in the block that reads it, as CodeGenPrepare's
/// `sinkCmpExpression` does, and the original stays for the block that made it.
#[test]
fn test_a_comparison_a_branch_in_another_block_reads_is_made_again_there() {
    let text = "define i16 @f(i16 %n) {
b0:
  %c = icmp eq i16 %n, 0
  br i1 %c, label %b3, label %b1

b1:
  %m = mul i16 %n, 3
  br label %b2

b2:
  br i1 %c, label %b3, label %b4

b3:
  ret i16 %n

b4:
  ret i16 %m
}
";
    let after = sunk(text, &[&[0], &[1], &[5]]);
    assert_eq!(after.matches("icmp eq").count(), 2, "{after}");
    let b2 = after.split("b2:").nth(1).expect("a block b2");
    assert!(b2.contains("icmp eq") && !b2.contains("br i1 %c"), "{after}");
}

/// x_hash's `while (keys[slot] != -1 && keys[slot] != k)`: the loop's exit
/// block read the loop's address through a closing phi whose every arm is that
/// address. Isel cannot fold a phi into an addressing mode, so the load through
/// it was four instructions to form the address in registers where
/// `_keys[edx*4]` would do (+12.6% clocks at -O1, once numbering made the loads
/// one address). The phi is the value every arm carries.
#[test]
fn test_a_phi_whose_arms_are_one_address_is_that_address() {
    let text = "define i16 @f(i16 %i, i16 %c) {
b0:
  %p = getelementptr inbounds i16, ptr @g, i16 %i
  %x = load i16, ptr %p
  %t = icmp slt i16 %x, %c
  br i1 %t, label %b1, label %b2

b1:
  %u = icmp sgt i16 %x, 7
  br i1 %u, label %b2, label %b3

b2:
  %q = phi ptr [ %p, %b0 ], [ %p, %b1 ]
  %y = load i16, ptr %q
  ret i16 %y

b3:
  ret i16 0
}
";
    let after = sunk(text, &[&[1, 5], &[1, -5], &[2, 9]]);
    assert!(!after.contains("phi") && after.contains("load i16, ptr %p\n  ret i16 %y"), "{after}");
}

/// Asks for each load's and store's reference, and changes nothing.
struct Warm;

impl llrm_mir::passes::FunctionPass for Warm {
    fn name(&self) -> &'static str {
        "warm"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        let _ = analyses.get::<llrm_analysis::manager::Annotated>(unit.context, unit.layout, unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// `addresssink left annotated held but stale` on 47 objects (bintree -O2): the
/// pass said it kept every load's reference while it replaced the pointer a
/// load reads, so the next pass was handed the reference through the phi that
/// was gone.
#[test]
fn test_replacing_a_phi_leaves_no_stale_reference_held() {
    let text = format!(
        "{HEAD}define i16 @f(i16 %i, i16 %c) {{
b0:
  %p = getelementptr inbounds i16, ptr @g, i16 %i
  %x = load i16, ptr %p
  %t = icmp slt i16 %x, %c
  br i1 %t, label %b1, label %b2

b1:
  %u = icmp sgt i16 %x, 7
  br i1 %u, label %b2, label %b3

b2:
  %q = phi ptr [ %p, %b0 ], [ %p, %b1 ]
  %y = load i16, ptr %q
  ret i16 %y

b3:
  ret i16 0
}}
"
    );
    let mut module = parsed(&text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.verify_invalidation = true;
    manager.add(Warm);
    manager.add(AddressSink);
    manager.run_module(&mut module, std::rc::Rc::new(crate::testing::Tuned::default())).unwrap();
    assert!(!crate::testing::printed(&module).contains("phi"));
}

/// Machine that tests `x & mask` against zero in one instruction.
fn testing() -> crate::testing::Tuned {
    crate::testing::Tuned { folds_masks: true, ..Default::default() }
}

/// `text` through the pass on `machine`, answering as before on `inputs`.
fn sunk_on(
    text: &str,
    inputs: &[&[i128]],
    machine: crate::testing::Tuned,
) -> String {
    let original = parsed(&format!("{HEAD}{text}"));
    let mut module = original.clone();
    let after = crate::testing::managed_on(&mut module, AddressSink, machine);
    assert_eq!(results(&module, inputs), results(&original, inputs), "{after}");
    after
}

/// Three branches on one test of a bit, as numbering leaves repeated tests.
const ONE_BIT_TESTED_THREE_TIMES: &str = "define i32 @f(i32 %x, i32 %a) {
b0:
  %m = and i32 %x, 1
  %c = icmp ne i32 %m, 0
  br i1 %c, label %b1, label %b2

b1:
  %p = add i32 %a, 3
  br label %b3

b2:
  %q = mul i32 %a, 5
  br label %b3

b3:
  %v = phi i32 [ %p, %b1 ], [ %q, %b2 ]
  br i1 %c, label %b4, label %b5

b4:
  %r = add i32 %v, 7
  br label %b6

b5:
  %s = mul i32 %v, 9
  br label %b6

b6:
  %w = phi i32 [ %r, %b4 ], [ %s, %b5 ]
  br i1 %c, label %b7, label %b8

b7:
  ret i32 %w

b8:
  %t = sub i32 %w, 1
  ret i32 %t
}
";

/// 800 diamonds testing 31 bits held 31 `x & mask` values across up to 30 of
/// them, 419 stack operands and 79 allocator splits (2.2 G to compile) where
/// a `test` at each branch has none: each block that branches on the bit
/// tests it itself.
#[test]
fn test_a_bit_test_the_branches_of_later_blocks_read_is_made_in_each() {
    let after = sunk_on(ONE_BIT_TESTED_THREE_TIMES, &[&[1, 2], &[0, 2], &[3, 5]], testing());
    assert_eq!(after.matches("and i32").count(), 3, "{after}");
    for name in ["b3", "b6"] {
        let text = after.split(&format!("{name}:")).nth(1).expect("a block").split("\n\n").next().unwrap();
        assert!(text.contains("and i32 %x, 1"), "{name} tests the bit itself:\n{after}");
    }
}

/// A target with no one instruction for it keeps the one value.
#[test]
fn test_a_target_without_a_bit_test_keeps_the_one_and() {
    let after = sunk_on(ONE_BIT_TESTED_THREE_TIMES, &[&[1, 2], &[0, 2]], Default::default());
    assert_eq!(after.matches("and i32").count(), 1, "{after}");
}

/// Two values read nowhere else would both be held to the compare: the copy
/// costs two registers where the `and` held one.
#[test]
fn test_an_and_of_two_values_read_nowhere_else_is_not_made_again() {
    let text = ONE_BIT_TESTED_THREE_TIMES
        .replace("define i32 @f(i32 %x, i32 %a)", "define i32 @f(i32 %p0, i32 %a)")
        .replace("  %m = and i32 %x, 1", "  %x = add i32 %p0, 1\n  %y = add i32 %p0, 2\n  %m = and i32 %x, %y");
    let after = sunk_on(&text, &[&[1, 2], &[2, 2]], testing());
    assert_eq!(after.matches("and i32").count(), 1, "{after}");
}

/// The bit tests (`%m..` and `%c..`) a block reads that another block
/// defined.
fn read_across_blocks(text: &str) -> usize {
    let mut across = 0;
    for block in text.split("\n\n") {
        let defined: Vec<&str> = block.lines().filter_map(|line| line.trim().split(" = ").next()).collect();
        for line in block.lines() {
            let reads = line.split(" = ").last().unwrap_or("");
            for word in reads
                .split(|c: char| !(c.is_alphanumeric() || c == '%'))
                .filter(|w| w.starts_with("%m") || w.starts_with("%c"))
            {
                across += usize::from(!defined.contains(&word));
            }
        }
    }
    across
}

/// `f` branches twice on each of 31 bits of `x`, as numbering leaves it.
#[test]
fn test_31_bit_tests_held_across_the_chain_are_none_after() {
    let mut text = String::from("define i32 @f(i32 %x, i32 %a) {\nb0:\n  br label %t0\n\n");
    let mut n = 0;
    for round in 0..2 {
        for bit in 0..31 {
            let name = format!("m{bit}");
            if round == 0 {
                text +=
                    &format!("t{n}:\n  %{name} = and i32 %x, {}\n  %c{bit} = icmp ne i32 %{name}, 0\n", 1u32 << bit);
            } else {
                text += &format!("t{n}:\n");
            }
            text += &format!(
                "  br i1 %c{bit}, label %y{n}, label %z{n}\n\ny{n}:\n  br label %t{}\n\nz{n}:\n  br label %t{}\n\n",
                n + 1,
                n + 1
            );
            n += 1;
        }
    }
    text += &format!("t{n}:\n  ret i32 %a\n}}\n");
    let before = read_across_blocks(&text);
    let after = sunk_on(&text, &[&[5, 1], &[0, 1]], testing());
    assert!(before >= 31, "premise: the bit tests are read across blocks, {before}");
    assert_eq!(read_across_blocks(&after), 0, "{after}");
}

/// `sinkCmpExpression` made the compare of B$SCMP's flags again in the block
/// of its select, apart from the call: isel refused the function ("a call's
/// flags read apart from its compare with zero", `string_compare_shapes.bas`
/// at -O2). A compare of a flags result stays by its call.
#[test]
fn test_a_compare_of_a_flags_result_is_not_made_again_in_its_users_block() {
    let text = "declare i16 @llrm.qb.B$SCMP(ptr, ptr) flagsresult

define i16 @f(ptr %a, ptr %b, i1 %p) {
b0:
  %s = call i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  %c = icmp slt i16 %s, 0
  br i1 %p, label %b1, label %b2

b1:
  %r = select i1 %c, i16 2, i16 3
  ret i16 %r

b2:
  ret i16 0
}
";
    let mut module = parsed(&format!("{HEAD}{text}"));
    let after = managed(&mut module, AddressSink);
    let b1 = after.split("b1:").nth(1).expect("b1");
    assert!(!b1.contains("icmp"), "{after}");
}

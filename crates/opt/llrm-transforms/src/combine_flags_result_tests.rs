//! A runtime routine that returns flags (`B$SCMP`, `runtime.toml`) may be
//! read only by compares: isel refuses it otherwise ("flags read other than by
//! a comparison"). Every fold of the whole combine leaves a user of such a
//! call a compare, for each predicate, constant and consumer below. Before the
//! guard, `icmp slt %s, 0` (= `sle %s, -1`) under a sext, a zext or a select
//! of constants became `ashr`/`lshr %s, 15` (combine_casts.rs `zext`/`sext`
//! of a sign test): 16 of the 360 cases.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::opcode::Opcode;

use crate::combine::combine_with;
use crate::testing::{parsed, printed};

const CONSUMERS: [&str; 12] = [
    "  %r = sext i1 %c to i16\n  ret i16 %r\n",
    "  %r = zext i1 %c to i16\n  ret i16 %r\n",
    "  %r = sext i1 %c to i32\n  %t = trunc i32 %r to i16\n  ret i16 %t\n",
    "  %r = zext i1 %c to i32\n  %t = trunc i32 %r to i16\n  ret i16 %t\n",
    "  %r = select i1 %c, i16 -1, i16 0\n  ret i16 %r\n",
    "  %r = select i1 %c, i16 1, i16 0\n  ret i16 %r\n",
    "  %r = select i1 %c, i16 0, i16 -1\n  ret i16 %r\n",
    "  %r = select i1 %c, i16 5, i16 7\n  ret i16 %r\n",
    "  %n = xor i1 %c, true\n  %r = zext i1 %n to i16\n  ret i16 %r\n",
    "  %d = icmp sgt i16 %s, 3\n  %e = and i1 %c, %d\n  %r = zext i1 %e to i16\n  ret i16 %r\n",
    "  %d = icmp slt i16 %s, -3\n  %e = or i1 %c, %d\n  %r = zext i1 %e to i16\n  ret i16 %r\n",
    "  %d = icmp ne i16 %s, 0\n  %e = and i1 %c, %d\n  %r = sext i1 %e to i16\n  ret i16 %r\n",
];

#[test]
fn test_combine_gives_a_flags_result_call_no_user_but_compares() {
    let mut bad = Vec::new();
    for predicate in ["slt", "sle", "sgt", "sge", "eq", "ne", "ult", "ule", "ugt", "uge"] {
        for constant in ["-1", "0", "1"] {
            for (n, tail) in CONSUMERS.iter().enumerate() {
                let text = format!(
                    "declare i16 @llrm.qb.B$SCMP(ptr, ptr) flagsresult
define i16 @f(ptr %a, ptr %b) {{
b0:
  %s = call i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  %c = icmp {predicate} i16 %s, {constant}
{tail}}}
"
                );
                let mut module = parsed(&text);
                let globals = module.globals.clone();
                let metadata = module.metadata.clone();
                let (context, function) = module.function_mut("f").expect("@f");
                combine_with(context, &globals, &metadata, &DataLayout::default(), function, crate::combine::visit);
                let call = function
                    .walk()
                    .map(|(_, one)| one)
                    .find(|&one| matches!(function.instruction(one).opcode, Opcode::Call(_)))
                    .expect("the call");
                let value = function.instruction(call).result.expect("a value");
                if !function
                    .users(value)
                    .iter()
                    .all(|one| matches!(function.instruction(one.user).opcode, Opcode::ICmp(_)))
                {
                    bad.push(format!("{predicate} {constant} consumer {n}:\n{}", printed(&module)));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} cases give the flags a user that is no compare:\n{}", bad.len(), bad.join("\n"));
}

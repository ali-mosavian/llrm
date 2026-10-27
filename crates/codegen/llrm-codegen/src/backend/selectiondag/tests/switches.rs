use llrm_mir::context::ConstantKind;
use llrm_mir::module::Operand;
use llrm_mir::opcode::Opcode;

use super::super::switchcg::{SwitchTarget, describe, plan};

/// Each switch of `switches.ll` makes the clusters the pin prints in
/// `Case clusters:` (jump tables `JT:`, bit tests `BT:`, the rest as
/// ranges). Without the planner the line is empty.
#[test]
fn switch_clusters_match_the_pins() {
    let text: String = include_str!("../golden/switches.ll")
        .lines()
        .filter(|line| !line.starts_with("target triple"))
        .map(|line| format!("{line}\n"))
        .collect();
    let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
    let mut ours = String::new();
    for (_, global, function) in module.functions() {
        for &block in function.layout() {
            let Some(terminator) = function.terminator(block) else { continue };
            let instruction = function.instruction(terminator);
            if instruction.opcode != Opcode::Switch {
                continue;
            }
            let Operand::Value(cond) = instruction.operands[0] else { panic!("a value condition") };
            let width = match module.context.types.get(function.value(cond).ty) {
                llrm_mir::types::Type::Int(bits) => *bits,
                other => panic!("{other:?}"),
            };
            let Operand::Block(default) = instruction.operands[1] else { panic!("a default") };
            let cases: Vec<(i128, u32)> = instruction.operands[2..]
                .chunks(2)
                .map(|pair| {
                    let (Operand::Constant(value), Operand::Block(dest)) = (pair[0], pair[1]) else { panic!("a case") };
                    let ConstantKind::Int(bits) = module.context.get(value).kind else { panic!("an integer case") };
                    let shift = 128 - width;
                    ((((bits << shift) as i128) >> shift), dest.0)
                })
                .collect();
            let planned = plan(&cases, default.0, &SwitchTarget::x86(32));
            let mut line = describe(&planned.clusters);
            if let Some((peeled, _)) = planned.peeled {
                line = format!("{}{line}", describe(&[peeled]));
            }
            ours.push_str(&format!("{}: {line}\n", global.name.as_deref().unwrap()));
        }
    }
    assert_eq!(ours, include_str!("../golden/switches.pin.txt"));
}

#!/usr/bin/env python3
"""One row per machine pass, ours against the pin's counterpart, from the same two
inclusive profiles as ledger4.py (LEDGER_OURS, LEDGER_CLANG): markdown on stdout.

passtable.py takes no arguments."""
import os
import re
import sys
from collections import Counter

import nested

# (our pass, the pin's counterpart regexes or None, proposed owner)
PASSES = [
    ("peephole::Peephole", [r"PeepholeOptimizerLegacy::runOnMachineFunction\(", r"X86FixupBWInstLegacy::runOnMachineFunction\("], "w-1"),
    ("peepholeopt::PeepholeOptimizer", None, "w-1"),
    ("earlytaildup::EarlyTailDuplicate", [r"EarlyTailDuplicateLegacy::runOnMachineFunction\("], "w-1"),
    ("machinesched::MachineScheduler", [r"MachineSchedulerLegacy::runOnMachineFunction\("], "w-2"),
    ("schedule::Scheduler", None, "w-2"),
    ("machinesink::MachineSink", [r"MachineSinkingLegacy::runOnMachineFunction\(", r"PostRAMachineSinkingLegacy::runOnMachineFunction\("], "w-2"),
    ("floatalloc::FloatAlloc", [r"X86FPStackifierLegacy::runOnMachineFunction\("], "w-2"),
    ("floatassign::FloatPrep", None, "w-2"),
    ("floatfold::FloatFold", None, "w-2"),
    ("jumps::ControlFlow", [r"BranchFolderLegacy::runOnMachineFunction\(", r"MachineBlockPlacementLegacy::runOnMachineFunction\("], "w-3"),
    ("machinelicm::MachineLicm", [r"MachineLICMBase::runOnMachineFunction\("], "w-3"),
    ("machinecp::CopyPropagation", [r"MachineCopyPropagationLegacy::runOnMachineFunction\("], "w-3"),
    ("precse::MachineCse", [r"MachineCSELegacy::runOnMachineFunction\("], "w-2 (unassigned)"),
    ("latecleanup::LateCleanup", [r"MachineLateInstrsCleanupLegacy::runOnMachineFunction\("], "funcattrs-port"),
    ("combiner::MachineCombiner", [r"MachineCombiner::runOnMachineFunction\("], "w-8"),
    ("liverangeshrink::LiveRangeShrink", [r"LiveRangeShrink::runOnMachineFunction\("], "w-2 (unassigned)"),
    ("loopslots::LoopSlots", None, "w-3 (unassigned)"),
    ("phiconst::SharedConstants", None, "w-3 (unassigned)"),
    ("farcall::FarIndirectCalls", None, "(m16 only)"),
    ("fixupsetcc::FixupSetCc", [r"X86FixupSetCCPass::runOnMachineFunction\("], "w-2 (unassigned)"),
    ("fixupbw::FixupBw", None, "w-1 (pin: with Peephole above)"),
    ("pushfold::LoadFold", None, "w-1"),
    ("cmpsub::CmpAsSub", None, "w-1"),
    ("leaopt::OptimizeLeas", [r"OptimizeLEAPass::runOnMachineFunction\("], "w-2 (lea fold)"),
    ("fixupleas::FixupLeas", [r"FixupLEAPass::runOnMachineFunction\("], "w-2 (lea fold)"),
    ("flagscopy::FlagsCopy", [r"X86FlagsCopyLoweringPass::runOnMachineFunction\("], "w-2 (unassigned)"),
    ("callframe::CallFrameOpt", [r"X86CallFrameOptimization::runOnMachineFunction\("], "w-2 (unassigned)"),
]


# Passes of the pin's bucket that have no pass of ours.
PIN_ONLY = [
    ("PEI (PrologEpilogInserter)", [r"PEILegacy::runOnMachineFunction\("], "(unassigned)"),
    ("ShrinkWrap", [r"ShrinkWrapLegacy::runOnMachineFunction\("], "(unassigned)"),
    ("ReachingDefAnalysis", [r"ReachingDefInfoWrapperPass::runOnMachineFunction\("], "(unassigned)"),
    ("ExecutionDomainFix", [r"ExecutionDomainFix::runOnMachineFunction\("], "(unassigned)"),
    ("DeadMachineInstructionElim", [r"DeadMachineInstructionElim::runOnMachineFunction\("], "(unassigned)"),
]


def parse(path, pin):
    costs = Counter()
    for line in open(path):
        m = re.match(r"\s*([\d,]+) \(\s*[\d.]+%\)\s+(.*)$", line)
        if m and not m.group(2).lstrip().startswith("=>"):
            costs[nested.clean(m.group(2), pin)] += int(m.group(1).replace(",", ""))
    return costs


def sum_of(costs, patterns):
    seen = {}
    for pattern in patterns:
        for name, ir in costs.items():
            if re.search(pattern, name):
                seen[name] = ir
    return sum(seen.values())


def main():
    ours, pin = parse(os.environ["LEDGER_OURS"], False), parse(os.environ["LEDGER_CLANG"], True)
    print("| our pass | ours M | pin M | ratio | proposed owner |\n|---|--:|--:|--:|---|")
    total_o = total_p = 0.0
    for name, counterparts, owner in PASSES:
        o = sum_of(ours, [r"<llrm_codegen::backend::" + re.escape(name) + r" as llrm_codegen::model::passes::LIRTransform>::transform_raising$"]) / 1e6
        p = sum_of(pin, counterparts) / 1e6 if counterparts else None
        total_o += o
        total_p += p or 0
        ratio = f"{o / p:.1f}" if p else "-"
        print(f"| {name} | {o:.1f} | {'-' if p is None else f'{p:.1f}'} | {ratio} | {owner} |")
    for name, counterparts, owner in PIN_ONLY:
        p = sum_of(pin, counterparts) / 1e6
        total_p += p
        print(f"| (none of ours) {name} | - | {p:.1f} | - | {owner} |")
    print(f"| total | {total_o:.1f} | {total_p:.1f} | {total_o / total_p:.2f} | |")


if __name__ == "__main__":
    if len(sys.argv) > 1:
        sys.exit("passtable: takes no arguments; set LEDGER_OURS and LEDGER_CLANG")
    main()

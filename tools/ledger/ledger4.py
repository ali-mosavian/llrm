import os, re, collections, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import nested
def parse(path, pin=False):
    d = collections.Counter()
    for l in open(path):
        m = re.match(r"\s*([\d,]+) \(\s*[\d.]+%\)\s+(.*)$", l)
        if not m: continue
        if m.group(2).lstrip().startswith("=>"): continue
        ir = int(m.group(1).replace(",", ""))
        sym = nested.clean(m.group(2), pin)
        d[sym] += ir
    return d
def settings(environ, argv):
    """The inputs, all named: no default file is read (a stale ours.incl beside the
    script was, silently, with positional arguments ignored)."""
    if argv[1:] not in ([], ["--detail"]):
        raise SystemExit(f"ledger4: positional arguments are not supported ({argv[1:]}); set LEDGER_OURS, LEDGER_CLANG, LEDGER_OT, LEDGER_PT; --detail lists the functions")
    missing = [name for name in ("LEDGER_OURS", "LEDGER_CLANG", "LEDGER_OT", "LEDGER_PT") if not environ.get(name)]
    if missing:
        raise SystemExit(f"ledger4: {', '.join(missing)} not set (no default file is read)")
    return environ["LEDGER_OURS"], environ["LEDGER_CLANG"], int(environ["LEDGER_OT"]), int(environ["LEDGER_PT"]), argv[1:] == ["--detail"]


OURS, CLANG, OT, PT, DETAIL = settings(os.environ, sys.argv)
ours = parse(OURS); clang = parse(CLANG, pin=True)
clang_hts = [v for k, v in clang.items() if k.startswith("clang::BackendConsumer::HandleTranslationUnit(")][0]
# bucket -> (regex on our symbol, regex on pin symbol)
O = lambda name: r"<llrm_transforms::" + name + r" as llrm_mir::(passes::(Function|Module)Pass|program::ProgramPass)>::run$"
OB = lambda name: r"<llrm_core::backend::" + name + r" as llrm_core::model::passes::LIRTransform>::transform_raising$"
B = collections.OrderedDict()
B["SimplifyCFG, JumpThreading, CVP"] = ([O(n) for n in ("simplifycfg::SimplifyCfg", "decide::Decide", "jumpthreading::JumpThreading", "cvp::Cvp", "dfajumpthreading::DfaJumpThreading")],
    [r"^llvm::SimplifyCFGPass::run\(", r"^llvm::JumpThreadingPass::run\(", r"^llvm::CorrelatedValuePropagationPass::run\(", r"^llvm::ConstraintEliminationPass::run\("])
B["InstCombine, InstSimplify, Reassociate"] = ([O(n) for n in ("combine_pass::Combine", "instsimplify::InstSimplify", "reassociate::Reassociate", "trunc_inst_combine::TruncInstCombine")],
    [r"^llvm::InstCombinePass::run\(", r"^llvm::InstSimplifyPass::run\(", r"^llvm::ReassociatePass::run\(", r"^llvm::AggressiveInstCombinePass::run\("])
B["loop passes (LICM, IndVars, rotate, unroll, idiom, deletion, LCSSA, simplify, LSR front)"] = ([O(n) for n in ("licm::Licm", "indvars::IndVars", "looprotate::LoopRotate", "loopunrollpass::LoopFullUnroll", "loopidioms::LoopIdioms", "loopidiom::pass::LoopIdiom", "loopidioms::Merge", "loopdeletion::LoopDeletion", "lcssa::LoopClosedSSA", "lcssa::LoopOpened", "loopsimplify::LoopSimplify", "looploadelim::LoopLoadElim", "simpleloopunswitch::SimpleLoopUnswitch", "loopinterchange::Interchange", "floatloop::FloatLoop", "lsr_pass::LoopStrengthReduce", "rotate::Rotate")],
    [r"^llvm::LICMPass::run\(", r"^llvm::IndVarSimplifyPass::run\(", r"^llvm::LoopRotatePass::run\(", r"^llvm::LoopUnrollPass::run\(", r"^llvm::LoopFullUnrollPass::run\(", r"^llvm::LoopIdiomRecognizePass::run\(", r"^llvm::LoopDeletionPass::run\(", r"^llvm::LCSSAPass::run\(", r"^llvm::LoopSimplifyPass::run\(", r"^llvm::LoopLoadEliminationPass::run\(", r"^llvm::LoopInstSimplifyPass::run\(", r"^llvm::LoopSimplifyCFGPass::run\(", r"^llvm::SimpleLoopUnswitchPass::run\(", r"^llvm::LoopStrengthReducePass::run\(", r"LoopStrengthReduce::runOnLoop"])
B["scalar: GVN, EarlyCSE, SROA, DSE, MemCpyOpt, SCCP, DCE, TRE"] = ([O(n) for n in ("gvn::Gvn", "earlycse::EarlyCse", "sroa::Sroa", "mem2reg::Mem2Reg", "deadstore::DeadStoreElimination", "memcpyopt::MemCpyOpt", "sccp::Sccp", "bdce::Bdce", "adce::Adce", "dead::Dead", "tailcallelim::TailCallElim", "float2int::Float2Int")],
    [r"^llvm::GVNPass::run\(", r"^llvm::EarlyCSEPass::run\(", r"^llvm::SROAPass::run\(", r"^llvm::DSEPass::run\(", r"^llvm::MemCpyOptPass::run\(", r"^llvm::SCCPPass::run\(", r"^llvm::BDCEPass::run\(", r"^llvm::ADCEPass::run\(", r"^llvm::TailCallElimPass::run\(", r"^llvm::Float2IntPass::run\(", r"^llvm::InferAlignmentPass::run\(", r"^llvm::SpeculativeExecutionPass::run\("])
B["llrm-only MIR passes (fold, algebraic, ports, window, gepoffset, addresssink, fixednarrow, ...)"] = ([O(n) for n in ("fold::Fold", "algebraic::Algebraic", "algebraic::Differences", "ports::Ports", "window::Window", "gepoffset::GepOffset", "addresssink::AddressSink", "fixednarrow::FixedNarrow", "inferspace::InferAddressSpaces", "switchlower::SwitchLower")], [])
B["module/IPO passes (interprocedural, ipo, globalopt, dce, stamp, homes, spares)"] = ([O(n) for n in ("interprocedural::Interprocedural", "walk::ModuleIpo", "interprocedural::Stamp", "globalopt::GlobalOpt", "globalopt::WriteOnly", "globaldce::GlobalDce", "homes::Homes", "spares::Spares", "calleepop::CalleePop")],
    [r"^llvm::IPSCCPPass::run\(", r"^llvm::GlobalOptPass::run\(", r"^llvm::GlobalDCEPass::run\(", r"^llvm::CalledValuePropagationPass::run\(", r"^llvm::PostOrderFunctionAttrsPass::run\(", r"^llvm::InlinerPass::run\(", r"^llvm::InferFunctionAttrsPass::run\(", r"^llvm::RecomputeGlobalsAAPass::run\(", r"^llvm::DeadArgumentEliminationPass::run\(", r"^llvm::ArgumentPromotionPass::run\("])
B["isel"] = ([r"llrm_core::backend::isel::selected_with$"], [r"^\(anonymous namespace\)::X86DAGToDAGISel::runOnMachineFunction\("])
B["regalloc (RA, coalesce, phi-elim, two-address, parcopy)"] = ([OB(n) for n in ("allocate::RegAlloc", "coalesce::Coalescer", "phielim::PhiElimination", "twoaddr::TwoAddress", "parcopy::ParallelCopy")],
    [r"^\(anonymous namespace\)::RAGreedyLegacy::runOnMachineFunction\(", r"^\(anonymous namespace\)::RegisterCoalescerLegacy::runOnMachineFunction\(", r"^llvm::LiveIntervalsWrapperPass::runOnMachineFunction\(", r"^\(anonymous namespace\)::PHIElimination::runOnMachineFunction\(", r"^\(anonymous namespace\)::TwoAddressInstructionLegacyPass::runOnMachineFunction\(", r"^llvm::LiveVariablesWrapperPass::runOnMachineFunction\(", r"VirtRegRewriterLegacy::runOnMachineFunction\(", r"StackSlotColoringLegacy::runOnMachineFunction\("])
B["machine clean-up + FP stackifier (peephole, sink, jumps, cse, float*, frame)"] = ([OB(n) for n in ("peephole::Peephole", "machinesink::MachineSink", "jumps::ControlFlow", "precse::MachineCse", "floatalloc::FloatAlloc", "floatassign::FloatPrep", "floatfold::FloatFold", "pressuresink::PressureSink", "loopslots::LoopSlots", "phiconst::SharedConstants", "schedule::Scheduler", "farcall::FarIndirectCalls")],
    [r"MachineSchedulerLegacy::runOnMachineFunction\(", r"MachineLICMBase::runOnMachineFunction\(", r"MachineSinkingLegacy::runOnMachineFunction\(", r"MachineBlockPlacementLegacy::runOnMachineFunction\(", r"MachineCopyPropagationLegacy::runOnMachineFunction\(", r"MachineCSELegacy::runOnMachineFunction\(", r"BranchFolderLegacy::runOnMachineFunction\(", r"X86FPStackifierLegacy::runOnMachineFunction\(", r"PeepholeOptimizerLegacy::runOnMachineFunction\(", r"DeadMachineInstructionElim::runOnMachineFunction\(", r"MachineLateInstrsCleanupLegacy::runOnMachineFunction\(", r"PEILegacy::runOnMachineFunction\(", r"LiveRangeShrink::runOnMachineFunction\(", r"X86FixupBWInstLegacy::runOnMachineFunction\(", r"ShrinkWrapLegacy::runOnMachineFunction\(", r"ReachingDefInfoWrapperPass::runOnMachineFunction\(", r"ExecutionDomainFix::runOnMachineFunction\(", r"PostRAMachineSinkingLegacy::runOnMachineFunction\("])
B["emit (object writer; pin: asm printer + MC encode)"] = ([r"llrm_core::backend::objbuild::written_in$"], [r"^llvm::X86AsmPrinter::runOnMachineFunction\(", r"^llvm::AsmPrinter::doFinalization\("])
FRONT = (OT - ours["llrm_core::driver::compiled"] - ours["llrm_core::backend::objbuild::written_in"] + ours["llrm_core::driver::emitted"], PT - clang_hts)
def total(table, pats, label=None):
    seen = {}
    for p in pats:
        r = re.compile(p)
        for sym, ir in table.items():
            if r.search(sym): seen[sym] = ir
    return sum(seen.values()), seen
# Instrument check: `-o /dev/null` makes llrm-c skip the object write (emit 0, front end inflated);
# run with -o on a real file. 2026-10-22.
assert total(ours, B["emit (object writer; pin: asm printer + MC encode)"][0])[0] > 0, "emit is 0: the compile did not write an object (-o /dev/null?)"
_calling = os.environ.get("LEDGER_CALLING")
_double = None
if _calling:
    _double = nested.check(nested.parse_calling(_calling), {n: set(total(ours, op)[1]) for n, (op, pp) in B.items()})
if os.environ.get("LEDGER_LISTED"):
    open(os.environ["LEDGER_LISTED"], "w").write("\n".join(sorted({s for _n, (_op, _pp) in B.items() for s in total(ours, _op)[1]})))
_pin_calling = os.environ.get("LEDGER_PIN_CALLING")
if _pin_calling:
    _double = (_double or "") + "\n" + "pin " + nested.check(
        nested.parse_calling(_pin_calling, pin=True), {n: set(total(clang, pp)[1]) for n, (op, pp) in B.items()}
    )
rows = []; so = sp = 0
for name, (op, pp) in B.items():
    o, os_ = total(ours, op); p, ps = total(clang, pp)
    rows.append((name, o / 1e6, p / 1e6)); so += o; sp += p
print(f"{'bucket':92} {'ours M':>8} {'pin M':>8} {'ratio':>6} {'excess':>8}")
for n, o, p in rows:
    print(f"{n:92} {o:8.1f} {p:8.1f} {(o/p if p else float('nan')):6.2f} {o-p:8.1f}")
print(f"{'front end (parse, translate, HIR, HIR->MIR / clang parse+sema)':92} {FRONT[0]/1e6:8.1f} {FRONT[1]/1e6:8.1f} {FRONT[0]/FRONT[1]:6.2f} {(FRONT[0]-FRONT[1])/1e6:8.1f}")
so += FRONT[0]; sp += FRONT[1]
print(f"{'sum of the buckets':92} {so/1e6:8.1f} {sp/1e6:8.1f}")
print(f"{'residual (pass-manager glue, analyses outside passes, walk own, allocation, lowering)':92} {(OT-so)/1e6:8.1f} {(PT-sp)/1e6:8.1f}")
print(_double or "double-count check: not run (no LEDGER_CALLING)")
print(f"{'total (callgrind Ir)':92} {OT/1e6:8.1f} {PT/1e6:8.1f} {OT/PT:6.2f}")
if DETAIL:
    for name, (op, pp) in B.items():
        o, os_ = total(ours, op); p, ps = total(clang, pp)
        print("\n##", name)
        for s, v in sorted(os_.items(), key=lambda kv: -kv[1])[:30]: print("  ours %8.1f %s" % (v / 1e6, re.sub(r"<llrm_\w+::(\w+(::\w+)*) as .*", r"\1", s)[:80]))
        for s, v in sorted(ps.items(), key=lambda kv: -kv[1])[:30]: print("  pin  %8.1f %s" % (v / 1e6, s[:80]))

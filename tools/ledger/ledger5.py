import re, sys, collections
# Self cost by callee, attributed to the pin pass the callee's module is the counterpart of
# (not by call site). Shared leaves (alloc, libc, llrm_mir core) stay as their own rows.
RULES = [
 (r"llrm_cc::|(?<![\w:])(clang|llvm)::|/(clang|llvm)/(lib|include|tools)/", "clang (llrm-cc front end)"),
 (r"llrm_codegen::(twoaddress|backend::twoaddr)", "TwoAddress"),
 (r"llrm_codegen::(coalescer|backend::coalesce)", "RegisterCoalescer"),
 (r"llrm_codegen::greedy|backend::allocate|backend::spiller|backend::lifetimes|backend::regalloc", "RAGreedy + SplitKit + InlineSpiller"),
 (r"llrm_codegen::liveness|backend::liveness|backend::lanes|backend::incremental", "LiveIntervals/LiveVariables"),
 (r"llrm_codegen::phielimination|backend::phielim|backend::optimizephis", "PHIElimination, OptimizePHIs"),
 (r"backend::(peephole|peep::|machinecse|precse|sharedstores|regthrash|machinedce|latecleanup|machinecp|copysink|spillforward|upperzero|exactaddress|leaopt|fixupsetcc)", "Peephole, MachineCSE, DCE, late cleanup"),
 (r"backend::(machinesink)", "MachineSink, LiveRangeShrink"),
 (r"backend::(jumps|schedule|loopslots|shrinkwrap|frame|prologue|masm)", "branch folding/placement, scheduler, PEI"),
 (r"backend::(floatalloc|floatassign|floatfold|fpstack|fpregs|fpcopyprop)", "X86FloatingPoint"),
 (r"backend::(isel|selectiondag|lower|legalize|matcher|arithmetic|division|selects|booleans)|llrm_dagisel|llrm_iselgen", "isel (SelectionDAG)"),
 (r"backend::objbuild|llrm_omf|backend::assemble", "emit (AsmPrinter, MC)"),
 (r"llrm_analysis::scev|llrm_transforms::(lsr|indvars|loop|licm|lcssa|rotate)", "loop passes, LSR, SCEV"),
 (r"llrm_transforms::(simplifycfg|decide|jumpthreading|cvp|dfajump|edges)|llrm_mir::(constraint|lazyvalue)", "SimplifyCFG, JumpThreading, CVP"),
 (r"llrm_transforms::(combine|instsimplify|reassociate|algebraic|trunc)|llrm_mir::(constantfolding|knownbits|constantrange|constfold)", "InstCombine, InstSimplify"),
 (r"llrm_transforms::(walk|interprocedural|globalopt|globaldce|inline|calleepop)|llrm_mir::(callgraph|inline)", "IPO (inliner, globalopt, ...)"),
 (r"llrm_analysis::(alias|memory|memoryssa|regions|basicaa|effects|references)", "AA, MemorySSA (analyses)"),
 (r"llrm_transforms::", "other mid-end passes"),
 (r"llrm_analysis::", "other analyses"),
 (r"llrm_mir::passes|llrm_mir::edit|llrm_mir::uselist", "pass manager, IR edits"),
 (r"llrm_mir::", "MIR core (shared: Function, Opcode, types)"),
 (r"llrm_hir|llrm_c::|llrm_frontend|llrm_lexer|frontends", "front end"),
 (r"alloc::|core::|std::|hashbrown|libc|\.so|\./elf/|sysdeps/|__rustc::|memmove|memcmp|memcpy|malloc|free|cfree", "alloc/libc/std (shared leaves)"),
]
R = [(re.compile(a), b) for a, b in RULES]
def load(path):
    out = collections.Counter(); tot = 0
    for l in open(path):
        m = re.match(r"\s*([\d,]+) \(\s*[\d.]+%\)\s+(.*)$", l)
        if not m: continue
        ir = int(m.group(1).replace(",", ""))
        sym = m.group(2)
        if "PROGRAM TOTALS" in sym: tot = ir; continue
        for r, b in R:
            if r.search(sym): out[b] += ir; break
        else: out["unattributed"] += ir
    out["(below threshold)"] += tot - sum(out.values())
    return out, tot
files = sys.argv[1:]
data = [load(f) for f in files]
names = sorted({k for d, _ in data for k in d}, key=lambda k: -data[-1][0].get(k, 0))
print(f"{'bucket (self Ir by callee, M)':56}" + "".join(f"{f.split('/')[0][3:13]:>14}" for f in files) + ("   delta" if len(files) > 1 else ""))
for k in names:
    row = [d.get(k, 0) / 1e6 for d, _ in data]
    print(f"{k:56}" + "".join(f"{v:14.1f}" for v in row) + (f"{row[-1]-row[0]:8.1f}" if len(row) > 1 else ""))
print(f"{'TOTAL':56}" + "".join(f"{t/1e6:14.1f}" for _, t in data))

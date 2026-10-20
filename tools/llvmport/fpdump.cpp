// Prints what LLVM's computeKnownFPClass proves of every floating-point scalar of a module, one line each:
//   <function> <n> all=<mask>/<sign> nan=<mask>/<sign> neg=<mask>/<sign>
// where <n> numbers the function's arguments and then its instructions with a scalar float or double result, and
// the three columns ask for fcAllFlags, fcNan and fcNegative; <mask> is the known classes in hex, <sign> 0, 1 or ?.
// usage: fpdump [--dc] FILE.ll...   --dc: the query has a DomConditionCache (the dominating branch conditions).   `tools/llvmport/fpdump.sh` builds and runs it; the Rust side is the `fpdump` example.
#include "llvm/Config/llvm-config.h"
#include "llvm/Analysis/AssumptionCache.h"
#include "llvm/Analysis/DomConditionCache.h"
#include "llvm/Analysis/SimplifyQuery.h"
#include "llvm/Analysis/ValueTracking.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/Dominators.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/KnownFPClass.h"
#include "llvm/Support/SourceMgr.h"
#include <cstdio>
#include <string>

using namespace llvm;

static std::string fp(const KnownFPClass &K) {
  char buf[32];
  std::optional<bool> S = K.getSignBit();
  snprintf(buf, sizeof buf, "%x/%c", (unsigned)K.getKnownFPClasses(), S ? (*S ? '1' : '0') : '?');
  return buf;
}

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s\n", LLVM_VERSION_STRING);
    return 0;
  }
  LLVMContext Ctx;
  bool UseDC = false;
  for (int a = 1; a < argc; ++a) {
    if (std::string(argv[a]) == "--dc") {
      UseDC = true;
      continue;
    }
    SMDiagnostic Err;
    std::unique_ptr<Module> M = parseAssemblyFile(argv[a], Err, Ctx);
    if (!M) {
      Err.print(argv[a], errs());
      return 1;
    }
    const DataLayout &DL = M->getDataLayout();
    for (Function &F : *M) {
      if (F.isDeclaration()) continue;
      AssumptionCache AC(F);
      DominatorTree DT(F);
      DomConditionCache DC;
      if (UseDC)
        for (BasicBlock &BB : F)
          if (auto *BI = dyn_cast<CondBrInst>(BB.getTerminator()))
            DC.registerBranch(BI);
      unsigned n = 0;
      auto dump = [&](Value *V, Instruction *CxtI) {
        Type *Ty = V->getType();
        if (!Ty->isFloatTy() && !Ty->isDoubleTy()) return;
        SimplifyQuery Q(DL, /*TLI*/ nullptr, &DT, &AC, CxtI, true, true, UseDC ? &DC : nullptr);
        printf("%s %u all=%s nan=%s neg=%s\n", F.getName().str().c_str(), n++,
               fp(computeKnownFPClass(V, fcAllFlags, Q)).c_str(), fp(computeKnownFPClass(V, fcNan, Q)).c_str(),
               fp(computeKnownFPClass(V, fcNegative, Q)).c_str());
      };
      for (Argument &Arg : F.args()) dump(&Arg, &*F.getEntryBlock().begin());
      for (BasicBlock &BB : F)
        for (Instruction &I : BB) dump(&I, &I);
    }
  }
  return 0;
}

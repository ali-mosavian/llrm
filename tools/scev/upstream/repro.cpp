// Reproducer: SCEVUse use-flags make getMulExpr/getAddExpr lose the rewritten
// AddRec (`Ops[i] == AddRec` in ScalarEvolution.cpp), so the search runs off Ops.
// Builds against a pinned tree (-DPINNED) or current main.
#include "llvm/Analysis/LoopInfo.h"
#include "llvm/Analysis/ScalarEvolution.h"
#include "llvm/Analysis/ScalarEvolutionExpressions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IRReader/IRReader.h"
#include "llvm/Passes/PassBuilder.h"
#include "llvm/Support/SourceMgr.h"
using namespace llvm;
#ifdef PINNED
using NW = SCEV::NoWrapFlags;
using Pair = SCEVFlags;
#else
using NW = SCEVFlags;
using Pair = SCEVFlagsPair;
#endif
int main(int, char **argv) {
  LLVMContext C; SMDiagnostic E;
  auto M = parseIRFile(argv[1], E, C);
  PassBuilder PB; FunctionAnalysisManager FAM; LoopAnalysisManager LAM; CGSCCAnalysisManager CAM; ModuleAnalysisManager MAM;
  PB.registerModuleAnalyses(MAM); PB.registerCGSCCAnalyses(CAM); PB.registerFunctionAnalyses(FAM); PB.registerLoopAnalyses(LAM);
  PB.crossRegisterProxies(LAM, FAM, CAM, MAM);
  Function &F = *M->begin();
  auto &SE = FAM.getResult<ScalarEvolutionAnalysis>(F);
  auto &LI = FAM.getResult<LoopAnalysis>(F);
  Loop *L = *LI.begin();
  const SCEV *B = SE.getUnknown(F.getArg(0)), *A = SE.getUnknown(F.getArg(1));
  SmallVector<SCEVUse, 2> Ops = {B, A};
  const SCEV *AR = SE.getAddRecExpr(Ops, L, Pair(NW::FlagNone));
  SCEVUse ARNuw(AR, NW::FlagNUW);                 // the recurrence held with a use flag
  const SCEV *X = SE.getUnknown(F.getArg(2));
  SCEVUse Div = SE.getUDivExpr(X, ARNuw);
  SCEVUse Sum = SE.getAddExpr(X, Div, ARNuw);      // crashes: Ops[i] == AddRec never true
  outs() << *Sum << "\n";
}

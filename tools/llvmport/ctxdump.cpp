// Prints what LLVM's context-derived ValueTracking functions answer on a module, one line each:
//   <f> ctx i<I> <v> kb=...     computeKnownBitsFromContext of operand <v> of instruction I (all branches in the DomConditionCache)
//   <f> imp <a> <b> <t> <r>     isImpliedCondition(a, b, t) over pairs of i1 values (r: T, F or -)
//   <f> dom i<I> <c> <r>        isImpliedByDomCondition(c, I)
//   <f> domp i<I> <c> <r>       isImpliedByDomCondition(pred, lhs, rhs, I) for the icmp c
//   <f> aff <c> <assume> <list> findValuesAffectedByCondition
//   <f> acache <v> <list>       the instructions of AssumptionCache::assumptionsFor(v)
//   <f> free <a> i<I> <r>       willNotFreeBetween
//   <f> valid <a> i<I> <dt> <r> isValidAssumeForContext, with (dt=1) or without a DominatorTree
// Values are named a<k> (argument), i<k> (k-th instruction of the function), @name or ?.
// usage: ctxdump FILE.ll...   `tools/llvmport/ctxdump.sh` runs it; examples/ctxdump.rs prints the same lines.
#include "llvm/Config/llvm-config.h"
#include "llvm/Analysis/AssumptionCache.h"
#include "llvm/Analysis/DomConditionCache.h"
#include "llvm/Analysis/SimplifyQuery.h"
#include "llvm/Analysis/ValueTracking.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/Dominators.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/IntrinsicInst.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/KnownBits.h"
#include "llvm/Support/SourceMgr.h"
#include <cstdio>
#include <map>
#include <string>
#include <vector>

using namespace llvm;

static const char *tri(std::optional<bool> r) { return r ? (*r ? "T" : "F") : "-"; }

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s\n", LLVM_VERSION_STRING);
    return 0;
  }
  if (argc < 2) return 2;
  LLVMContext Ctx;
  for (int a = 1; a < argc; ++a) {
    SMDiagnostic Err;
    std::unique_ptr<Module> M = parseAssemblyFile(argv[a], Err, Ctx);
    if (!M) {
      Err.print(argv[a], errs());
      return 1;
    }
    const DataLayout &DL = M->getDataLayout();
    for (Function &F : *M) {
      if (F.isDeclaration()) continue;
      std::string FN = F.getName().str();
      AssumptionCache AC(F);
      DominatorTree DT(F);
      DomConditionCache DC;
      std::vector<Instruction *> Insts;
      std::map<const Value *, std::string> Name;
      unsigned k = 0;
      for (Argument &Arg : F.args()) Name[&Arg] = "a" + std::to_string(k++);
      k = 0;
      for (BasicBlock &BB : F)
        for (Instruction &I : BB) {
          Name[&I] = "i" + std::to_string(k++);
          Insts.push_back(&I);
          if (auto *BI = dyn_cast<CondBrInst>(&I)) DC.registerBranch(BI);
        }
      auto nm = [&](const Value *V) -> std::string {
        auto It = Name.find(V);
        if (It != Name.end()) return It->second;
        if (auto *G = dyn_cast<GlobalValue>(V)) return "@" + G->getName().str();
        return "?";
      };
      auto scalar = [&](Type *Ty) -> unsigned {
        if (Ty->isPointerTy()) return DL.getPointerTypeSizeInBits(Ty);
        if (Ty->isIntegerTy()) return Ty->getIntegerBitWidth();
        return 0;
      };
      std::vector<Value *> Bools;
      for (Argument &Arg : F.args())
        if (Arg.getType()->isIntegerTy(1)) Bools.push_back(&Arg);
      for (Instruction *I : Insts)
        if (I->getType()->isIntegerTy(1)) Bools.push_back(I);
      std::vector<Instruction *> Assumes;
      for (Instruction *I : Insts)
        if (isa<AssumeInst>(I)) Assumes.push_back(I);

      for (Instruction *I : Insts) {
        SimplifyQuery Q(DL, nullptr, &DT, &AC, I, true, true, &DC);
        std::vector<Value *> Seen;
        for (Value *Op : I->operands()) {
          if (!isa<Argument>(Op) && !isa<Instruction>(Op)) continue;
          unsigned W = scalar(Op->getType());
          if (W == 0 || W > 128) continue;
          if (std::find(Seen.begin(), Seen.end(), Op) != Seen.end()) continue;
          Seen.push_back(Op);
          KnownBits K(W);
          computeKnownBitsFromContext(Op, K, Q, 0);
          std::string S;
          raw_string_ostream OS(S);
          K.print(OS);
          printf("%s ctx %s %s kb=%s\n", FN.c_str(), nm(I).c_str(), nm(Op).c_str(), S.c_str());
        }
      }
      if (Bools.size() <= 40) {
        for (Value *A : Bools)
          for (Value *B : Bools)
            for (int T = 0; T < 2; ++T)
              printf("%s imp %s %s %d %s\n", FN.c_str(), nm(A).c_str(), nm(B).c_str(), T,
                     tri(isImpliedCondition(A, B, DL, T)));
      }
      for (Instruction *I : Insts) {
        for (Value *C : Bools)
          printf("%s dom %s %s %s\n", FN.c_str(), nm(I).c_str(), nm(C).c_str(), tri(isImpliedByDomCondition(C, I, DL)));
        for (Instruction *J : Insts)
          if (auto *Cmp = dyn_cast<ICmpInst>(J))
            printf("%s domp %s %s %s\n", FN.c_str(), nm(I).c_str(), nm(J).c_str(),
                   tri(isImpliedByDomCondition(Cmp->getCmpPredicate(), Cmp->getOperand(0), Cmp->getOperand(1), I, DL)));
      }
      for (Value *C : Bools)
        for (int Asm = 0; Asm < 2; ++Asm) {
          std::string L;
          findValuesAffectedByCondition(C, Asm, [&](Value *V) { L += " " + nm(V); });
          printf("%s aff %s %d%s\n", FN.c_str(), nm(C).c_str(), Asm, L.c_str());
        }
      auto acache = [&](Value *V) {
        std::string L;
        for (auto &E : AC.assumptionsFor(V)) L += " " + nm(E.Assume);
        if (!L.empty()) printf("%s acache %s%s\n", FN.c_str(), nm(V).c_str(), L.c_str());
      };
      for (Argument &Arg : F.args()) acache(&Arg);
      for (Instruction *I : Insts) acache(I);
      for (Instruction *As : Assumes)
        for (Instruction *I : Insts) {
          printf("%s free %s %s %d\n", FN.c_str(), nm(As).c_str(), nm(I).c_str(), willNotFreeBetween(As, I));
          printf("%s valid %s %s 1 %d\n", FN.c_str(), nm(As).c_str(), nm(I).c_str(), isValidAssumeForContext(As, I, &DT));
          printf("%s valid %s %s 0 %d\n", FN.c_str(), nm(As).c_str(), nm(I).c_str(), isValidAssumeForContext(As, I, nullptr));
        }
    }
  }
  return 0;
}

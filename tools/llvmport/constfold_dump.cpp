// Prints what LLVM's ConstantFoldInstruction makes of every instruction of a module whose operands are all constants,
// one line each:  <function> <n> <opcode> <constant | ->
// where <n> numbers those instructions with a result and <constant> is written as in `show` (the Rust example `cfdump` writes
// the same). usage: constfold_dump FILE.ll...     `tools/llvmport/cfcompare.py` runs it beside `cfdump`.
#include "llvm/ADT/StringExtras.h"
#include "llvm/Config/llvm-config.h"
#include "llvm/Analysis/ConstantFolding.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/GlobalVariable.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Operator.h"
#include "llvm/Support/SourceMgr.h"
#include "llvm/Support/raw_ostream.h"
#include <cstdio>
#include <string>

using namespace llvm;

static std::string ty(Type *T) {
  std::string s;
  raw_string_ostream os(s);
  T->print(os, false, true);
  return s;
}

static std::string show(Constant *C) {
  std::string t = ty(C->getType());
  if (auto *I = dyn_cast<ConstantInt>(C)) return t + " " + toString(I->getValue(), 10, false);
  if (auto *F = dyn_cast<ConstantFP>(C)) {
    if (!F->getType()->isFloatTy() && !F->getType()->isDoubleTy()) return t + " fp";
    return t + " 0x" + toString(F->getValueAPF().bitcastToAPInt(), 16, false);
  }
  if (isa<ConstantPointerNull>(C)) return t + " null";
  if (isa<PoisonValue>(C)) return t + " poison";
  if (isa<UndefValue>(C)) return t + " undef";
  if (isa<ConstantAggregateZero>(C)) return t + " zeroinitializer";
  if (auto *G = dyn_cast<GlobalValue>(C)) return t + " @" + G->getName().str();
  if (isa<ConstantAggregate>(C) || isa<ConstantDataSequential>(C)) {
    std::string s = t + " ";
    std::string open = C->getType()->isStructTy() ? "{ " : C->getType()->isVectorTy() ? "<" : "[";
    std::string close = C->getType()->isStructTy() ? " }" : C->getType()->isVectorTy() ? ">" : "]";
    s += open;
    for (unsigned i = 0; Constant *E = C->getAggregateElement(i); ++i) s += (i ? ", " : "") + show(E);
    return s + close;
  }
  if (auto *CE = dyn_cast<ConstantExpr>(C)) {
    if (auto *GEP = dyn_cast<GEPOperator>(CE)) {
      std::string flags;
      if (GEP->isInBounds()) flags += ",inbounds";
      else if (GEP->hasNoUnsignedSignedWrap()) flags += ",nusw";
      if (GEP->hasNoUnsignedWrap()) flags += ",nuw";
      std::string s = t + " gep[" + flags.substr(flags.empty() ? 0 : 1) + "] " + ty(GEP->getSourceElementType()) + " (";
      for (unsigned i = 0; i < CE->getNumOperands(); ++i) s += (i ? ", " : "") + show(cast<Constant>(CE->getOperand(i)));
      return s + ")";
    }
    if (CE->isCast()) return t + " " + CE->getOpcodeName() + " (" + show(CE->getOperand(0)) + ")";
    return t + " expr:" + CE->getOpcodeName();
  }
  return t + " ?";
}

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s\n", LLVM_VERSION_STRING);
    return 0;
  }
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
      unsigned n = 0;
      for (BasicBlock &BB : F)
        for (Instruction &I : BB) {
          if (I.getType()->isVoidTy()) continue;
          bool all = true;
          for (Use &U : I.operands()) all &= isa<Constant>(U);
          if (!all) continue;
          Constant *R = ConstantFoldInstruction(&I, DL, nullptr);
          printf("%s %u %s %s\n", F.getName().str().c_str(), n++, I.getOpcodeName(), R ? show(R).c_str() : "-");
        }
    }
  }
  return 0;
}

// Prints what LLVM's ValueTracking proves of every integer and pointer value of a module, one line each:
//   <function> <n> <column>=<answer> ...
// where <n> numbers the function's arguments and then its instructions that have a scalar integer or pointer result.
// usage: vtdump COLS FILE.ll...   COLS is a comma list of: kb sb nz p2 p2z gp nn ng cru crs sp ov (the order printed)
// `sp` is matchSelectPattern of a select: flavor/NaN behaviour/ordered/cast/LHS/RHS (`-` on anything else).
// `ov` is computeOverflowFor* of an add, sub or mul, as letters M(ay) L(ow) H(igh) N(ever): add u/s(AddOperator)/s(operands), sub u/s, mul u/s.
// `tools/llvmport/vtcompare.py` runs it beside the Rust `vtdump` example and compares the lines.
#include "llvm/Analysis/AssumptionCache.h"
#include "llvm/Analysis/SimplifyQuery.h"
#include "llvm/Analysis/ValueTracking.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/ConstantRange.h"
#include "llvm/IR/Dominators.h"
#include "llvm/ADT/StringExtras.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/Operator.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/KnownBits.h"
#include "llvm/Support/SourceMgr.h"
#include <cstdio>
#include <set>
#include <sstream>
#include <string>

using namespace llvm;

static std::set<std::string> split(const std::string &s) {
  std::set<std::string> out;
  std::stringstream ss(s);
  std::string item;
  while (std::getline(ss, item, ',')) out.insert(item);
  return out;
}

static std::string kb(const KnownBits &K) {
  std::string s;
  llvm::raw_string_ostream os(s);
  K.print(os);
  return s;
}

static std::string cr(const ConstantRange &R) {
  std::string s;
  llvm::raw_string_ostream os(s);
  R.print(os);
  return s;
}

static std::string spValue(Value *V) {
  if (auto *CI = dyn_cast<ConstantInt>(V))
    return "i" + std::to_string(CI->getBitWidth()) + ":" + toString(CI->getValue(), 10, false);
  if (auto *CF = dyn_cast<ConstantFP>(V))
    return "f:" + toString(CF->getValueAPF().bitcastToAPInt(), 16, false);
  if (isa<PoisonValue>(V)) return "poison";
  return V->hasName() ? V->getName().str() : "?";
}

static std::string sp(Value *V) {
  if (!isa<SelectInst>(V)) return "-";
  Value *L = nullptr, *R = nullptr;
  Instruction::CastOps CO = Instruction::CastOpsEnd;
  SelectPatternResult SPR = matchSelectPattern(V, L, R, &CO);
  static const char *Flavors[] = {"none", "smin", "umin", "smax", "umax", "fminnum", "fmaxnum", "abs", "nabs"};
  static const char *NaNs[] = {"na", "nan", "other", "any"};
  std::string s = Flavors[SPR.Flavor];
  if (SPR.Flavor == SPF_UNKNOWN) return s;
  s += std::string("/") + NaNs[SPR.NaNBehavior] + "/" + (SPR.Ordered ? "1" : "0") + "/";
  s += CO == Instruction::CastOpsEnd ? "-" : Instruction::getOpcodeName(CO);
  return s + "/" + spValue(L) + "/" + spValue(R);
}

static char ovLetter(OverflowResult R) {
  switch (R) {
  case OverflowResult::MayOverflow: return 'M';
  case OverflowResult::AlwaysOverflowsLow: return 'L';
  case OverflowResult::AlwaysOverflowsHigh: return 'H';
  case OverflowResult::NeverOverflows: return 'N';
  }
  return '?';
}

static std::string ov(Value *V, const SimplifyQuery &Q) {
  auto *I = dyn_cast<BinaryOperator>(V);
  if (!I) return "-";
  Value *A = I->getOperand(0), *B = I->getOperand(1);
  std::string s;
  switch (I->getOpcode()) {
  case Instruction::Add:
    s += ovLetter(computeOverflowForUnsignedAdd(A, B, Q));
    s += ovLetter(computeOverflowForSignedAdd(cast<AddOperator>(I), Q));
    s += ovLetter(computeOverflowForSignedAdd(A, B, Q));
    return s;
  case Instruction::Sub:
    s += ovLetter(computeOverflowForUnsignedSub(A, B, Q));
    s += ovLetter(computeOverflowForSignedSub(A, B, Q));
    return s;
  case Instruction::Mul:
    s += ovLetter(computeOverflowForUnsignedMul(A, B, Q, I->hasNoSignedWrap()));
    s += ovLetter(computeOverflowForSignedMul(A, B, Q));
    return s;
  default: return "-";
  }
}

int main(int argc, char **argv) {
  if (argc < 3) return 2;
  std::set<std::string> cols = split(argv[1]);
  LLVMContext Ctx;
  for (int a = 2; a < argc; ++a) {
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
      unsigned n = 0;
      auto dump = [&](Value *V, Instruction *CxtI) {
        Type *Ty = V->getType();
        if (!Ty->isIntegerTy() && !Ty->isPointerTy()) return;
        if (Ty->isIntegerTy() && Ty->getIntegerBitWidth() > 128) return;
        printf("%s %u", F.getName().str().c_str(), n++);
        SimplifyQuery Q(DL, /*TLI*/ nullptr, &DT, &AC, CxtI);
        if (cols.count("kb")) printf(" kb=%s", kb(computeKnownBits(V, Q, 0)).c_str());
        if (cols.count("sb")) printf(" sb=%u", ComputeNumSignBits(V, DL, &AC, CxtI, &DT, true, 0));
        if (cols.count("nz")) printf(" nz=%d", isKnownNonZero(V, Q, 0));
        if (cols.count("p2")) printf(" p2=%d", isKnownToBeAPowerOfTwo(V, DL, false, &AC, CxtI, &DT, true, 0));
        if (cols.count("p2z")) printf(" p2z=%d", isKnownToBeAPowerOfTwo(V, DL, true, &AC, CxtI, &DT, true, 0));
        if (cols.count("gp"))
          printf(" gp=%d%d", isGuaranteedNotToBePoison(V, &AC, CxtI, &DT), isGuaranteedNotToBeUndef(V, &AC, CxtI, &DT));
        if (cols.count("nn")) printf(" nn=%d", isKnownNonNegative(V, Q, 0));
        if (cols.count("ng")) printf(" ng=%d", isKnownNegative(V, Q, 0));
        if (Ty->isIntegerTy()) {
          if (cols.count("cru")) printf(" cru=%s", cr(computeConstantRange(V, false, Q, 0)).c_str());
          if (cols.count("crs")) printf(" crs=%s", cr(computeConstantRange(V, true, Q, 0)).c_str());
        }
        if (cols.count("sp")) printf(" sp=%s", sp(V).c_str());
        if (cols.count("ov")) printf(" ov=%s", ov(V, Q).c_str());
        printf("\n");
      };
      for (Argument &Arg : F.args()) dump(&Arg, &*F.getEntryBlock().begin());
      for (BasicBlock &BB : F)
        for (Instruction &I : BB) dump(&I, &I);
    }
  }
  return 0;
}

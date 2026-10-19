// Prints what LLVM's ValueTracking proves of every integer and pointer value of a module, one line each:
//   <function> <n> <column>=<answer> ...
// where <n> numbers the function's arguments and then its instructions that have a scalar integer or pointer result.
// usage: vtdump COLS FILE.ll...   COLS is a comma list of: kb sb nz p2 p2z gp nn ng cru crs pat ov bw uo (the order printed)
// `bw` is isBytewiseValue: none, self (an i8 value), poison, or i8:<n>; it also prints one `<function> c bw=<answer>` line per constant
// operand of the function that is not a global, a block or an alloca's size, in instruction order.
// `uo` is getUnderlyingObject of a pointer: the dump number of the value it is, `@name`, `null` or `const`.
// `pat` is matchSelectPattern of a select: flavor/NaN behaviour/ordered/cast/LHS/RHS (`-` on anything else).
// `ov` is computeOverflowFor* of an add, sub or mul, as letters M(ay) L(ow) H(igh) N(ever): add u/s(AddOperator)/s(operands), sub u/s, mul u/s.
// `tools/llvmport/vtcompare.py` runs it beside the Rust `vtdump` example and compares the lines.
#include "llvm/Config/llvm-config.h"
#include "llvm/Analysis/AssumptionCache.h"
#include "llvm/Analysis/SimplifyQuery.h"
#include "llvm/Analysis/ValueTracking.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/ConstantRange.h"
#include "llvm/IR/Dominators.h"
#include "llvm/ADT/StringExtras.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/Operator.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/KnownBits.h"
#include "llvm/Support/SourceMgr.h"
#include <cstdio>
#include <map>
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

static std::string bw(Value *V, const DataLayout &DL) {
  Value *R = isBytewiseValue(V, DL);
  if (!R) return "none";
  if (auto *CI = dyn_cast<ConstantInt>(R)) return "i8:" + std::to_string(CI->getZExtValue());
  if (isa<PoisonValue>(R)) return "poison";
  if (isa<UndefValue>(R)) return "undef";
  return "self";
}

static std::string uo(Value *V, std::map<Value *, unsigned> &numbers) {
  Value *U = getUnderlyingObject(V);
  if (auto *G = dyn_cast<GlobalValue>(U)) return "@" + G->getName().str();
  if (isa<ConstantPointerNull>(U)) return "null";
  if (isa<Constant>(U)) return "const";
  auto at = numbers.find(U);
  return at == numbers.end() ? "?" : std::to_string(at->second);
}

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s\n", LLVM_VERSION_STRING);
    return 0;
  }
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
      std::map<Value *, unsigned> numbers;
      {
        unsigned k = 0;
        auto number = [&](Value *V) {
          Type *Ty = V->getType();
          if (!Ty->isIntegerTy() && !Ty->isPointerTy()) return;
          if (Ty->isIntegerTy() && Ty->getIntegerBitWidth() > 128) return;
          numbers[V] = k++;
        };
        for (Argument &Arg : F.args()) number(&Arg);
        for (BasicBlock &BB : F)
          for (Instruction &I : BB) number(&I);
      }
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
        if (cols.count("pat")) printf(" pat=%s", sp(V).c_str());
        if (cols.count("ov")) printf(" ov=%s", ov(V, Q).c_str());
        if (cols.count("bw")) printf(" bw=%s", bw(V, DL).c_str());
        if (cols.count("uo") && Ty->isPointerTy()) printf(" uo=%s", uo(V, numbers).c_str());
        printf("\n");
      };
      for (Argument &Arg : F.args()) dump(&Arg, &*F.getEntryBlock().begin());
      for (BasicBlock &BB : F)
        for (Instruction &I : BB) dump(&I, &I);
      if (cols.count("bw"))
        for (BasicBlock &BB : F)
          for (Instruction &I : BB)
            for (Value *Op : I.operands())
              if (!isa<AllocaInst>(I) && isa<Constant>(Op) && !isa<GlobalValue>(Op) && !Op->getType()->isVoidTy() && !Op->getType()->isLabelTy())
                printf("%s c bw=%s\n", F.getName().str().c_str(), bw(Op, DL).c_str());
    }
  }
  return 0;
}

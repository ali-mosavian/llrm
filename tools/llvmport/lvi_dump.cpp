// Prints what LLVM's LazyValueInfo answers for a module, one line per query:
//   val   <fn> <n> cr=<range|-> cst=<const|-> pn=<T|F|->     every int or pointer argument and result, at itself
//   rval  <fn> <n> cr=<range>                               every int result, last first, in a fresh analysis
//   edge  <fn> <from> <to> <n> cr=<range>                    every int value an instruction of <to> uses, on the edge
//   pred  <fn> <i> a=<T|F|-> b=<T|F|->                       every icmp, getPredicateAt with / without block values
//   epred <fn> <from> <succ> <i> <T|F|->                     every conditional icmp branch, getPredicateOnEdge
//   use   <fn> <i> <k> cr=<range>                            every int operand use, getConstantRangeAtUse
//   lat   <fn> arg<k>|i<i> bb<b> <lattice>                   printLVI's annotations, sorted; only for functions whose blocks,
//                                                            arguments and results all have names
// <n> numbers the function's arguments then its int or pointer results (as vtdump.cpp), <i> its instructions, <from>
// and <to> its blocks, all in order. Unreachable blocks and values that do not dominate the query are skipped.
// usage: lvi_dump FILE.ll...     `tools/llvmport/lvicompare.py` runs it beside the Rust `lvidump` example.
#include "llvm/Config/llvm-config.h"
#include "llvm/Analysis/AssumptionCache.h"
#include "llvm/Analysis/LazyValueInfo.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/ConstantRange.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/Dominators.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/SourceMgr.h"
#include <cstdio>
#include <map>
#include <algorithm>
#include <set>
#include <sstream>
#include <vector>
#include <string>

using namespace llvm;

static std::string cr(const ConstantRange &R) {
  std::string s = "i" + std::to_string(R.getBitWidth()) + ":";
  llvm::raw_string_ostream os(s);
  R.print(os);
  return s;
}

static char tf(Constant *C) {
  if (!C) return '-';
  if (auto *CI = dyn_cast<ConstantInt>(C)) return CI->isZero() ? 'F' : 'T';
  return '?';
}

static std::string cst(Constant *C) {
  if (!C) return "-";
  if (auto *CI = dyn_cast<ConstantInt>(C))
    return "i" + std::to_string(CI->getBitWidth()) + ":" + toString(CI->getValue(), 10, false);
  if (C->isNullValue()) return "null";
  if (auto *GV = dyn_cast<GlobalValue>(C)) return "@" + GV->getName().str();
  return "?";
}

static bool scalar(Type *Ty) {
  if (Ty->isPointerTy()) return true;
  return Ty->isIntegerTy() && Ty->getIntegerBitWidth() <= 128;
}

static bool isInt(Type *Ty) { return Ty->isIntegerTy() && Ty->getIntegerBitWidth() <= 128; }

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
    for (Function &F : *M) {
      if (F.isDeclaration()) continue;
      AssumptionCache AC(F);
      DominatorTree DT(F);
      LazyValueInfo LVI(&F, &AC);
      const char *Name = F.getName().data();
      std::map<Value *, unsigned> Number, Index;
      std::map<BasicBlock *, unsigned> Block;
      unsigned n = 0, u = 0, b = 0;
      for (Argument &Arg : F.args())
        if (scalar(Arg.getType())) Number[&Arg] = n++;
      for (BasicBlock &BB : F) {
        Block[&BB] = b++;
        for (Instruction &I : BB) {
          Index[&I] = u++;
          if (scalar(I.getType())) Number[&I] = n++;
        }
      }
      // A value that may be asked about at the end of `At`.
      auto dominatesBlock = [&](Value *V, BasicBlock *At) {
        auto *Def = dyn_cast<Instruction>(V);
        return !Def || DT.dominates(Def->getParent(), At);
      };
      Instruction *EntryFirst = &*F.getEntryBlock().begin();
      auto dumpValue = [&](Value *V, Instruction *CxtI) {
        if (!Number.count(V)) return;
        if (isa<Instruction>(V) && !DT.isReachableFromEntry(cast<Instruction>(V)->getParent())) return;
        printf("val %s %u", Name, Number[V]);
        if (isInt(V->getType()))
          printf(" cr=%s", cr(LVI.getConstantRange(V, CxtI, false)).c_str());
        else
          printf(" cr=-");
        printf(" cst=%s", cst(LVI.getConstant(V, CxtI)).c_str());
        if (V->getType()->isPointerTy())
          printf(" pn=%c", tf(LVI.getPredicateAt(CmpInst::ICMP_NE, V,
                                                 Constant::getNullValue(V->getType()), CxtI, true)));
        else
          printf(" pn=-");
        printf("\n");
      };
      for (Argument &Arg : F.args()) dumpValue(&Arg, EntryFirst);
      for (BasicBlock &BB : F)
        if (DT.isReachableFromEntry(&BB))
          for (Instruction &I : BB) dumpValue(&I, &I);

      // The same values, last first, in a fresh analysis: the deepest chain is solved by one query.
      {
        LazyValueInfo Fresh(&F, &AC);
        for (auto BB = F.end(); BB != F.begin();) {
          --BB;
          if (!DT.isReachableFromEntry(&*BB)) continue;
          for (auto I = BB->end(); I != BB->begin();) {
            --I;
            if (!isInt(I->getType())) continue;
            printf("rval %s %u cr=%s\n", Name, Number[&*I], cr(Fresh.getConstantRange(&*I, &*I, false)).c_str());
          }
        }
      }

      // Edges.
      for (BasicBlock &BB : F) {
        if (!DT.isReachableFromEntry(&BB)) continue;
        std::set<BasicBlock *> Seen;
        for (BasicBlock *S : successors(&BB)) {
          if (!Seen.insert(S).second) continue;
          std::set<Value *> Done;
          for (Instruction &I : *S)
            for (unsigned k = 0; k < I.getNumOperands(); ++k) {
              Value *V = I.getOperand(k);
              if (isa<Constant>(V) || !isInt(V->getType()) || !dominatesBlock(V, &BB)) continue;
              if (auto *PN = dyn_cast<PHINode>(&I))
                if (PN->getIncomingBlock(k) != &BB) continue;
              if (!Done.insert(V).second) continue;
              printf("edge %s %u %u %u cr=%s\n", Name, Block[&BB], Block[S], Number[V],
                     cr(LVI.getConstantRangeOnEdge(V, &BB, S, nullptr)).c_str());
            }
        }
      }

      // Predicates at each icmp.
      for (BasicBlock &BB : F) {
        if (!DT.isReachableFromEntry(&BB)) continue;
        for (Instruction &I : BB) {
          auto *Cmp = dyn_cast<ICmpInst>(&I);
          if (!Cmp) continue;
          Value *A = Cmp->getOperand(0), *B = Cmp->getOperand(1);
          if (!scalar(A->getType())) continue;
          if (!dominatesBlock(A, &BB) || !dominatesBlock(B, &BB)) continue;
          printf("pred %s %u a=%c b=%c\n", Name, Index[&I],
                 tf(LVI.getPredicateAt(Cmp->getPredicate(), A, B, &I, true)),
                 tf(LVI.getPredicateAt(Cmp->getPredicate(), A, B, &I, false)));
        }
      }

      // Predicates on the edges of conditional branches.
      for (BasicBlock &BB : F) {
        if (!DT.isReachableFromEntry(&BB)) continue;
        auto *Br = dyn_cast<CondBrInst>(BB.getTerminator());
        if (!Br) continue;
        auto *Cmp = dyn_cast<ICmpInst>(Br->getCondition());
        if (!Cmp || !scalar(Cmp->getOperand(0)->getType())) continue;
        Value *V;
        Constant *C;
        CmpInst::Predicate Pred = Cmp->getPredicate();
        if ((C = dyn_cast<Constant>(Cmp->getOperand(1)))) {
          V = Cmp->getOperand(0);
        } else if ((C = dyn_cast<Constant>(Cmp->getOperand(0)))) {
          V = Cmp->getOperand(1);
          Pred = CmpInst::getSwappedPredicate(Pred);
        } else {
          continue;
        }
        if (isa<Constant>(V) || !dominatesBlock(V, &BB)) continue;
        for (unsigned s = 0; s < 2; ++s)
          printf("epred %s %u %u %u %c\n", Name, Block[&BB], s, Index[Cmp],
                 tf(LVI.getPredicateOnEdge(Pred, V, C, &BB, Br->getSuccessor(s), nullptr)));
      }

      // Uses.
      for (BasicBlock &BB : F) {
        if (!DT.isReachableFromEntry(&BB)) continue;
        for (Instruction &I : BB)
          for (unsigned k = 0; k < I.getNumOperands(); ++k) {
            Value *V = I.getOperand(k);
            if (isa<Constant>(V) || !isInt(V->getType())) continue;
            if (auto *Def = dyn_cast<Instruction>(V)) {
              if (Def->getParent() == &BB ? (isa<PHINode>(&I) || !Def->comesBefore(&I))
                                          : !DT.dominates(Def->getParent(), &BB))
                continue;
            }
            printf("use %s %u %u cr=%s\n", Name, Index[&I], k,
                   cr(LVI.getConstantRangeAtUse(I.getOperandUse(k), false)).c_str());
          }
      }

      // printLVI.
      bool Named = true;
      for (Argument &Arg : F.args()) Named &= Arg.hasName();
      for (BasicBlock &BB : F) {
        Named &= BB.hasName();
        for (Instruction &I : BB) Named &= I.getType()->isVoidTy() || I.hasName();
      }
      if (Named) {
        std::string Text;
        raw_string_ostream OS(Text);
        LVI.printLVI(F, DT, OS);
        OS.flush();
        std::map<std::string, unsigned> BlockByText;
        for (BasicBlock &BB : F) {
          std::string T;
          raw_string_ostream BOS(T);
          BB.printAsOperand(BOS, false);
          BOS.flush();
          BlockByText[T] = Block[&BB];
        }
        std::map<std::string, unsigned> ArgByText;
        unsigned k = 0;
        for (Argument &Arg : F.args()) {
          std::string T;
          raw_string_ostream AOS(T);
          AOS << Arg;
          AOS.flush();
          ArgByText[T] = k++;
        }
        std::vector<std::string> Lines, Pending;
        unsigned Next = 0, CurBlock = 0;
        std::istringstream In(Text);
        std::string Line;
        const std::string Head = "; LatticeVal for: '";
        std::string Open;
        while (std::getline(In, Line)) {
          if (!Open.empty()) {
            // An instruction that prints on several lines (a switch) spans its annotation's.
            Open += "\n" + Line;
            if (Open.find("' is: ") != std::string::npos) {
              Pending.push_back(Open);
              Open.clear();
            }
          } else if (Line.compare(0, Head.size(), Head) == 0) {
            if (Line.find("' is: ") == std::string::npos)
              Open = Line;
            else
              Pending.push_back(Line);
          } else if (Line.size() > 2 && Line[0] != ' ' && Line[0] != ';' && Line.compare(0, 6, "define") != 0 &&
                     Line[0] != '}' && Line.find(':') != std::string::npos) {
            CurBlock = BlockByText["%" + Line.substr(0, Line.find(':'))];
          } else if (Line.size() > 2 && Line[0] == ' ' && Line[1] == ' ' && Line[2] != ' ' && Line[2] != ';' &&
                     Line[2] != ']') {
            for (const std::string &P : Pending) {
              size_t Is = P.rfind("' is: ");
              size_t InBB = P.rfind("' in BB: '");
              std::string Subject = P.substr(Head.size(), std::min(Is, InBB) - Head.size());
              if (InBB == std::string::npos) {
                Lines.push_back("lat " + std::string(Name) + " arg" + std::to_string(ArgByText[Subject]) + " bb" +
                                std::to_string(CurBlock) + " " + P.substr(Is + 6));
              } else {
                size_t BE = Is;
                std::string BBText = P.substr(InBB + 10, BE - InBB - 10);
                Lines.push_back("lat " + std::string(Name) + " i" + std::to_string(Next) + " bb" +
                                std::to_string(BlockByText[BBText]) + " " + P.substr(BE + 6));
              }
            }
            Pending.clear();
            ++Next;
          }
        }
        // `*Constant` writes a global's whole definition; MIR writes its type and name.
        for (std::string &L : Lines) {
          size_t At = L.find("constant<@");
          if (At == std::string::npos) continue;
          size_t Eq = L.find(" = ", At), End = L.find('>', At);
          L = L.substr(0, At) + "constant<ptr " + L.substr(At + 9, Eq - At - 9) + L.substr(End);
        }
        std::sort(Lines.begin(), Lines.end());
        for (const std::string &L : Lines) printf("%s\n", L.c_str());
      }
    }
  }
  return 0;
}

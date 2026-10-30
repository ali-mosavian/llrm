// Prints LLVM's DominatorTree and PostDominatorTree of every function of the given modules, in the format of
// crates/ir/llrm-mir/examples/domdump.rs. Blocks are named by their position in the function.
//   @f                           a function
//   dt <i> <idom> <level> <kids>  idom: a block, or - for the root; kids sorted; `x` for a block not in the tree
//   dt roots <list>
//   pdt <i> ...                  idom V: the virtual root; `pdt V` is the virtual root's line
// usage: domdump FILE.ll...   `tools/llvmport/domdump.sh` runs it.
#include "llvm/Analysis/PostDominators.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/Config/llvm-config.h"
#include "llvm/IR/Dominators.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/SourceMgr.h"
#include "llvm/Support/VCSRevision.h"
#include <algorithm>
#include <cstdio>
#include <map>
#include <string>
#include <vector>

using namespace llvm;

template <typename DT> static void dump(const char *tag, const DT &T, Function &F, std::map<const BasicBlock *, unsigned> &index) {
  auto num = [&](const BasicBlock *B) { return std::to_string(index.at(B)); };
  auto line = [&](const std::string &name, const BasicBlock *B) {
    auto *N = T.getNode(B);
    if (!N) {
      printf("%s %s x\n", tag, name.c_str());
      return;
    }
    std::string idom = "-";
    if (auto *I = N->getIDom()) idom = I->getBlock() ? num(I->getBlock()) : "V";
    std::vector<unsigned> kids;
    for (auto *C : *N)
      if (C->getBlock()) kids.push_back(index.at(C->getBlock()));
    std::sort(kids.begin(), kids.end());
    std::string list;
    for (unsigned k : kids) list += std::to_string(k) + ",";
    printf("%s %s %s %u %s\n", tag, name.c_str(), idom.c_str(), N->getLevel(), list.c_str());
  };
  if (T.isPostDominator()) line("V", nullptr);
  for (BasicBlock &B : F) line(num(&B), &B);
  std::vector<unsigned> roots;
  for (auto *R : T.roots()) roots.push_back(index.at(R));
  std::sort(roots.begin(), roots.end());
  std::string list;
  for (unsigned r : roots) list += std::to_string(r) + ",";
  printf("%s roots %s\n", tag, list.c_str());
}

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s %s\n", LLVM_VERSION_STRING, LLVM_REVISION);
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
      std::map<const BasicBlock *, unsigned> index;
      unsigned k = 0;
      for (BasicBlock &B : F) index[&B] = k++;
      printf("@%s\n", F.getName().str().c_str());
      DominatorTree DT(F);
      PostDominatorTree PDT(F);
      dump("dt", DT, F, index);
      dump("pdt", PDT, F, index);
    }
  }
  return 0;
}

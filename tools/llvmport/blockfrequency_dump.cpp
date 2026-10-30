// Writes what LLVM's CycleInfo and BlockFrequencyInfo make of fixed and seeded random graphs, every edge's probability
// set from a weight: the golden file crates/ir/llrm-mir/src/blockfrequency/golden.txt, which `blockfrequency/tests.rs`
// recomputes from the Rust port.
//   tools/llvmport/blockfrequency_dump.sh > crates/ir/llrm-mir/src/blockfrequency/golden.txt
// A line is `<succs> | <weights> | freq <per block, 0 unreached> | irr <headers> | cycles <header:depth:reducible:blocks>...`;
// successors keep repeats (a switch case to a block twice), the first is the switch's default.
#include "llvm/Analysis/BlockFrequencyInfo.h"
#include "llvm/Analysis/BranchProbabilityInfo.h"
#include "llvm/IR/BasicBlock.h"
#include "llvm/IR/CycleInfo.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Support/BranchProbability.h"
#include "llvm/Support/VCSRevision.h"
#include <cstdint>
#include <cstdio>
#include <string>
#include <vector>

using namespace llvm;
using Graph = std::vector<std::vector<unsigned>>;

static uint64_t Seed = 0x2545F4914F6CDD1DULL;
static unsigned next(unsigned n) {
  Seed = Seed * 6364136223846793005ULL + 1442695040888963407ULL;
  return unsigned(Seed >> 33) % n;
}

static void run(LLVMContext &C, const Graph &g, const Graph &w) {
  Module M("m", C);
  auto *Ty = FunctionType::get(Type::getVoidTy(C), {Type::getInt32Ty(C)}, false);
  Function *F = Function::Create(Ty, Function::ExternalLinkage, "f", M);
  std::vector<BasicBlock *> bb;
  for (unsigned b = 0; b < g.size(); ++b)
    bb.push_back(BasicBlock::Create(C, "b" + std::to_string(b), F));
  for (unsigned b = 0; b < g.size(); ++b) {
    IRBuilder<> B(bb[b]);
    const auto &s = g[b];
    if (s.empty()) {
      B.CreateRetVoid();
    } else if (s.size() == 1) {
      B.CreateBr(bb[s[0]]);
    } else {
      SwitchInst *S = B.CreateSwitch(F->getArg(0), bb[s[0]], s.size() - 1);
      for (unsigned i = 1; i < s.size(); ++i)
        S->addCase(B.getInt32(i), bb[s[i]]);
    }
  }
  CycleInfo CI;
  CI.compute(*F);
  BranchProbabilityInfo BPI(*F, CI);
  for (unsigned b = 0; b < g.size(); ++b) {
    if (g[b].empty())
      continue;
    std::vector<BranchProbability> P;
    for (unsigned x : w[b])
      P.push_back(BranchProbability::getRaw(x));
    BranchProbability::normalizeProbabilities(P);
    BPI.setEdgeProbability(bb[b], P);
  }
  BlockFrequencyInfo BFI;
  BFI.calculate(*F, BPI, CI);
  std::string line;
  for (unsigned b = 0; b < g.size(); ++b) {
    line += std::to_string(b) + ":";
    for (unsigned i = 0; i < g[b].size(); ++i)
      line += (i ? "," : "") + std::to_string(g[b][i]);
    line += b + 1 < g.size() ? ";" : "";
  }
  line += " | ";
  for (unsigned b = 0; b < g.size(); ++b) {
    for (unsigned i = 0; i < w[b].size(); ++i)
      line += (i ? "," : "") + std::to_string(w[b][i]);
    line += b + 1 < g.size() ? ";" : "";
  }
  line += " | freq";
  for (unsigned b = 0; b < g.size(); ++b)
    line += " " + std::to_string(BFI.getBlockFreq(bb[b]).getFrequency());
  line += " | irr";
  for (unsigned b = 0; b < g.size(); ++b)
    if (BFI.isIrrLoopHeader(bb[b]))
      line += " " + std::to_string(b);
  line += " | cycles";
  for (CycleRef Cy : CI.cycles()) {
    line += " " + std::to_string(CI.getHeader(Cy)->getNumber()) + ":" + std::to_string(CI.getDepth(Cy)) + ":" +
            (CI.isReducible(Cy) ? "r" : "i") + ":";
    bool first = true;
    for (BasicBlock *X : CI.getBlocks(Cy)) {
      line += (first ? "" : ",") + std::to_string(X->getNumber());
      first = false;
    }
  }
  std::printf("%s\n", line.c_str());
}

static Graph weightsFor(const Graph &g, unsigned zeroOneIn) {
  Graph w(g.size());
  for (unsigned b = 0; b < g.size(); ++b)
    for (unsigned i = 0; i < g[b].size(); ++i)
      w[b].push_back(zeroOneIn && !next(zeroOneIn) ? 0 : 1 + next(100));
  return w;
}

int main() {
  std::printf("# LLVM %s %s\n", LLVM_VERSION_STRING, LLVM_REVISION);
  LLVMContext C;
  std::vector<Graph> fixed = {
      {{}},
      {{1}, {2}, {}},
      {{1, 2}, {3}, {3}, {}},
      {{1}, {1, 2}, {}},
      {{1}, {2, 3}, {1}, {}},
      {{1}, {2}, {3, 2}, {1, 4}, {}},
      {{1, 2}, {2}, {1, 3}, {}},
      {{1}, {2, 3}, {3}, {2, 4}, {1, 5}, {}},
      {{1}, {1}},
      {{1, 1, 2}, {2, 2}, {}},
      {{1, 2}, {3}, {3, 4}, {1, 5}, {3}, {}},
      {{1}, {2, 3}, {4}, {4, 2}, {1, 5}, {}},
      {{1, 4}, {2}, {3, 1}, {1, 5}, {2}, {}},
  };
  for (const auto &g : fixed)
    run(C, g, weightsFor(g, 0));
  // A block with more distinct targets than combineWeights sorts (128).
  Graph wide(203);
  for (unsigned i = 1; i <= 201; ++i) {
    wide[0].push_back(i);
    wide[i].push_back(i % 3 ? 202 : 0 < i - 1 ? i - 1 : 202);
  }
  run(C, wide, weightsFor(wide, 0));
  for (unsigned k = 0; k < 3000; ++k) {
    unsigned n = 2 + next(11);
    Graph g(n);
    for (unsigned b = 0; b + 1 < n; ++b) {
      unsigned out = 1 + next(3);
      for (unsigned i = 0; i < out; ++i)
        g[b].push_back(next(4) ? 1 + next(n - 1) : b + 1);
    }
    run(C, g, weightsFor(g, k % 4 == 0 ? 6 : 0));
  }
  return 0;
}

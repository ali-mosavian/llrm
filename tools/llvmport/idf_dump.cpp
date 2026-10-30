// Writes the iterated dominance frontier LLVM's ForwardIDFCalculator and ReverseIDFCalculator give on the fixed 8-block graph of
// domtree_dump.cpp and each of its 64 single-edge flips: the golden file crates/ir/llrm-mir/src/idf_golden.txt, which `idf_tests.rs`
// recomputes from the Rust port.
//   tools/llvmport/idf_dump.sh > crates/ir/llrm-mir/src/idf_golden.txt
// A `list` line is `list <fwd|rev> <defining mask> <live-in mask or -> : <blocks in the order calculate() gives them>` for the base graph; a
// `hash` line is `hash <fwd|rev> <graph key> <live-in mode> <hash of the lists over all 256 defining masks>`.
#include "llvm/Config/llvm-config.h"
#include "llvm/IR/BasicBlock.h"
#include "llvm/IR/Dominators.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Analysis/PostDominators.h"
#include "llvm/Analysis/IteratedDominanceFrontier.h"
#include "llvm/ADT/SmallPtrSet.h"
#include "llvm/Support/VCSRevision.h"
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <string>
#include <utility>
#include <vector>

using namespace llvm;
using Graph = std::vector<std::vector<unsigned>>;
using Edge = std::pair<unsigned, unsigned>;

static const unsigned N = 8;

static Graph base() { return {{1, 2}, {3}, {3, 4}, {1, 5}, {3, 6}, {6}, {5}, {2}}; }

static bool has(const Graph &g, Edge e) {
  for (unsigned t : g[e.first])
    if (t == e.second) return true;
  return false;
}

// The graph with the edge flipped, and the update that did it.
static Graph flip(const Graph &g, Edge e, bool &insert) {
  Graph r = g;
  insert = !has(g, e);
  if (insert) {
    r[e.first].push_back(e.second);
  } else {
    std::vector<unsigned> keep;
    for (unsigned t : r[e.first])
      if (t != e.second) keep.push_back(t);
    r[e.first] = keep;
  }
  return r;
}

// Replaces the terminators of F by the successors of G: none: ret; one: br; more: a switch whose default is the first.
static void setTerminators(Function &F, const Graph &g, std::vector<BasicBlock *> &bb) {
  LLVMContext &C = F.getContext();
  Value *cond = F.getArg(0), *sel = F.getArg(1);
  for (unsigned b = 0; b < N; ++b) {
    if (bb[b]->hasTerminator()) bb[b]->getTerminator()->eraseFromParent();
    IRBuilder<> B(bb[b]);
    const auto &s = g[b];
    if (s.empty()) {
      B.CreateRetVoid();
    } else if (s.size() == 1) {
      B.CreateBr(bb[s[0]]);
    } else if (s.size() == 2) {
      B.CreateCondBr(cond, bb[s[0]], bb[s[1]]);
    } else {
      SwitchInst *sw = B.CreateSwitch(sel, bb[s[0]], s.size() - 1);
      for (unsigned i = 1; i < s.size(); ++i) sw->addCase(ConstantInt::get(Type::getInt32Ty(C), i), bb[s[i]]);
    }
  }
}

static uint64_t H;
static void mix(const std::string &s) {
  H = 0xcbf29ce484222325ULL;
  for (unsigned char c : s) {
    H ^= c;
    H *= 0x100000001b3ULL;
  }
}


static const unsigned LIVE_MODES[] = {0, 0xFF, 0x55, 0xAA, 0x0F, 0xF0};  // 0: no live-in set

template <typename DT, typename IDF>
static std::string lists(Function &F, std::vector<BasicBlock *> &bb, unsigned live, bool text, const char *dir) {
  DT T;
  T.recalculate(F);
  std::string all;
  auto num = [&](BasicBlock *B) -> int {
    for (unsigned i = 0; i < N; ++i)
      if (bb[i] == B) return (int)i;
    return -1;
  };
  for (unsigned def = 0; def < 256; ++def) {
    SmallPtrSet<BasicBlock *, 8> defs, liveIn;
    for (unsigned b = 0; b < N; ++b) {
      if (def >> b & 1) defs.insert(bb[b]);
      if (live >> b & 1) liveIn.insert(bb[b]);
    }
    IDF calc(T);
    calc.setDefiningBlocks(defs);
    if (live) calc.setLiveInBlocks(liveIn);
    SmallVector<BasicBlock *, 8> out;
    calc.calculate(out);
    std::string line;
    for (auto *B : out) line += std::to_string(num(B)) + ",";
    if (text)
      printf("list %s %02x %s : %s\n", dir, def, live ? "live" : "-", line.c_str());
    all += line + ";";
  }
  return all;
}

template <typename DT, typename IDF>
static void run(const char *dir, const Graph &g, const std::string &key, bool text) {
  LLVMContext C;
  Module M("m", C);
  Type *V = Type::getVoidTy(C);
  Function *F = Function::Create(FunctionType::get(V, {Type::getInt1Ty(C), Type::getInt32Ty(C)}, false),
                                 GlobalValue::ExternalLinkage, "f", M);
  std::vector<BasicBlock *> bb;
  for (unsigned b = 0; b < N; ++b) bb.push_back(BasicBlock::Create(C, "b" + std::to_string(b), F));
  setTerminators(*F, g, bb);
  for (unsigned live : LIVE_MODES) {
    std::string all = lists<DT, IDF>(*F, bb, live, text && live <= 0xFF && (live == 0 || live == 0x55), dir);
    mix(all);
    printf("hash %s %s %02x %016llx\n", dir, key.c_str(), live, (unsigned long long)H);
  }
}

int main() {
  static_assert(LLVM_VERSION_MAJOR == 24, "the pin is clang 24.0.0git");
  printf("# LLVM %s %s\n", LLVM_VERSION_STRING, LLVM_REVISION);
  Graph base_graph = base();
  run<DominatorTree, ForwardIDFCalculator>("fwd", base_graph, "base", true);
  run<PostDominatorTree, ReverseIDFCalculator>("rev", base_graph, "base", true);
  for (unsigned f = 0; f < N; ++f)
    for (unsigned t = 0; t < N; ++t) {
      bool insert;
      Graph g = flip(base_graph, {f, t}, insert);
      std::string key = std::to_string(f) + (insert ? "+" : "-") + std::to_string(t);
      run<DominatorTree, ForwardIDFCalculator>("fwd", g, key, false);
      run<PostDominatorTree, ReverseIDFCalculator>("rev", g, key, false);
    }
  return 0;
}

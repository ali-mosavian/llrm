// Writes the tree LLVM's DominatorTree / PostDominatorTree has after each edge update of a fixed 8-block graph: the golden
// file crates/ir/llrm-mir/src/domtree_golden.txt, which `domtree_tests.rs` recomputes from the Rust port.
//   tools/llvmport/domtree_dump.sh > crates/ir/llrm-mir/src/domtree_golden.txt
// A line is `<dom|post> <updates> <updated tree> <updated tree, children and roots sorted> <fresh tree of the new graph, sorted>`;
// the last two differ where LLVM's incremental update leaves a tree a fresh build would not (post-dominators of infinite loops).
#include "llvm/Config/llvm-config.h"
#include "llvm/Analysis/DomTreeUpdater.h"
#include "llvm/IR/BasicBlock.h"
#include "llvm/IR/Dominators.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/Module.h"
#include "llvm/Analysis/PostDominators.h"
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

static unsigned N = 8;

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
  for (unsigned b = 0; b < g.size(); ++b) {
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

static int num(BasicBlock *B, std::vector<BasicBlock *> &bb) {
  for (unsigned i = 0; i < bb.size(); ++i)
    if (bb[i] == B) return (int)i;
  return -1;
}

template <typename DT> static std::string dump(const DT &T, std::vector<BasicBlock *> &bb, bool sorted) {
  std::string out;
  auto num = [&](BasicBlock *B) -> int {
    for (unsigned i = 0; i < bb.size(); ++i)
      if (bb[i] == B) return (int)i;
    return -1;
  };
  auto line = [&](const char *name, BasicBlock *B) {
    auto *n = T.getNode(B);
    if (!n) {
      out += std::string(name) + ":x\n";
      return;
    }
    out += std::string(name) + ":";
    out += n->getIDom() && n->getIDom()->getBlock() ? std::to_string(num(n->getIDom()->getBlock())) : "-";
    out += ":" + std::to_string(n->getLevel()) + ":";
    std::vector<int> kids;
    for (auto *c : *n) kids.push_back(num(c->getBlock()));
    if (sorted) std::sort(kids.begin(), kids.end());
    for (int k : kids) out += std::to_string(k) + ",";
    out += "\n";
  };
  if (T.isPostDominator()) line("V", nullptr);
  for (unsigned b = 0; b < bb.size(); ++b) line(std::to_string(b).c_str(), bb[b]);
  out += "roots:";
  std::vector<int> roots;
  for (auto *r : T.roots()) roots.push_back(num(r));
  if (sorted) std::sort(roots.begin(), roots.end());
  for (int r : roots) out += std::to_string(r) + ",";
  out += "\n";
  return out;
}

template <typename DT> static void run(const char *name, const Graph &before, const std::vector<Edge> &edges) {
  LLVMContext C;
  Module M("m", C);
  Type *V = Type::getVoidTy(C);
  Function *F = Function::Create(FunctionType::get(V, {Type::getInt1Ty(C), Type::getInt32Ty(C)}, false),
                                 GlobalValue::ExternalLinkage, "f", M);
  std::vector<BasicBlock *> bb;
  for (unsigned b = 0; b < N; ++b) bb.push_back(BasicBlock::Create(C, "b" + std::to_string(b), F));
  setTerminators(*F, before, bb);
  DT T;
  T.recalculate(*F);
  Graph g = before;
  std::vector<typename DT::UpdateType> updates;
  std::string key;
  for (Edge e : edges) {
    bool ins;
    g = flip(g, e, ins);
    updates.push_back({ins ? DT::Insert : DT::Delete, bb[e.first], bb[e.second]});
    key += std::to_string(e.first) + (ins ? "+" : "-") + std::to_string(e.second) + ",";
  }
  setTerminators(*F, g, bb);
  T.applyUpdates(updates);
  mix(dump(T, bb, false));
  uint64_t inc = H;
  mix(dump(T, bb, true));
  uint64_t incSorted = H;
  DT Fresh;
  Fresh.recalculate(*F);
  mix(dump(Fresh, bb, true));
  uint64_t freshSorted = H;
  // The same updates through a Lazy DomTreeUpdater, one at a time, then asked for the tree; and through
  // applyUpdatesPermissive with the first update repeated.
  uint64_t lazy = 0, permissive = 0;
  for (int variant = 0; variant < 2; ++variant) {
    LLVMContext C2;
    Module M2("m", C2);
    Function *F2 = Function::Create(FunctionType::get(Type::getVoidTy(C2), {Type::getInt1Ty(C2), Type::getInt32Ty(C2)}, false),
                                    GlobalValue::ExternalLinkage, "f", M2);
    std::vector<BasicBlock *> bb2;
    for (unsigned b = 0; b < N; ++b) bb2.push_back(BasicBlock::Create(C2, "b" + std::to_string(b), F2));
    setTerminators(*F2, before, bb2);
    DT T2;
    T2.recalculate(*F2);
    setTerminators(*F2, g, bb2);
    std::vector<typename DT::UpdateType> ups;
    for (auto &u : updates) ups.push_back({u.getKind(), bb2[num(u.getFrom(), bb)], bb2[num(u.getTo(), bb)]});
    DomTreeUpdater DTU(T2, DomTreeUpdater::UpdateStrategy::Lazy);
    if (variant == 0) {
      for (auto &u : ups) DTU.applyUpdates({u});
    } else {
      ups.push_back(ups[0]);
      DTU.applyUpdatesPermissive(ups);
    }
    if (T2.isPostDominator()) DTU.getPostDomTree(); else DTU.getDomTree();
    mix(dump(T2, bb2, false));
    (variant == 0 ? lazy : permissive) = H;
  }
  printf("%s %s %016llx %016llx %016llx %016llx %016llx\n", name, key.c_str(), (unsigned long long)inc,
         (unsigned long long)incSorted, (unsigned long long)freshSorted, (unsigned long long)lazy,
         (unsigned long long)permissive);
}

// A critical edge split: the tree of the 8 blocks, then block 8 made, the edge retargeted through it and the updater told.
template <typename DT> static void runSplit(const char *name, const Graph &before, Edge e, bool lazy) {
  LLVMContext C;
  Module M("m", C);
  Function *F = Function::Create(FunctionType::get(Type::getVoidTy(C), {Type::getInt1Ty(C), Type::getInt32Ty(C)}, false),
                                 GlobalValue::ExternalLinkage, "f", M);
  std::vector<BasicBlock *> bb;
  for (unsigned b = 0; b < N; ++b) bb.push_back(BasicBlock::Create(C, "b" + std::to_string(b), F));
  setTerminators(*F, before, bb);
  DT T;
  T.recalculate(*F);
  bb.push_back(BasicBlock::Create(C, "b8", F));
  Graph g = before;
  for (auto &t : g[e.first])
    if (t == e.second) {
      t = N;
      break;
    }
  g.push_back({e.second});
  setTerminators(*F, g, bb);
  DomTreeUpdater DTU(T, lazy ? DomTreeUpdater::UpdateStrategy::Lazy : DomTreeUpdater::UpdateStrategy::Eager);
  DTU.splitCriticalEdge(bb[e.first], bb[e.second], bb[N]);
  if (T.isPostDominator()) DTU.getPostDomTree(); else DTU.getDomTree();
  mix(dump(T, bb, false));
  printf("split %s %u>%u %s %016llx\n", name, e.first, e.second, lazy ? "lazy" : "eager", (unsigned long long)H);
}

int main() {
  static_assert(LLVM_VERSION_MAJOR == 24, "the pin is clang 24.0.0git");
  printf("# LLVM %s %s\n", LLVM_VERSION_STRING, LLVM_REVISION);
  Graph g = base();
  std::vector<Edge> edges;
  for (unsigned f = 0; f < N; ++f)
    for (unsigned t = 0; t < N; ++t) edges.push_back({f, t});
  std::vector<std::vector<Edge>> batches;
  for (unsigned i = 0; i < edges.size(); ++i) batches.push_back({edges[i]});
  for (unsigned i = 0; i < edges.size(); ++i)
    for (unsigned j = i + 1; j < edges.size(); ++j) {
      batches.push_back({edges[i], edges[j]});
      if ((i + j) % 7 == 0) {
        Edge c = edges[(i * 5 + j * 3) % edges.size()];
        if (c != edges[i] && c != edges[j]) batches.push_back({edges[i], edges[j], c});
      }
    }
  for (auto &b : batches) {
    run<DominatorTree>("dom", g, b);
    run<PostDominatorTree>("post", g, b);
  }
  for (unsigned f = 0; f < N; ++f)
    for (unsigned t : g[f])
      for (int lazy = 0; lazy < 2; ++lazy) {
        if (f != 7) runSplit<DominatorTree>("dom", g, {f, t}, lazy);
        runSplit<PostDominatorTree>("post", g, {f, t}, lazy);
      }
  return 0;
}

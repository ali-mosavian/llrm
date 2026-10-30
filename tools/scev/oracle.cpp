// The pinned LLVM's ScalarEvolution as an oracle for llrm's: builds the
// expressions it is given with LLVM's own constructors and prints them.
//
//   oracle FILE.ll < expressions > results
//
// FILE.ll defines the function whose arguments are the unknowns `(u N)` and
// whose loops (in LoopInfo's preorder) are the loops `L` of `addrec`. One
// expression per input line, one result per output line: the expression as
// SCEV::print writes it, then ` : ` and its type, or `ERR` (a SCEV API
// assertion cannot be caught; the generator keeps to valid calls).
//
//   e ::= (c W V)                  constant of W bits, V decimal
//       | (u N)                    the Nth argument as an unknown
//       | (use FL e)               e with use flags FL
//       | (trunc|zext|sext|anyext W e)
//       | (add|mul FL e e ...)     FL = EXPR:USE, each - or nuw,nsw,nw
//       | (udiv|udivexact|urem e e) | (minus e e)
//       | (neg e) | (not e)
//       | (smax|umax|smin|umin|sequmin e e ...)
//       | (addrec FL L e e ...)
//       | (tozext|tosext|totrunc|noopzext|noopsext|noopanyext|toanyext W e)
#include "llvm/Analysis/LoopInfo.h"
#include "llvm/Analysis/ScalarEvolution.h"
#include "llvm/Analysis/ScalarEvolutionExpressions.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IRReader/IRReader.h"
#include "llvm/Passes/PassBuilder.h"
#include "llvm/Support/SourceMgr.h"
#include <iostream>
#include <sstream>
using namespace llvm;

struct Node {
  std::string atom;
  std::vector<Node> kids;
};

static Node parse(std::istringstream &in) {
  while (in.peek() == ' ') in.get();
  Node n;
  if (in.peek() == '(') {
    in.get();
    for (;;) {
      while (in.peek() == ' ') in.get();
      if (in.peek() == ')') { in.get(); break; }
      n.kids.push_back(parse(in));
    }
    return n;
  }
  while (in.peek() != EOF && in.peek() != ' ' && in.peek() != ')') n.atom += (char)in.get();
  return n;
}

struct Ctx {
  LLVMContext &C;
  ScalarEvolution &SE;
  Function &F;
  SmallVector<Loop *, 8> Loops;
};

static SCEV::NoWrapFlags flags(const std::string &s) {
  SCEV::NoWrapFlags F = SCEV::FlagNone;
  if (s == "-") return F;
  std::istringstream in(s);
  std::string t;
  while (std::getline(in, t, ',')) {
    if (t == "nw") F |= SCEV::FlagNW;
    else if (t == "nuw") F |= SCEV::FlagNUW;
    else if (t == "nsw") F |= SCEV::FlagNSW;
  }
  return F;
}

static SCEVFlags scevFlags(const std::string &s) {
  auto colon = s.find(':');
  return SCEVFlags(flags(s.substr(0, colon)), flags(s.substr(colon + 1)));
}

static SCEVUse eval(Ctx &X, const Node &n) {
  auto &SE = X.SE;
  const std::string &op = n.kids[0].atom;
  auto arg = [&](unsigned i) { return eval(X, n.kids[i]); };
  auto args = [&](unsigned from) {
    SmallVector<SCEVUse, 4> v;
    for (unsigned i = from; i < n.kids.size(); ++i) v.push_back(arg(i));
    return v;
  };
  auto ty = [&](unsigned i) { return Type::getIntNTy(X.C, std::stoul(n.kids[i].atom)); };
  if (op == "c") {
    unsigned W = std::stoul(n.kids[1].atom);
    APInt V(W, n.kids[2].atom, 10);
    return SE.getConstant(V);
  }
  if (op == "u") return SE.getUnknown(X.F.getArg(std::stoul(n.kids[1].atom)));
  if (op == "use") {
    // Use flags need an expression that can carry them; others pass through.
    SCEVUse E = arg(2);
    if (!isa<SCEVAddExpr, SCEVMulExpr, SCEVAddRecExpr>(E.getPointer())) return E;
    return SCEVUse(E.getPointer(), flags(n.kids[1].atom));
  }
  if (op == "trunc") return SE.getTruncateExpr(arg(2), ty(1));
  if (op == "zext") return SE.getZeroExtendExpr(arg(2), ty(1));
  if (op == "sext") return SE.getSignExtendExpr(arg(2), ty(1));
  if (op == "anyext") return SE.getAnyExtendExpr(arg(2), ty(1));
  if (op == "add") { auto v = args(2); return SE.getAddExpr(v, scevFlags(n.kids[1].atom)); }
  if (op == "mul") { auto v = args(2); return SE.getMulExpr(v, scevFlags(n.kids[1].atom)); }
  if (op == "udiv") return SE.getUDivExpr(arg(1), arg(2));
  if (op == "udivexact") return SE.getUDivExactExpr(arg(1), arg(2));
  if (op == "urem") return SE.getURemExpr(arg(1), arg(2));
  if (op == "minus") return SE.getMinusSCEV(arg(1), arg(2));
  if (op == "neg") return SE.getNegativeSCEV(arg(1).getPointer());
  if (op == "not") return SE.getNotSCEV(arg(1).getPointer());
  if (op == "smax") { auto v = args(1); return SE.getSMaxExpr(v); }
  if (op == "umax") { auto v = args(1); return SE.getUMaxExpr(v); }
  if (op == "smin") { auto v = args(1); return SE.getSMinExpr(v); }
  if (op == "umin") { auto v = args(1); return SE.getUMinExpr(v); }
  if (op == "sequmin") { auto v = args(1); return SE.getUMinExpr(v, true); }
  if (op == "addrec") {
    auto v = args(3);
    return SE.getAddRecExpr(v, X.Loops[std::stoul(n.kids[2].atom)], scevFlags(n.kids[1].atom));
  }
  if (op == "tozext") return SE.getTruncateOrZeroExtend(arg(2).getPointer(), ty(1));
  if (op == "tosext") return SE.getTruncateOrSignExtend(arg(2).getPointer(), ty(1));
  if (op == "totrunc") return SE.getTruncateOrNoop(arg(2).getPointer(), ty(1));
  if (op == "noopzext") return SE.getNoopOrZeroExtend(arg(2).getPointer(), ty(1));
  if (op == "noopsext") return SE.getNoopOrSignExtend(arg(2).getPointer(), ty(1));
  if (op == "noopanyext") return SE.getNoopOrAnyExtend(arg(2).getPointer(), ty(1));
  std::cerr << "unknown op " << op << "\n";
  std::exit(2);
}

int main(int argc, char **argv) {
  LLVMContext C;
  SMDiagnostic E;
  auto M = parseIRFile(argv[1], E, C);
  if (!M) { E.print("oracle", errs()); return 1; }
  PassBuilder PB;
  FunctionAnalysisManager FAM;
  LoopAnalysisManager LAM;
  CGSCCAnalysisManager CAM;
  ModuleAnalysisManager MAM;
  PB.registerModuleAnalyses(MAM);
  PB.registerCGSCCAnalyses(CAM);
  PB.registerFunctionAnalyses(FAM);
  PB.registerLoopAnalyses(LAM);
  PB.crossRegisterProxies(LAM, FAM, CAM, MAM);
  Function &F = *M->begin();
  auto &SE = FAM.getResult<ScalarEvolutionAnalysis>(F);
  auto &LI = FAM.getResult<LoopAnalysis>(F);
  Ctx X{C, SE, F, {}};
  for (Loop *L : LI.getLoopsInPreorder()) X.Loops.push_back(L);
  std::string line;
  while (std::getline(std::cin, line)) {
    std::istringstream in(line);
    Node n = parse(in);
    SCEVUse R = eval(X, n);
    std::string out;
    raw_string_ostream os(out);
    R.print(os);
    os << " : " << *R->getType();
    std::cout << os.str() << "\n";
  }
}

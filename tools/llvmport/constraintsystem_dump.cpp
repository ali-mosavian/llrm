// Writes one hash per family of systems for LLVM's ConstraintSystem: the golden file
// crates/ir/llrm-mir/src/constraintsystem_golden.txt, which `constraintsystem_tests.rs` recomputes.
//   tools/llvmport/constraintsystem_dump.sh > crates/ir/llrm-mir/src/constraintsystem_golden.txt
// A row is (k, c1..cn): k >= c1*v1 + ... + cn*vn. A system is added row by row with addRow; each system is asked mayHaveSolution
// (on a copy), and each query row isConditionImplied and isConditionImpliedInSubSystem. Queries naming a variable the system does
// not have (NumVariables is set by the first accepted row) are skipped and marked.
//   A1 i       systems of row i of 108 (k in -1,0,1,2; three variables with coefficients -1,0,1), queries: all 108
//   A2 i j     systems of rows i, j; the same queries
//   B3 i j     systems of rows i, j and every l of 36 (two variables), queries: all 36
//   G n u      n variables, a row 1 >= xi - xj for every ordered pair (and, for u = 1, -2 >= x1 - x2): Fourier-Motzkin grows past 500
//              rows and gives up for the larger n; the answer and the rows left are hashed
//   O  i j     two-variable rows with k and coefficients in {MIN+1, -1, 1, MAX}: 64 rows, systems of i, j, queries all 64
#include "llvm/Analysis/ConstraintSystem.h"
#include "llvm/Config/llvm-config.h"
#include "llvm/Support/VCSRevision.h"
#include <cstdint>
#include <cstdio>
#include <algorithm>
#include <limits>
#include <vector>

using namespace llvm;
using Row = ConstraintSystem::RowTy;

static uint64_t H;
static void reset() { H = 0xcbf29ce484222325ULL; }
static void mix(unsigned char c) {
  H ^= c;
  H *= 0x100000001b3ULL;
}

struct Spec {
  unsigned vars;
  std::vector<int64_t> ks, cs;
  std::vector<Row> rows;
};

static Row makeRow(int64_t k, const std::vector<int64_t> &c) {
  Row R;
  R.emplace_back(k, 0);
  for (unsigned i = 0; i < c.size(); ++i)
    if (c[i] != 0) R.emplace_back(c[i], i + 1);
  return R;
}

static Spec spec(unsigned vars, std::vector<int64_t> ks, std::vector<int64_t> cs) {
  Spec S{vars, ks, cs, {}};
  unsigned total = ks.size();
  for (unsigned i = 0; i < vars; ++i) total *= cs.size();
  for (unsigned r = 0; r < total; ++r) {
    unsigned x = r;
    int64_t k = ks[x % ks.size()];
    x /= ks.size();
    std::vector<int64_t> c;
    for (unsigned i = 0; i < vars; ++i) {
      c.push_back(cs[x % cs.size()]);
      x /= cs.size();
    }
    S.rows.push_back(makeRow(k, c));
  }
  return S;
}

// Mixes the answers of the system made of `ids` over the spec's rows.
static void answer(const Spec &S, const std::vector<unsigned> &ids) {
  ConstraintSystem CS;
  unsigned numVars = 0;
  for (unsigned id : ids)
    if (CS.addRow(S.rows[id], S.vars)) numVars = S.vars;
  mix(CS.size());
  ConstraintSystem Copy = CS;
  mix(Copy.mayHaveSolution());
  for (const Row &Q : S.rows) {
    if (Q.back().Id > numVars) {
      mix('-');
      continue;
    }
    mix(CS.isConditionImplied(Q));
    mix(CS.isConditionImpliedInSubSystem(Q));
  }
}

int main() {
  static_assert(LLVM_VERSION_MAJOR == 24, "the pin is clang 24.0.0git");
  printf("# LLVM %s %s\n", LLVM_VERSION_STRING, LLVM_REVISION);
  Spec A = spec(3, {-1, 0, 1, 2}, {-1, 0, 1});
  for (unsigned i = 0; i < A.rows.size(); ++i) {
    reset();
    answer(A, {i});
    printf("A1 %u %016llx\n", i, (unsigned long long)H);
  }
  for (unsigned i = 0; i < A.rows.size(); ++i)
    for (unsigned j = 0; j < A.rows.size(); ++j) {
      reset();
      answer(A, {i, j});
      printf("A2 %u %u %016llx\n", i, j, (unsigned long long)H);
    }
  Spec B = spec(2, {-1, 0, 1, 2}, {-1, 0, 1});
  for (unsigned i = 0; i < B.rows.size(); ++i)
    for (unsigned j = 0; j < B.rows.size(); ++j) {
      reset();
      for (unsigned l = 0; l < B.rows.size(); ++l) answer(B, {i, j, l});
      printf("B3 %u %u %016llx\n", i, j, (unsigned long long)H);
    }
  const int64_t mx = std::numeric_limits<int64_t>::max(), mn = std::numeric_limits<int64_t>::min() + 1;
  Spec O = spec(2, {mn, -1, 1, mx}, {mn, -1, 1, mx});
  for (unsigned i = 0; i < O.rows.size(); ++i)
    for (unsigned j = 0; j < O.rows.size(); ++j) {
      reset();
      answer(O, {i, j});
      printf("O %u %u %016llx\n", i, j, (unsigned long long)H);
    }
  for (unsigned n = 3; n <= 12; ++n)
    for (unsigned u = 0; u < 2; ++u) {
      reset();
      ConstraintSystem CS;
      for (unsigned i = 1; i <= n; ++i)
        for (unsigned j = 1; j <= n; ++j)
          if (i != j) {
            Row R;
            R.emplace_back(1, 0);
            Row V;
            V.emplace_back(1, std::min(i, j));
            V.emplace_back(-1, std::max(i, j));
            // xi - xj as entries in ascending variable order
            R.emplace_back(i < j ? 1 : -1, std::min(i, j));
            R.emplace_back(i < j ? -1 : 1, std::max(i, j));
            CS.addRow(R, n);
          }
      if (u) {
        Row R;
        R.emplace_back(-2, 0);
        R.emplace_back(1, 1);
        R.emplace_back(-1, 2);
        CS.addRow(R, n);
      }
      mix(CS.size());
      mix(CS.mayHaveSolution());
      mix(CS.size() > 255 ? 255 : CS.size());
      printf("G %u %u %016llx\n", n, u, (unsigned long long)H);
    }
  return 0;
}

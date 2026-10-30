// Writes one hash per ConstantFPRange operation over every range of the two 8-bit formats (unary operations) and the
// ranges between 18 key values of every format (binary ones): the golden file
// crates/ir/llrm-mir/src/constantfprange_golden.txt, which `constantfprange_tests.rs` recomputes.
// A line is `operation format hash distinct`: the hash of every answer, and how many different answers there were.
//   tools/llvmport/constantfprange_dump.sh > crates/ir/llrm-mir/src/constantfprange_golden.txt
#include "llvm/Config/llvm-config.h"
#include "llvm/ADT/APFloat.h"
#include "llvm/ADT/FloatingPointMode.h"
#include "llvm/IR/ConstantFPRange.h"
#include <cstdint>
#include <cstdio>
#include <set>
#include <string>
#include <vector>

#include "fpinputs.h"

typedef ConstantFPRange R;
typedef FCmpInst::Predicate P;

static uint64_t H, C;
static std::set<uint64_t> D;
static void mix(uint64_t v) {
  for (int i = 0; i < 8; ++i) {
    H ^= (v >> (8 * i)) & 0xff;
    H *= 0x100000001b3ULL;
  }
}
static void hbegin() { C = 0xcbf29ce484222325ULL; }
static void hadd(uint64_t v) {
  for (int i = 0; i < 8; ++i) {
    C ^= (v >> (8 * i)) & 0xff;
    C *= 0x100000001b3ULL;
  }
}
static void hend() {
  D.insert(C);
  mix(C);
}
static void line(const std::string &name, const char *fmt) {
  printf("%s %s %016llx %zu\n", name.c_str(), fmt, (unsigned long long)H, D.size());
  H = 0xcbf29ce484222325ULL;
  D.clear();
}
static void addF(const APFloat &f) {
  APInt b = f.bitcastToAPInt();
  unsigned w = b.getBitWidth();
  hadd(b.extractBitsAsZExtValue(w < 64 ? w : 64, 0));
  hadd(w > 64 ? b.extractBitsAsZExtValue(w - 64, 64) : 0);
}
static void addR(const R &r) {
  addF(r.getLower());
  addF(r.getUpper());
  hadd(r.containsQNaN());
  hadd(r.containsSNaN());
}
static void addOptR(const std::optional<R> &r) {
  hadd(r.has_value());
  if (r) addR(*r);
}

// The same strict order as ConstantFPRange.cpp: -0 is below +0.
static APFloat::cmpResult strictCompare(const APFloat &L, const APFloat &R_) {
  if (L.isZero() && R_.isZero()) {
    if (L.isNegative() == R_.isNegative()) return APFloat::cmpEqual;
    return L.isNegative() ? APFloat::cmpLessThan : APFloat::cmpGreaterThan;
  }
  return L.compare(R_);
}

// Every range whose bounds come from `values` (non-NaN), with the four NaN flags: lower <= upper, then the canonical
// empty range [+inf, -inf].
static std::vector<R> rangesOf(const Fmt &f, const std::vector<APFloat> &values) {
  std::vector<R> v;
  for (int flags = 0; flags < 4; ++flags) {
    bool q = flags & 1, s = flags & 2;
    for (const APFloat &lo : values)
      for (const APFloat &hi : values)
        if (strictCompare(lo, hi) != APFloat::cmpGreaterThan) v.push_back(R(lo, hi, q, s));
    v.push_back(R(APFloat::getInf(*f.sem, false), APFloat::getInf(*f.sem, true), q, s));
  }
  return v;
}

static const P PREDS[16] = {FCmpInst::FCMP_FALSE, FCmpInst::FCMP_OEQ, FCmpInst::FCMP_OGT, FCmpInst::FCMP_OGE,
                            FCmpInst::FCMP_OLT,   FCmpInst::FCMP_OLE, FCmpInst::FCMP_ONE, FCmpInst::FCMP_ORD,
                            FCmpInst::FCMP_UNO,   FCmpInst::FCMP_UEQ, FCmpInst::FCMP_UGT, FCmpInst::FCMP_UGE,
                            FCmpInst::FCMP_ULT,   FCmpInst::FCMP_ULE, FCmpInst::FCMP_UNE, FCmpInst::FCMP_TRUE};

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s\n", LLVM_VERSION_STRING);
    return 0;
  }
  H = 0xcbf29ce484222325ULL;
  const std::vector<Fmt> fmts = formats();
  const RoundingMode RMS[5] = {RoundingMode::TowardZero, RoundingMode::NearestTiesToEven, RoundingMode::TowardPositive,
                               RoundingMode::TowardNegative, RoundingMode::NearestTiesToAway};
  const DenormalMode::DenormalModeKind KINDS[4] = {DenormalMode::IEEE, DenormalMode::PreserveSign,
                                                   DenormalMode::PositiveZero, DenormalMode::Dynamic};

  for (const Fmt &f : fmts) {
    const char *n = f.name;
    const fltSemantics &S = *f.sem;
    // Every non-NaN value (tiny formats), or the key values, as the bounds of the ranges the unary operations run on.
    std::vector<APFloat> keys, bounds;
    for (u128 b : keyPatterns(f)) keys.push_back(make(f, b));
    if (f.tiny) {
      for (u128 b : inputs(f)) {
        APFloat x = make(f, b);
        if (!x.isNaN()) bounds.push_back(x);
      }
    } else {
      bounds = keys;
    }
    const std::vector<R> unaryRanges = rangesOf(f, bounds), keyRanges = rangesOf(f, keys);
    std::vector<APFloat> probes = keys;
    for (int neg = 0; neg < 2; ++neg) {
      probes.push_back(APFloat::getQNaN(S, neg));
      probes.push_back(APFloat::getSNaN(S, neg));
    }

    // Constants.
    hbegin();
    for (int a = 0; a < 2; ++a)
      for (int b = 0; b < 2; ++b) addR(R::getNaNOnly(S, a, b));
    addR(R::getEmpty(S)); addR(R::getFull(S)); addR(R::getFinite(S)); addR(R::getNonNaN(S));
    hend();
    line("constants", n);
    for (u128 b : inputs(f)) {
      hbegin();
      addR(R(make(f, b)));
      hend();
    }
    line("fromValue", n);

    auto un = [&](const char *name, auto fn) {
      for (const R &r : unaryRanges) {
        hbegin();
        fn(r);
        hend();
      }
      line(name, n);
    };
    un("queries", [&](const R &r) {
      hadd(r.isFullSet()); hadd(r.isEmptySet()); hadd(r.isNaNOnly()); hadd(r.containsNaN());
      hadd(r.isSingleElement(false)); hadd(r.isSingleElement(true));
      for (bool ex : {false, true}) {
        const APFloat *e = r.getSingleElement(ex);
        hadd(e != nullptr);
        if (e) addF(*e);
      }
      std::optional<bool> sb = r.getSignBit();
      hadd(sb ? 1 + *sb : 0);
      hadd(r.classify());
      addF(r.getLower()); addF(r.getUpper());
    });
    un("contains", [&](const R &r) {
      for (const APFloat &p : probes) hadd(r.contains(p));
    });
    un("abs", [](const R &r) { addR(r.abs()); });
    un("negate", [](const R &r) { addR(r.negate()); });
    un("getWithoutNaN", [](const R &r) { addR(r.getWithoutNaN()); });
    un("getWithoutInf", [](const R &r) { addR(r.getWithoutInf()); });
    un("flushDenormals", [&](const R &r) {
      for (auto k : KINDS) {
        R t = r;
        t.flushDenormals(k);
        addR(t);
      }
    });
    for (const Fmt &g : fmts) {
      if (&g == &f) continue;
      un((std::string("cast_") + g.name).c_str(), [&](const R &r) {
        for (RoundingMode rm : RMS) addR(r.cast(*g.sem, rm));
      });
    }

    // Predicates against a range, over every range of the 8-bit formats.
    for (int i = 0; i < 16; ++i) {
      P pred = PREDS[i];
      un(("makeAllowed" + std::to_string(i)).c_str(), [&](const R &r) { addR(R::makeAllowedFCmpRegion(pred, r)); });
      un(("makeSatisfying" + std::to_string(i)).c_str(), [&](const R &r) { addR(R::makeSatisfyingFCmpRegion(pred, r)); });
    }
    {
      std::vector<u128> vals = inputs(f);
      for (int i = 0; i < 16; ++i) {
        for (u128 b : vals) {
          hbegin();
          addOptR(R::makeExactFCmpRegion(PREDS[i], make(f, b)));
          hend();
        }
        line("makeExact" + std::to_string(i), n);
      }
    }

    // Binary operations over the ranges between the key values.
    auto bin = [&](const std::string &name, auto fn) {
      for (const R &a : keyRanges)
        for (const R &b : keyRanges) {
          hbegin();
          fn(a, b);
          hend();
        }
      line(name, n);
    };
    bin("intersectWith", [](const R &a, const R &b) { addR(a.intersectWith(b)); });
    bin("unionWith", [](const R &a, const R &b) { addR(a.unionWith(b)); });
    bin("containsRange", [](const R &a, const R &b) { hadd(a.contains(b)); hadd(a == b); hadd(a != b); });
    bin("add", [](const R &a, const R &b) { addR(a.add(b)); });
    bin("sub", [](const R &a, const R &b) { addR(a.sub(b)); });
    bin("mul", [](const R &a, const R &b) { addR(a.mul(b)); });
    bin("div", [](const R &a, const R &b) { addR(a.div(b)); });
    bin("fcmp", [](const R &a, const R &b) {
      for (P p : PREDS) hadd(a.fcmp(p, b));
    });
  }
  return 0;
}

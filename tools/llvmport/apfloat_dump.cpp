// Writes one hash per APFloat operation over every value of two 8-bit formats and a sample of float, double and
// x87: the golden file crates/ir/llrm-mir/src/apfloat_golden.txt, which `apfloat_tests.rs` recomputes.
// A line is `operation format hash distinct`: the hash of every answer, and how many different answers there were.
// Build against the LLVM of d1106deb71cc:
//   tools/llvmport/apfloat_dump.sh > crates/ir/llrm-mir/src/apfloat_golden.txt
#include "llvm/Config/llvm-config.h"
#include "llvm/ADT/APFloat.h"
#include "llvm/ADT/APInt.h"
#include "llvm/ADT/ArrayRef.h"
#include <cstdint>
#include <cstdio>
#include <set>
#include <string>
#include <vector>

using namespace llvm;
typedef unsigned __int128 u128;

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
static void addF(const APFloat &f) {
  APInt b = f.bitcastToAPInt();
  unsigned w = b.getBitWidth();
  hadd(b.extractBitsAsZExtValue(w < 64 ? w : 64, 0));
  hadd(w > 64 ? b.extractBitsAsZExtValue(w - 64, 64) : 0);
}
static void line(const char *name, const char *fmt) {
  printf("%s %s %016llx %zu\n", name, fmt, (unsigned long long)H, D.size());
  H = 0xcbf29ce484222325ULL;
  D.clear();
}

struct Fmt {
  const char *name;
  const fltSemantics *sem;
  unsigned eb, fb; // exponent bits, stored fraction bits (without the explicit integer bit)
  bool expl;
  unsigned size;
  bool tiny;
};

static APFloat make(const Fmt &f, u128 bits) {
  if (f.size <= 64) return APFloat(*f.sem, APInt(f.size, (uint64_t)bits));
  uint64_t w[2] = {(uint64_t)bits, (uint64_t)(bits >> 64)};
  return APFloat(*f.sem, APInt(f.size, ArrayRef<uint64_t>(w, 2)));
}

static u128 ones(unsigned n) { return n >= 128 ? ~(u128)0 : (((u128)1 << n) - 1); }

static u128 pattern(const Fmt &f, unsigned sign, u128 exp, u128 frac, unsigned intbit) {
  unsigned trailing = f.fb + (f.expl ? 1 : 0);
  return ((u128)sign << (f.size - 1)) | (exp << trailing) | ((u128)intbit << f.fb) | frac;
}

static std::vector<u128> inputs(const Fmt &f) {
  std::vector<u128> v;
  if (f.tiny) {
    for (unsigned i = 0; i < 256; ++i) v.push_back(i);
    return v;
  }
  u128 bias = ((u128)1 << (f.eb - 1)) - 1, top = ones(f.eb), fr = ones(f.fb);
  unsigned x = f.expl ? 1 : 0;
  for (unsigned s = 0; s < 2; ++s) {
    v.push_back(pattern(f, s, 0, 0, 0));
    v.push_back(pattern(f, s, 0, 1, 0));
    v.push_back(pattern(f, s, 0, fr, 0));
    v.push_back(pattern(f, s, 0, (u128)1 << (f.fb - 1), 0));
    v.push_back(pattern(f, s, 1, 0, x));
    v.push_back(pattern(f, s, 1, 1, x));
    v.push_back(pattern(f, s, bias - 1, (u128)1 << (f.fb - 1), x));
    v.push_back(pattern(f, s, bias, 0, x));
    v.push_back(pattern(f, s, bias, 1, x));
    v.push_back(pattern(f, s, bias, (u128)1 << (f.fb - 1), x));
    v.push_back(pattern(f, s, bias + 1, 0, x));
    v.push_back(pattern(f, s, bias + 1, (u128)1 << (f.fb - 1), x));
    v.push_back(pattern(f, s, bias + f.fb - 1, 1, x));
    v.push_back(pattern(f, s, bias + f.fb, 0, x));
    v.push_back(pattern(f, s, bias + f.fb + 1, 0, x));
    v.push_back(pattern(f, s, top - 1, fr - 1, x));
    v.push_back(pattern(f, s, top - 1, fr, x));
    v.push_back(pattern(f, s, top - 1, 0, x));
    v.push_back(pattern(f, s, top, 0, x));
    v.push_back(pattern(f, s, top, (u128)1 << (f.fb - 1), x));
    v.push_back(pattern(f, s, top, ((u128)1 << (f.fb - 1)) | 1, x));
    v.push_back(pattern(f, s, top, 1, x));
    v.push_back(pattern(f, s, top, (u128)1 << (f.fb - 2), x));
    v.push_back(pattern(f, s, top, fr, x));
    if (f.expl) {
      v.push_back(pattern(f, s, 0, 0, 1));
      v.push_back(pattern(f, s, 0, 1, 1));
      v.push_back(pattern(f, s, 1, 5, 0));
      v.push_back(pattern(f, s, bias, 0, 0));
      v.push_back(pattern(f, s, top, 0, 0));
      v.push_back(pattern(f, s, top, 3, 0));
    }
  }
  uint64_t st = 0x9e3779b97f4a7c15ULL ^ f.size;
  auto next = [&] { st = st * 6364136223846793005ULL + 1442695040888963407ULL; return st >> 33; };
  for (int i = 0; i < 120; ++i) {
    unsigned s = next() & 1;
    u128 e;
    if (next() & 1) {
      int64_t d = (int64_t)(next() % 25) - 12;
      int64_t c = (int64_t)bias + d;
      e = c < 0 ? 0 : (u128)c > top ? top : (u128)c;
    } else {
      e = (u128)next() % ((u128)top + 1);
    }
    u128 w1 = next(), w2 = next(), w3 = next();
    u128 frac = ((((w1 << 32) ^ w2) << 32) ^ w3) & fr;
    if (next() % 4 == 0) frac &= ~ones(f.fb / 2);
    unsigned ib = f.expl ? (e != 0) : 0;
    if (f.expl && next() % 16 == 0) ib ^= 1;
    v.push_back(pattern(f, s, e, frac, ib));
  }
  return v;
}

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s\n", LLVM_VERSION_STRING);
    return 0;
  }
  H = 0xcbf29ce484222325ULL;
  std::vector<Fmt> fmts = {
      {"e5m2", &APFloat::Float8E5M2(), 5, 2, false, 8, true},
      {"e4m3", &APFloat::Float8E4M3(), 4, 3, false, 8, true},
      {"single", &APFloat::IEEEsingle(), 8, 23, false, 32, false},
      {"double", &APFloat::IEEEdouble(), 11, 52, false, 64, false},
      {"x87", &APFloat::x87DoubleExtended(), 15, 63, true, 80, false},
  };
  const RoundingMode RMS[5] = {RoundingMode::TowardZero, RoundingMode::NearestTiesToEven, RoundingMode::TowardPositive,
                               RoundingMode::TowardNegative, RoundingMode::NearestTiesToAway};

  for (const Fmt &f : fmts) {
    std::vector<u128> in = inputs(f);
    std::vector<APFloat> v;
    for (u128 b : in) v.push_back(make(f, b));
    const char *n = f.name;
    auto each = [&](const char *name, auto fn) {
      for (const APFloat &x : v) {
        hbegin();
        if (fn(x)) hend();
      }
      line(name, n);
    };
    auto pairs = [&](const char *name, auto fn) {
      for (const APFloat &x : v)
        for (const APFloat &y : v) {
          hbegin();
          if (fn(x, y)) hend();
        }
      line(name, n);
    };

    each("inputs", [&](const APFloat &x) { hadd(in[&x - &v[0]] & ~(uint64_t)0); hadd((uint64_t)(in[&x - &v[0]] >> 64)); return true; });
    each("bits", [&](const APFloat &x) { addF(x); return true; });
    each("flags", [&](const APFloat &x) {
      hadd(x.isNaN()); hadd(x.isInfinity()); hadd(x.isZero()); hadd(x.isNegative()); hadd(x.isDenormal());
      hadd(x.isNormal()); hadd(x.isSmallest()); hadd(x.isSmallestNormalized()); hadd(x.isLargest()); hadd(x.isFinite());
      hadd(x.isFiniteNonZero()); hadd(x.isSignaling()); hadd(x.isInteger()); hadd(x.isNonZero()); hadd(x.isPosZero());
      hadd(x.isNegZero()); hadd(x.isPosInfinity()); hadd(x.isNegInfinity()); hadd(x.getCategory());
      return true;
    });
    each("classify", [&](const APFloat &x) { hadd(x.classify()); return true; });
    each("ilogb", [&](const APFloat &x) {
      hadd((uint64_t)(int64_t)ilogb(x)); hadd((uint64_t)(int64_t)x.getExactLog2Abs());
      hadd((uint64_t)(int64_t)x.getExactLog2());
      for (int k = -3; k < 4; ++k) { hadd(x.isPowerOf2(k)); hadd(x.isNegPowerOf2(k)); }
      return true;
    });
    each("nextUp", [&](const APFloat &x) { APFloat y = x; hadd(y.next(false)); addF(y); return true; });
    each("nextDown", [&](const APFloat &x) { APFloat y = x; hadd(y.next(true)); addF(y); return true; });
    each("neg", [&](const APFloat &x) { addF(-x); return true; });
    each("abs", [&](const APFloat &x) { addF(abs(x)); return true; });
    each("makeQuiet", [&](const APFloat &x) { if (!x.isNaN()) return false; addF(x.makeQuiet()); return true; });
    for (int r = 0; r < 5; ++r) {
      std::string name = "roundToIntegral" + std::to_string(r);
      each(name.c_str(), [&](const APFloat &x) { APFloat y = x; hadd(y.roundToIntegral(RMS[r])); addF(y); return true; });
    }
    for (const Fmt &g : fmts) {
      if (&g == &f) continue;
      for (int r = 0; r < 5; ++r) {
        std::string name = std::string("convert_") + g.name + "_" + std::to_string(r);
        each(name.c_str(), [&](const APFloat &x) {
          APFloat y = x;
          bool loses = false;
          hadd(y.convert(*g.sem, RMS[r], &loses));
          hadd(loses);
          addF(y);
          return true;
        });
      }
    }

    // Constants and the queries on the semantics.
    hbegin();
    const fltSemantics &S = *f.sem;
    for (int neg = 0; neg < 2; ++neg) {
      addF(APFloat::getZero(S, neg)); addF(APFloat::getOne(S, neg)); addF(APFloat::getInf(S, neg));
      addF(APFloat::getLargest(S, neg)); addF(APFloat::getSmallest(S, neg)); addF(APFloat::getSmallestNormalized(S, neg));
      for (uint64_t p : {0ULL, 1ULL, 5ULL, 0x123456789abcdefULL, ~0ULL}) addF(APFloat::getNaN(S, neg, p));
      for (uint64_t p : {0ULL, 1ULL, 2ULL, 0x1234ULL, ~0ULL}) {
        APInt payload(64, p);
        addF(APFloat::getQNaN(S, neg, &payload));
        addF(APFloat::getSNaN(S, neg, &payload));
      }
      addF(APFloat::getQNaN(S, neg, nullptr));
      addF(APFloat::getSNaN(S, neg, nullptr));
    }
    hend();
    line("constants", n);
    hbegin();
    hadd(APFloat::semanticsPrecision(S)); hadd((uint64_t)(int64_t)APFloat::semanticsMaxExponent(S));
    hadd((uint64_t)(int64_t)APFloat::semanticsMinExponent(S)); hadd(APFloat::semanticsSizeInBits(S));
    hadd(APFloat::getSizeInBits(S)); hadd(APFloat::isIEEELikeFP(S)); hadd(APFloat::semanticsHasInf(S));
    hadd(APFloat::semanticsHasNaN(S)); hadd(APFloat::semanticsHasSignedRepr(S)); hadd(APFloat::semanticsHasZero(S));
    for (const Fmt &g : fmts) hadd(APFloat::isRepresentableAsNormalIn(S, *g.sem));
    hend();
    line("semantics", n);

    pairs("compare", [&](const APFloat &x, const APFloat &y) { hadd(x.compare(y)); return true; });
    pairs("compareAbs", [&](const APFloat &x, const APFloat &y) {
      if (!x.isFiniteNonZero() || !y.isFiniteNonZero()) return false;
      hadd(x.compareAbsoluteValue(y));
      return true;
    });
    pairs("bitwiseIsEqual", [&](const APFloat &x, const APFloat &y) { hadd(x.bitwiseIsEqual(y)); return true; });
    for (int r = 0; r < 5; ++r) {
      auto arith = [&](const char *op, auto fn) {
        std::string name = std::string(op) + std::to_string(r);
        pairs(name.c_str(), [&](const APFloat &x, const APFloat &y) {
          APFloat z = x;
          hadd(fn(z, y, RMS[r]));
          addF(z);
          return true;
        });
      };
      arith("add", [](APFloat &a, const APFloat &b, RoundingMode m) { return a.add(b, m); });
      arith("sub", [](APFloat &a, const APFloat &b, RoundingMode m) { return a.subtract(b, m); });
      arith("mul", [](APFloat &a, const APFloat &b, RoundingMode m) { return a.multiply(b, m); });
      arith("div", [](APFloat &a, const APFloat &b, RoundingMode m) { return a.divide(b, m); });
    }
    pairs("plus", [&](const APFloat &x, const APFloat &y) { addF(x + y); return true; });
    pairs("minus", [&](const APFloat &x, const APFloat &y) { addF(x - y); return true; });
    pairs("times", [&](const APFloat &x, const APFloat &y) { addF(x * y); return true; });
    pairs("dividedBy", [&](const APFloat &x, const APFloat &y) { addF(x / y); return true; });
    pairs("minnum", [&](const APFloat &x, const APFloat &y) { addF(minnum(x, y)); return true; });
    pairs("maxnum", [&](const APFloat &x, const APFloat &y) { addF(maxnum(x, y)); return true; });
    pairs("minimum", [&](const APFloat &x, const APFloat &y) { addF(minimum(x, y)); return true; });
    pairs("maximum", [&](const APFloat &x, const APFloat &y) { addF(maximum(x, y)); return true; });
    pairs("minimumnum", [&](const APFloat &x, const APFloat &y) { addF(minimumnum(x, y)); return true; });
    pairs("maximumnum", [&](const APFloat &x, const APFloat &y) { addF(maximumnum(x, y)); return true; });
    pairs("copySign", [&](const APFloat &x, const APFloat &y) { addF(APFloat::copySign(x, y)); return true; });
  }
  return 0;
}

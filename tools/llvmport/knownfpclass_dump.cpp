// Writes one hash per KnownFPClass operation over every class mask, the three sign states, eight denormal modes and
// a sample of values of every format: the golden file crates/ir/llrm-mir/src/knownfpclass_golden.txt, which
// `knownfpclass_tests.rs` recomputes.
// A line is `operation hash distinct`: the hash of every answer, and how many different answers there were.
//   tools/llvmport/knownfpclass_dump.sh > crates/ir/llrm-mir/src/knownfpclass_golden.txt
#include "llvm/Config/llvm-config.h"
#include "llvm/ADT/APFloat.h"
#include "llvm/ADT/FloatingPointMode.h"
#include "llvm/Support/KnownBits.h"
#include "llvm/Support/KnownFPClass.h"
#include <cstdint>
#include <cstdio>
#include <set>
#include <string>
#include <vector>

#include "fpinputs.h"

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
static void line(const std::string &name) {
  printf("%s %016llx %zu\n", name.c_str(), (unsigned long long)H, D.size());
  H = 0xcbf29ce484222325ULL;
  D.clear();
}
static void addK(const KnownFPClass &k) {
  hadd(k.getKnownFPClasses());
  hadd(k.getSignBit() ? 1 + *k.getSignBit() : 0);
}
static void addBits(const KnownBits &k) {
  unsigned w = k.getBitWidth();
  hadd(w);
  for (const APInt *p : {&k.Zero, &k.One}) {
    hadd(p->extractBitsAsZExtValue(w < 64 ? w : 64, 0));
    hadd(w > 64 ? p->extractBitsAsZExtValue(w - 64, 64) : 0);
  }
}

typedef std::vector<KnownFPClass> States;
typedef DenormalMode Mode;

static States allStates() {
  States s;
  for (unsigned m = 0; m < 1024; ++m)
    for (int sg = 0; sg < 3; ++sg)
      s.push_back(KnownFPClass(static_cast<FPClassTest>(m), sg == 0 ? std::optional<bool>() : std::optional<bool>(sg == 2)));
  return s;
}
static States fullStates() {
  States s;
  for (unsigned m = 0; m < 1024; ++m) s.push_back(KnownFPClass(static_cast<FPClassTest>(m)));
  return s;
}
static std::vector<unsigned> curatedMasks() {
  std::vector<unsigned> v = {0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, fcNan, fcInf, fcNormal, fcSubnormal, fcZero,
                             fcPosFinite, fcNegFinite, fcFinite, fcPositive, fcNegative, fcAllFlags,
                             fcAllFlags & ~fcNan, fcAllFlags & ~fcZero, fcAllFlags & ~fcInf, fcZero | fcNan,
                             fcPosZero | fcPosNormal, fcNegZero | fcNegNormal};
  uint64_t st = 0x2545f4914f6cdd1dULL;
  for (int i = 0; i < 24; ++i) {
    st = st * 6364136223846793005ULL + 1442695040888963407ULL;
    v.push_back((st >> 33) & 0x3ff);
  }
  return v;
}
static States curatedStates(bool signs) {
  States s;
  for (unsigned m : curatedMasks())
    for (int sg = 0; sg < (signs ? 3 : 1); ++sg)
      s.push_back(KnownFPClass(static_cast<FPClassTest>(m), sg == 0 ? std::optional<bool>() : std::optional<bool>(sg == 2)));
  return s;
}
static std::vector<Mode> allModes() {
  using K = DenormalMode;
  return {K(K::IEEE, K::IEEE),
          K(K::PreserveSign, K::PreserveSign),
          K(K::PositiveZero, K::PositiveZero),
          K(K::Dynamic, K::Dynamic),
          K(K::IEEE, K::PreserveSign),
          K(K::PositiveZero, K::IEEE),
          K(K::Dynamic, K::IEEE),
          K(K::PreserveSign, K::Dynamic)};
}
static std::vector<Mode> twoModes() { return {DenormalMode::getIEEE(), DenormalMode::getDynamic()}; }

// The known bits of a pattern: the bits of `mask` are known.
static KnownBits knownOf(unsigned w, u128 v, u128 mask) {
  KnownBits k(w);
  auto ap = [&](u128 x) {
    if (w <= 64) return APInt(w, (uint64_t)x);
    uint64_t t[2] = {(uint64_t)x, (uint64_t)(x >> 64)};
    return APInt(w, ArrayRef<uint64_t>(t, 2));
  };
  k.Zero = ap(~v & mask);
  k.One = ap(v & mask);
  return k;
}


// Known bits of an exponent: every state of a width up to 5, else a fixed sample.
static unsigned expCount(unsigned w) {
  if (w > 5) return 300;
  unsigned n = 1;
  for (unsigned i = 0; i < w; ++i) n *= 3;
  return n;
}
static KnownBits expBits(unsigned w, unsigned c, uint64_t &st) {
  uint64_t z = 0, o = 0;
  if (w <= 5) {
    unsigned t = c;
    for (unsigned i = 0; i < w; ++i, t /= 3) {
      if (t % 3 == 1) z |= 1ULL << i;
      if (t % 3 == 2) o |= 1ULL << i;
    }
  } else {
    uint64_t m = (1ULL << w) - 1;
    st = st * 6364136223846793005ULL + 1442695040888963407ULL;
    uint64_t a = (st >> 33) & m;
    st = st * 6364136223846793005ULL + 1442695040888963407ULL;
    uint64_t b = (st >> 33) & m;
    st = st * 6364136223846793005ULL + 1442695040888963407ULL;
    uint64_t keep = (st >> 33) & m;
    z = a & keep & ~b & m;
    o = b & keep;
  }
  KnownBits e(w);
  e.Zero = APInt(w, z);
  e.One = APInt(w, o);
  return e;
}

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s\n", LLVM_VERSION_STRING);
    return 0;
  }
  H = 0xcbf29ce484222325ULL;
  const States all = allStates(), full = fullStates(), cur = curatedStates(true), curNoSign = curatedStates(false);
  const std::vector<Mode> modes = allModes(), two = twoModes();
  const std::vector<Fmt> fmts = formats();
  const std::vector<unsigned> masks = curatedMasks();

  // State queries.
  {
    for (const auto &k : all) {
      hbegin();
      for (bool b : {k.isKnownNeverNaN(), k.isKnownAlwaysNaN(), k.isKnownNeverInfinity(), k.isKnownNeverInfOrNaN(),
                     k.isKnownNeverPosInfinity(), k.isKnownNeverNegInfinity(), k.isKnownNeverSubnormal(),
                     k.isKnownNeverPosSubnormal(), k.isKnownNeverNegSubnormal(), k.isKnownNeverZero(),
                     k.isKnownNeverPosZero(), k.isKnownNeverNegZero(), k.cannotBeOrderedLessThanZero(),
                     k.cannotBeOrderedGreaterThanZero(), k.signBitIsZeroOrNaN(), k.isUnknown()})
        hadd(b);
      for (unsigned m : masks) {
        hadd(k.isKnownNever(static_cast<FPClassTest>(m)));
        hadd(k.isKnownAlways(static_cast<FPClassTest>(m)));
      }
      hend();
    }
    line("queries");
    for (const auto &k : all)
      for (const Mode &m : modes) {
        hbegin();
        hadd(k.isKnownNeverLogicalZero(m)); hadd(k.isKnownNeverLogicalNegZero(m));
        hadd(k.isKnownNeverLogicalPosZero(m)); hadd(k.cannotBeOrderedLessEqZero(m));
        hadd(k.cannotBeOrderedGreaterEqZero(m));
        hend();
      }
    line("logicalQueries");
  }

  // Mutators and unary operations of the state alone.
  auto un = [&](const char *name, auto fn) {
    for (const auto &k : all) {
      hbegin();
      fn(k);
      hend();
    }
    line(name);
  };
  un("knownNot", [&](const KnownFPClass &k) {
    for (unsigned m : masks) { KnownFPClass t = k; t.knownNot(static_cast<FPClassTest>(m)); addK(t); }
  });
  un("fneg", [](const KnownFPClass &k) { KnownFPClass t = k; t.fneg(); addK(t); addK(KnownFPClass::fneg(k)); });
  un("fabs", [](const KnownFPClass &k) { KnownFPClass t = k; t.fabs(); addK(t); addK(KnownFPClass::fabs(k)); });
  un("signBit", [](const KnownFPClass &k) {
    KnownFPClass a = k, b = k, c = k;
    a.signBitMustBeZero(); b.signBitMustBeOne(); c.resetAll();
    addK(a); addK(b); addK(c);
  });
  un("propagateNonNaN", [](const KnownFPClass &k) {
    KnownFPClass a, b;
    a.propagateNonSNaN(k); b.propagateNonNaN(k);
    addK(a); addK(b);
  });
  un("exp", [](const KnownFPClass &k) { addK(KnownFPClass::exp(k)); });
  un("sin", [](const KnownFPClass &k) { addK(KnownFPClass::sin(k)); });
  un("cos", [](const KnownFPClass &k) { addK(KnownFPClass::cos(k)); });
  un("tan", [](const KnownFPClass &k) { addK(KnownFPClass::tan(k)); });
  un("sinh", [](const KnownFPClass &k) { addK(KnownFPClass::sinh(k)); });
  un("cosh", [](const KnownFPClass &k) { addK(KnownFPClass::cosh(k)); });
  un("tanh", [](const KnownFPClass &k) { addK(KnownFPClass::tanh(k)); });
  un("asin", [](const KnownFPClass &k) { addK(KnownFPClass::asin(k)); });
  un("acos", [](const KnownFPClass &k) { addK(KnownFPClass::acos(k)); });
  un("atan", [](const KnownFPClass &k) { addK(KnownFPClass::atan(k)); });
  un("fptrunc", [](const KnownFPClass &k) { addK(KnownFPClass::fptrunc(k)); });
  un("roundToIntegral", [](const KnownFPClass &k) {
    for (int t = 0; t < 2; ++t)
      for (int mu = 0; mu < 2; ++mu) addK(KnownFPClass::roundToIntegral(k, t, mu));
  });
  un("fpext", [&](const KnownFPClass &k) {
    for (const Fmt &d : fmts)
      for (const Fmt &s : fmts) addK(KnownFPClass::fpext(k, *d.sem, *s.sem));
  });
  un("toKnownBits", [&](const KnownFPClass &k) {
    for (const Fmt &f : fmts) addBits(k.toKnownBits(*f.sem));
  });

  // Operations of one state and a mode.
  auto um = [&](const char *name, auto fn) {
    for (const auto &k : all)
      for (const Mode &m : modes) {
        hbegin();
        fn(k, m);
        hend();
      }
    line(name);
  };
  um("fadd_self", [](const KnownFPClass &k, Mode m) { addK(KnownFPClass::fadd_self(k, m)); });
  um("fdiv_self", [](const KnownFPClass &k, Mode m) { addK(KnownFPClass::fdiv_self(k, m)); });
  um("frem_self", [](const KnownFPClass &k, Mode m) { addK(KnownFPClass::frem_self(k, m)); });
  um("square", [](const KnownFPClass &k, Mode m) { addK(KnownFPClass::square(k, m)); });
  um("canonicalize", [](const KnownFPClass &k, Mode m) { addK(KnownFPClass::canonicalize(k, m)); });
  um("log", [](const KnownFPClass &k, Mode m) { addK(KnownFPClass::log(k, m)); });
  um("sqrt", [](const KnownFPClass &k, Mode m) { addK(KnownFPClass::sqrt(k, m)); });
  um("frexp_mant", [](const KnownFPClass &k, Mode m) { addK(KnownFPClass::frexp_mant(k, m)); });
  um("propagateDenormal", [](const KnownFPClass &k, Mode m) {
    KnownFPClass a(fcNone, true), b;
    a.propagateDenormal(k, m);
    b.propagateCanonicalizingSrc(k, m);
    addK(a); addK(b);
  });

  // Binary operations over every mask (sign unknown) with two modes, and over the curated states with every mode.
  auto bin = [&](const std::string &name, auto fn) {
    for (int pass = 0; pass < 2; ++pass) {
      const States &S = pass == 0 ? full : cur;
      const std::vector<Mode> &M = pass == 0 ? two : modes;
      for (const auto &a : S)
        for (const auto &b : S)
          for (const Mode &m : M) {
            hbegin();
            fn(a, b, m);
            hend();
          }
      line(name + (pass == 0 ? ".full" : ".cur"));
    }
  };
  bin("fadd", [](const KnownFPClass &a, const KnownFPClass &b, Mode m) { addK(KnownFPClass::fadd(a, b, m)); });
  bin("fsub", [](const KnownFPClass &a, const KnownFPClass &b, Mode m) { addK(KnownFPClass::fsub(a, b, m)); });
  bin("fmul", [](const KnownFPClass &a, const KnownFPClass &b, Mode m) { addK(KnownFPClass::fmul(a, b, m)); });
  bin("fdiv", [](const KnownFPClass &a, const KnownFPClass &b, Mode m) { addK(KnownFPClass::fdiv(a, b, m)); });
  bin("frem", [](const KnownFPClass &a, const KnownFPClass &b, Mode m) { addK(KnownFPClass::frem(a, b, m)); });
  bin("atan2", [](const KnownFPClass &a, const KnownFPClass &b, Mode m) { addK(KnownFPClass::atan2(a, b, m)); });
  bin("fma_square", [](const KnownFPClass &a, const KnownFPClass &b, Mode m) { addK(KnownFPClass::fma_square(a, b, m)); });
  bin("minMaxLike", [](const KnownFPClass &a, const KnownFPClass &b, Mode m) {
    for (auto kind : {KnownFPClass::MinMaxKind::minimum, KnownFPClass::MinMaxKind::maximum,
                      KnownFPClass::MinMaxKind::minimumnum, KnownFPClass::MinMaxKind::maximumnum,
                      KnownFPClass::MinMaxKind::minnum, KnownFPClass::MinMaxKind::maxnum})
      addK(KnownFPClass::minMaxLike(a, b, kind, m));
  });
  bin("pairs", [](const KnownFPClass &a, const KnownFPClass &b, Mode) {
    addK(KnownFPClass::pow(a, b));
    addK(a.intersectWith(b)); addK(a.unionWith(b)); addK(a | b);
    KnownFPClass t = a;
    t |= b;
    addK(t);
    addK(KnownFPClass::copysign(a, b));
    KnownFPClass c = a;
    c.copysign(b);
    addK(c);
    KnownFPClass x, y, z;
    x.propagateNonSNaN(a, b); y.propagateNonNaN(a, b); z.propagateXorSign(a, b);
    addK(x); addK(y); addK(z);
    hadd(a == b);
  });

  // fma over the curated masks.
  for (const auto &a : curNoSign)
    for (const auto &b : curNoSign)
      for (const auto &c : curNoSign)
        for (const Mode &m : two) {
          hbegin();
          addK(KnownFPClass::fma(a, b, c, m));
          hend();
        }
  line("fma");

  // Constants.
  for (const Fmt &f : fmts) {
    for (u128 bits : inputs(f)) {
      hbegin();
      addK(KnownFPClass(make(f, bits)));
      hend();
    }
    line(std::string("fromAPFloat.") + f.name);
  }
  for (const Fmt &f : fmts) {
    for (const auto &k : cur)
      for (u128 bits : inputs(f))
        for (const Mode &m : two) {
          hbegin();
          addK(KnownFPClass::fmul(k, make(f, bits), m));
          hend();
        }
    line(std::string("fmulConstant.") + f.name);
  }

  // Bitcast and known bits.
  for (const Fmt &f : fmts) {
    std::vector<KnownBits> kb;
    u128 expMask = ones(f.size - 1) & ~ones(f.fb + (f.expl ? 1 : 0));
    u128 quiet = (u128)1 << (f.fb - 1);
    u128 sign = (u128)1 << (f.size - 1);
    u128 manMask = ones(f.fb);
    uint64_t st = 0x51ed270b0b1d4f3dULL ^ f.size;
    auto next = [&] { st = st * 6364136223846793005ULL + 1442695040888963407ULL; return st >> 33; };
    for (u128 v : inputs(f)) {
      u128 r1 = next(), r2 = next();
      r1 = (r1 << 32) ^ r2;
      for (u128 mk : {ones(f.size), sign, expMask, expMask | quiet, manMask, (u128)0, sign | expMask, r1 & ones(f.size)})
        kb.push_back(knownOf(f.size, v, mk));
    }
    for (int i = 0; i < 200; ++i) {
      u128 a = next(), b = next(), c = next(), d = next(), e = next(), g = next();
      u128 za = (((a << 32) ^ b) << 16) ^ c, oa = (((d << 32) ^ e) << 16) ^ g;
      KnownBits k = knownOf(f.size, 0, 0);
      auto ap = [&](u128 x) {
        x &= ones(f.size);
        if (f.size <= 64) return APInt(f.size, (uint64_t)x);
        uint64_t t[2] = {(uint64_t)x, (uint64_t)(x >> 64)};
        return APInt(f.size, ArrayRef<uint64_t>(t, 2));
      };
      if (i % 4 == 0) {  // may conflict
        k.Zero = ap(za);
        k.One = ap(oa);
      } else {  // never conflicts
        k.Zero = ap(za & ~oa);
        k.One = ap(oa);
      }
      kb.push_back(k);
    }
    kb.push_back(knownOf(f.size, 0, 0));
    for (const KnownBits &k : kb) {
      hbegin();
      addK(KnownFPClass::bitcast(*f.sem, k));
      hend();
    }
    line(std::string("bitcast.") + f.name);
  }

  // ldexp.
  {
    const int64_t E[] = {-300, -24, -23, -1, 0, 1, 2, 3, 22, 23, 24, 51, 52, 53, 62, 63, 64, 300};
    for (const Fmt &f : fmts) {
      for (const auto &k : cur)
        for (int64_t lo : E)
          for (int64_t hi : E) {
            if (lo > hi) continue;
            for (const Mode &m : two) {
              hbegin();
              addK(KnownFPClass::ldexp(k, APInt(32, (uint64_t)lo, true), APInt(32, (uint64_t)hi, true), *f.sem, m));
              hend();
            }
          }
      line(std::string("ldexp.") + f.name);
    }
    for (unsigned w : {5u, 8u})
      for (const Fmt &f : fmts) {
        for (const auto &k : cur) {
          uint64_t st = 0x3c6ef372fe94f82bULL;
          for (unsigned c = 0; c < expCount(w); ++c) {
            KnownBits e = expBits(w, c, st);
            for (const Mode &m : two) {
              hbegin();
              addK(KnownFPClass::ldexp(k, e, *f.sem, m));
              hend();
            }
          }
        }
        line(std::string("ldexpBits") + std::to_string(w) + "." + f.name);
      }
  }

  // powi.
  for (unsigned w : {4u, 8u}) {
    for (const auto &k : cur) {
      uint64_t st = 0x7a1c3d5e9b2f4601ULL;
      for (unsigned c = 0; c < (w == 4 ? expCount(4) : 300); ++c) {
        hbegin();
        addK(KnownFPClass::powi(k, expBits(w, c, st)));
        hend();
      }
    }
    line("powi" + std::to_string(w));
  }
  return 0;
}

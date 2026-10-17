// Writes one hash per ConstantRange operation over every range of widths 1..4 and a sample of widths 6, 8, 16, 64:
// the golden file crates/ir/llrm-mir/src/constantrange_golden.txt, which `constantrange_tests.rs` recomputes.
//   tools/llvmport/constantrange_dump.sh > crates/ir/llrm-mir/src/constantrange_golden.txt
#include "llvm/ADT/APInt.h"
#include "llvm/IR/ConstantRange.h"
#include "llvm/IR/Instructions.h"
#include "llvm/IR/Intrinsics.h"
#include "llvm/IR/Operator.h"
#include "llvm/Support/KnownBits.h"
#include <cstdint>
#include <cstdio>
#include <functional>
#include <string>
#include <vector>

using namespace llvm;
using CR = ConstantRange;
using Bin = std::function<CR(const CR &, const CR &)>;
using Un = std::function<CR(const CR &)>;

static uint64_t H = 0xcbf29ce484222325ULL;
static void mix(uint64_t v) {
  for (int i = 0; i < 8; ++i) {
    H ^= (v >> (8 * i)) & 0xff;
    H *= 0x100000001b3ULL;
  }
}
// An APInt as one word, or two above 64 bits: the Rust side hashes the same words.
static void mixAP(const APInt &v) {
  unsigned w = v.getBitWidth();
  mix(v.extractBitsAsZExtValue(w < 64 ? w : 64, 0));
  if (w > 64) mix(v.extractBitsAsZExtValue(w - 64, 64));
}
static void out(const CR &r) {
  mix(r.getBitWidth());
  mixAP(r.getLower());
  mixAP(r.getUpper());
}
static void outKB(const KnownBits &k) {
  mix(k.getBitWidth());
  mixAP(k.Zero);
  mixAP(k.One);
}
static void line(const std::string &name, unsigned W) {
  printf("%s %u %016llx\n", name.c_str(), W, (unsigned long long)H);
  H = 0xcbf29ce484222325ULL;
}

static uint64_t maskOf(unsigned W) { return W >= 64 ? ~0ULL : ((1ULL << W) - 1); }

static std::vector<CR> ranges(unsigned W) {
  std::vector<CR> v;
  uint64_t m = maskOf(W);
  if (W > 64) {
    uint64_t s = 0x9e3779b97f4a7c15ULL ^ W;
    auto next = [&] { s = s * 6364136223846793005ULL + 1442695040888963407ULL; return s >> 33; };
    auto word = [&](unsigned bits) -> uint64_t {
      if (bits < 64) return next() & ((1ULL << bits) - 1);
      uint64_t hi = next(), lo = next();
      return hi << 32 ^ lo;
    };
    auto mk = [&] {
      uint64_t lo = word(64), hi = word(W - 64);
      uint64_t arr[2] = {lo, hi};
      return APInt(W, ArrayRef<uint64_t>(arr, 2));
    };
    APInt mx = APInt::getMaxValue(W), sm = APInt::getSignedMinValue(W);
    v.push_back(CR::getFull(W));
    v.push_back(CR::getEmpty(W));
    APInt specials[] = {APInt(W, 0), APInt(W, 1), APInt(W, 2), APInt(W, 3), mx, mx - 1, mx - 2,
                        sm, sm - 1, sm + 1, sm + 2, sm - 2};
    for (const APInt &a : specials)
      for (const APInt &b : {APInt(W, 0), APInt(W, 1), sm, mx, sm + 1, APInt(W, 5)}) {
        if (a == b && !a.isZero() && !a.isMaxValue()) continue;
        v.push_back(CR(a, b));
      }
    for (unsigned i = 0; i < 50; ++i) {
      APInt a = mk();
      APInt b = mk();
      if (i % 3 == 0) b = a + APInt(W, next() & 0xf);
      if (a == b && !a.isZero() && !a.isMaxValue()) continue;
      v.push_back(CR(a, b));
    }
    return v;
  }
  if (W <= 4) {
    for (uint64_t l = 0; l <= m; ++l)
      for (uint64_t u = 0; u <= m; ++u) {
        if (l == u && l != 0 && l != m) continue;
        v.push_back(CR(APInt(W, l), APInt(W, u)));
      }
  } else {
    uint64_t s = 0x9e3779b97f4a7c15ULL ^ W;
    auto next = [&] { s = s * 6364136223846793005ULL + 1442695040888963407ULL; return s >> 11 ^ (s << 20); };
    v.push_back(CR::getFull(W));
    v.push_back(CR::getEmpty(W));
    uint64_t sm = 1ULL << (W - 1);
    uint64_t specials[] = {0, 1, 2, 3, m, m - 1, m - 2, sm, sm - 1, sm + 1, sm + 2, sm - 2};
    for (uint64_t a : specials)
      for (uint64_t b : std::initializer_list<uint64_t>{0, 1, sm, m, sm + 1, 5}) {
        a &= m; b &= m;
        if (a == b && a != 0 && a != m) continue;
        v.push_back(CR(APInt(W, a), APInt(W, b)));
      }
    unsigned n = W <= 8 ? 120 : 50;
    for (unsigned i = 0; i < n; ++i) {
      uint64_t a = next() & m, b = next() & m;
      if (i % 3 == 0) b = (a + (next() & 0xf)) & m;
      if (a == b && a != 0 && a != m) continue;
      v.push_back(CR(APInt(W, a), APInt(W, b)));
    }
  }
  return v;
}

int main() {
  std::vector<std::pair<std::string, Bin>> bins;
  auto B = [&](const std::string &n, Bin f) { bins.push_back({n, f}); };
  B("add", [](const CR &a, const CR &b) { return a.add(b); });
  B("sub", [](const CR &a, const CR &b) { return a.sub(b); });
  for (unsigned k = 0; k < 4; ++k) {
    B("mul" + std::to_string(k), [=](const CR &a, const CR &b) { return a.multiply(b, k); });
    B("addNW" + std::to_string(k), [=](const CR &a, const CR &b) { return a.addWithNoWrap(b, k); });
    B("subNW" + std::to_string(k), [=](const CR &a, const CR &b) { return a.subWithNoWrap(b, k); });
    B("shlNW" + std::to_string(k), [=](const CR &a, const CR &b) { return a.shlWithNoWrap(b, k); });
  }
  B("smul_fast", [](const CR &a, const CR &b) { return a.smul_fast(b); });
  B("smax", [](const CR &a, const CR &b) { return a.smax(b); });
  B("umax", [](const CR &a, const CR &b) { return a.umax(b); });
  B("smin", [](const CR &a, const CR &b) { return a.smin(b); });
  B("umin", [](const CR &a, const CR &b) { return a.umin(b); });
  B("udiv", [](const CR &a, const CR &b) { return a.udiv(b); });
  B("sdiv", [](const CR &a, const CR &b) { return a.sdiv(b); });
  B("urem", [](const CR &a, const CR &b) { return a.urem(b); });
  B("srem", [](const CR &a, const CR &b) { return a.srem(b); });
  B("and", [](const CR &a, const CR &b) { return a.binaryAnd(b); });
  B("or", [](const CR &a, const CR &b) { return a.binaryOr(b); });
  B("orDisjoint", [](const CR &a, const CR &b) { return a.binaryOr(b, true); });
  B("xor", [](const CR &a, const CR &b) { return a.binaryXor(b); });
  B("shl", [](const CR &a, const CR &b) { return a.shl(b); });
  B("lshr", [](const CR &a, const CR &b) { return a.lshr(b); });
  B("ashr", [](const CR &a, const CR &b) { return a.ashr(b); });
  B("uadd_sat", [](const CR &a, const CR &b) { return a.uadd_sat(b); });
  B("sadd_sat", [](const CR &a, const CR &b) { return a.sadd_sat(b); });
  B("usub_sat", [](const CR &a, const CR &b) { return a.usub_sat(b); });
  B("ssub_sat", [](const CR &a, const CR &b) { return a.ssub_sat(b); });
  B("umul_sat", [](const CR &a, const CR &b) { return a.umul_sat(b); });
  B("smul_sat", [](const CR &a, const CR &b) { return a.smul_sat(b); });
  B("ushl_sat", [](const CR &a, const CR &b) { return a.ushl_sat(b); });
  B("sshl_sat", [](const CR &a, const CR &b) { return a.sshl_sat(b); });
  B("difference", [](const CR &a, const CR &b) { return a.difference(b); });
  for (int t = 0; t < 3; ++t) {
    auto T = (CR::PreferredRangeType)t;
    B("intersect" + std::to_string(t), [=](const CR &a, const CR &b) { return a.intersectWith(b, T); });
    B("union" + std::to_string(t), [=](const CR &a, const CR &b) { return a.unionWith(b, T); });
  }
  B("addNW3signed", [](const CR &a, const CR &b) { return a.addWithNoWrap(b, 3, CR::Signed); });
  B("subNW3unsigned", [](const CR &a, const CR &b) { return a.subWithNoWrap(b, 3, CR::Unsigned); });
  B("noWrapAddU", [](const CR &, const CR &b) { return CR::makeGuaranteedNoWrapRegion(Instruction::Add, b, 1); });
  B("noWrapAddS", [](const CR &, const CR &b) { return CR::makeGuaranteedNoWrapRegion(Instruction::Add, b, 2); });
  B("noWrapSubU", [](const CR &, const CR &b) { return CR::makeGuaranteedNoWrapRegion(Instruction::Sub, b, 1); });
  B("noWrapSubS", [](const CR &, const CR &b) { return CR::makeGuaranteedNoWrapRegion(Instruction::Sub, b, 2); });
  B("noWrapMulU", [](const CR &, const CR &b) { return CR::makeGuaranteedNoWrapRegion(Instruction::Mul, b, 1); });
  B("noWrapMulS", [](const CR &, const CR &b) { return CR::makeGuaranteedNoWrapRegion(Instruction::Mul, b, 2); });
  B("noWrapShlU", [](const CR &, const CR &b) { return CR::makeGuaranteedNoWrapRegion(Instruction::Shl, b, 1); });
  B("noWrapShlS", [](const CR &, const CR &b) { return CR::makeGuaranteedNoWrapRegion(Instruction::Shl, b, 2); });
  for (int p = 32; p <= 41; ++p) {
    auto P = (CmpInst::Predicate)p;
    B("allowed" + std::to_string(p), [=](const CR &, const CR &b) { return CR::makeAllowedICmpRegion(P, b); });
    B("satisfying" + std::to_string(p), [=](const CR &, const CR &b) { return CR::makeSatisfyingICmpRegion(P, b); });
    B("allowedSameSign" + std::to_string(p), [=](const CR &, const CR &b) { return CR::makeAllowedICmpRegion(CmpPredicate(P, true), b); });
  }

  std::vector<std::pair<std::string, Un>> uns;
  auto U = [&](const std::string &n, Un f) { uns.push_back({n, f}); };
  U("inverse", [](const CR &a) { return a.inverse(); });
  U("abs", [](const CR &a) { return a.abs(); });
  U("absPoison", [](const CR &a) { return a.abs(true); });
  U("ctlz", [](const CR &a) { return a.ctlz(); });
  U("ctlzPoison", [](const CR &a) { return a.ctlz(true); });
  U("cttz", [](const CR &a) { return a.cttz(); });
  U("cttzPoison", [](const CR &a) { return a.cttz(true); });
  U("ctpop", [](const CR &a) { return a.ctpop(); });
  U("sqrtFloor", [](const CR &a) { return a.sqrtFloor(); });
  U("binaryNot", [](const CR &a) { return a.binaryNot(); });
  U("zext3", [](const CR &a) { return a.getBitWidth() + 3 > 128 ? a : a.zeroExtend(a.getBitWidth() + 3); });
  U("sext3", [](const CR &a) { return a.getBitWidth() + 3 > 128 ? a : a.signExtend(a.getBitWidth() + 3); });
  U("splitPos", [](const CR &a) { return a.splitPosNeg().first; });
  U("splitNeg", [](const CR &a) { return a.splitPosNeg().second; });
  for (unsigned d = 1; d < 8; ++d)
    for (unsigned nw = 0; nw < 2; ++nw) {
      U("trunc" + std::to_string(d) + "_" + std::to_string(nw),
        [=](const CR &a) { return d < a.getBitWidth() ? a.truncate(d, nw) : a; });
    }
  for (unsigned c = 0; c < 4; ++c) {
    U("castF2I" + std::to_string(c), [=](const CR &a) {
      if (a.getBitWidth() + 2 > 128) return a;
      return a.castOp(c == 0 ? Instruction::UIToFP : c == 1 ? Instruction::SIToFP : c == 2 ? Instruction::FPToUI : Instruction::BitCast,
                      a.getBitWidth() + (c < 2 ? 2 : 0));
    });
  }

  for (unsigned W : {1u, 2u, 3u, 4u, 6u, 8u, 16u, 64u, 65u, 100u, 128u}) {
    std::vector<CR> in = ranges(W);
    for (auto &[name, f] : bins) {
      for (auto &a : in)
        for (auto &b : in) out(f(a, b));
      line(name, W);
    }
    for (auto &[name, f] : uns) {
      for (auto &a : in) out(f(a));
      line(name, W);
    }
    // Predicates and queries.
    for (int p = 32; p <= 41; ++p) {
      for (auto &a : in)
        for (auto &b : in) mix(a.icmp((CmpInst::Predicate)p, b));
      line("icmp" + std::to_string(p), W);
    }
    for (auto &a : in)
      for (auto &b : in) {
        mix(a.contains(b)); mix(a.isSizeStrictlySmallerThan(b)); mix(a == b);
        mix(a.unsignedAddMayOverflow(b) == CR::OverflowResult::NeverOverflows ? 3 : (int)a.unsignedAddMayOverflow(b));
        mix((int)a.signedAddMayOverflow(b)); mix((int)a.unsignedSubMayOverflow(b));
        mix((int)a.signedSubMayOverflow(b)); mix((int)a.unsignedMulMayOverflow(b));
        mix(CR::areInsensitiveToSignednessOfICmpPredicate(a, b));
        mix(CR::areInsensitiveToSignednessOfInvertedICmpPredicate(a, b));
        auto ei = a.exactIntersectWith(b);
        mix(ei.has_value());
        if (ei) out(*ei);
        auto eu = a.exactUnionWith(b);
        mix(eu.has_value());
        if (eu) out(*eu);
        for (int p : {34, 35, 36, 37, 38, 39, 40, 41})
          mix((int)CR::getEquivalentPredWithFlippedSignedness((CmpInst::Predicate)p, a, b));
      }
    line("pairQueries", W);
    for (auto &a : in) {
      mixAP(a.getUnsignedMin()); mixAP(a.getUnsignedMax());
      mixAP(a.getSignedMin()); mixAP(a.getSignedMax());
      mix(a.isFullSet()); mix(a.isEmptySet()); mix(a.isWrappedSet()); mix(a.isUpperWrapped());
      mix(a.isSignWrappedSet()); mix(a.isUpperSignWrapped()); mix(a.isAllNegative());
      mix(a.isAllNonNegative()); mix(a.isAllPositive()); mix(a.getActiveBits()); mix(a.getMinSignedBits());
      mix(a.isSingleElement());
      if (a.getSingleElement()) mixAP(*a.getSingleElement()); else mix(99);
      if (a.getSingleMissingElement()) mixAP(*a.getSingleMissingElement()); else mix(99);
      for (unsigned k : {1u, 2u, 3u, 4u, 100u}) mix(a.isSizeLargerThan(k));
      mix(a.isSizeLargerThan(0));
      CmpInst::Predicate P; APInt RHS, Off;
      a.getEquivalentICmp(P, RHS, Off);
      mix((int)P); mixAP(RHS); mixAP(Off);
      outKB(a.toKnownBits());
    }
    line("queries", W);
    if (W <= 8) {
      for (auto &a : in)
        for (uint64_t v = 0; v <= maskOf(W) && v < 64; ++v) {
          mix(a.contains(APInt(W, v)));
          out(a.subtract(APInt(W, v)));
        }
      line("containsSubtract", W);
    }
    for (uint64_t v = 0; v <= maskOf(W) && v < 16; ++v) {
      for (int p = 32; p <= 41; ++p) out(CR::makeExactICmpRegion((CmpInst::Predicate)p, APInt(W, v)));
      for (unsigned k : {1u, 2u}) {
        out(CR::makeExactNoWrapRegion(Instruction::Add, APInt(W, v), k));
        out(CR::makeExactNoWrapRegion(Instruction::Sub, APInt(W, v), k));
        out(CR::makeExactNoWrapRegion(Instruction::Mul, APInt(W, v), k));
        out(CR::makeExactNoWrapRegion(Instruction::Shl, APInt(W, v), k));
      }
    }
    line("exactRegions", W);
    if (W <= 4) {
      for (uint64_t m = 0; m <= maskOf(W); ++m)
        for (uint64_t c = 0; c <= maskOf(W); ++c) out(CR::makeMaskNotEqualRange(APInt(W, m), APInt(W, c)));
      line("maskNotEqual", W);
      uint64_t n = 1;
      for (unsigned i = 0; i < W; ++i) n *= 3;
      for (uint64_t c = 0; c < n; ++c) {
        uint64_t Z = 0, O = 0, t = c;
        for (unsigned i = 0; i < W; ++i, t /= 3) {
          if (t % 3 == 1) Z |= 1ULL << i;
          if (t % 3 == 2) O |= 1ULL << i;
        }
        KnownBits K(W);
        K.Zero = APInt(W, Z);
        K.One = APInt(W, O);
        out(CR::fromKnownBits(K, false));
        out(CR::fromKnownBits(K, true));
      }
      line("fromKnownBits", W);
    }
  }
  return 0;
}

// Writes one hash per KnownBits operation over every input of widths 1..5 and a sample of width 8:
// the golden file crates/ir/llrm-mir/src/knownbits_golden.txt, which `knownbits_tests.rs` recomputes.
// Build against the LLVM of d1106deb71cc:
//   tools/llvmport/knownbits_dump.sh > crates/ir/llrm-mir/src/knownbits_golden.txt
#include "llvm/ADT/APInt.h"
#include "llvm/Support/KnownBits.h"
#include <cstdint>
#include <cstdio>
#include <functional>
#include <string>
#include <vector>

using namespace llvm;
using Bin = std::function<KnownBits(const KnownBits &, const KnownBits &)>;
using Cmp = std::function<std::optional<bool>(const KnownBits &, const KnownBits &)>;
using Un = std::function<KnownBits(const KnownBits &)>;

static uint64_t H;
static void mix(uint64_t v) {
  for (int i = 0; i < 8; ++i) {
    H ^= (v >> (8 * i)) & 0xff;
    H *= 0x100000001b3ULL;
  }
}
static void out(const KnownBits &k) {
  mix(k.getBitWidth());
  mix(k.Zero.getZExtValue());
  mix(k.One.getZExtValue());
}

static KnownBits make(unsigned W, uint64_t Z, uint64_t O) {
  KnownBits K(W);
  K.Zero = APInt(W, Z);
  K.One = APInt(W, O);
  return K;
}

// Every non-conflicting KnownBits of width W (W <= 5), or a sample at W == 8.
static std::vector<KnownBits> inputs(unsigned W) {
  std::vector<KnownBits> v;
  if (W <= 5) {
    uint64_t n = 1;
    for (unsigned i = 0; i < W; ++i) n *= 3;
    for (uint64_t c = 0; c < n; ++c) {
      uint64_t Z = 0, O = 0, t = c;
      for (unsigned i = 0; i < W; ++i, t /= 3) {
        if (t % 3 == 1) Z |= 1ULL << i;
        if (t % 3 == 2) O |= 1ULL << i;
      }
      v.push_back(make(W, Z, O));
    }
  } else {
    uint64_t s = 0x9e3779b97f4a7c15ULL;
    auto next = [&] { s = s * 6364136223846793005ULL + 1442695040888963407ULL; return s >> 33; };
    for (int i = 0; i < 160; ++i) {
      uint64_t a = next() & 0xff, b = next() & 0xff, keep = next() & 0xff;
      v.push_back(make(W, a & keep & ~b, b & keep));
    }
    v.push_back(make(W, 0, 0));
    v.push_back(make(W, 0xff, 0));
    v.push_back(make(W, 0, 0xff));
  }
  return v;
}

static void line(const char *name, unsigned W) { printf("%s %u %016llx\n", name, W, (unsigned long long)H); H = 0xcbf29ce484222325ULL; }

int main() {
  H = 0xcbf29ce484222325ULL;
  std::vector<std::pair<std::string, Bin>> bins;
#define B(NAME, EXPR) bins.push_back({NAME, [](const KnownBits &L, const KnownBits &R) { return EXPR; }});
  for (int nsw = 0; nsw < 2; ++nsw)
    for (int nuw = 0; nuw < 2; ++nuw) {
      std::string s = std::to_string(nsw) + std::to_string(nuw);
      bins.push_back({"add" + s, [=](const KnownBits &L, const KnownBits &R) { return KnownBits::add(L, R, nsw, nuw); }});
      bins.push_back({"sub" + s, [=](const KnownBits &L, const KnownBits &R) { return KnownBits::sub(L, R, nsw, nuw); }});
    }
  B("sadd_sat", KnownBits::sadd_sat(L, R)) B("ssub_sat", KnownBits::ssub_sat(L, R))
  B("uadd_sat", KnownBits::uadd_sat(L, R)) B("usub_sat", KnownBits::usub_sat(L, R))
  B("avgFloorS", KnownBits::avgFloorS(L, R)) B("avgFloorU", KnownBits::avgFloorU(L, R))
  B("avgCeilS", KnownBits::avgCeilS(L, R)) B("avgCeilU", KnownBits::avgCeilU(L, R))
  B("mul", KnownBits::mul(L, R)) B("mulhs", KnownBits::mulhs(L, R)) B("mulhu", KnownBits::mulhu(L, R))
  B("sdiv", KnownBits::sdiv(L, R)) B("sdivExact", KnownBits::sdiv(L, R, true))
  B("udiv", KnownBits::udiv(L, R)) B("udivExact", KnownBits::udiv(L, R, true))
  B("urem", KnownBits::urem(L, R)) B("srem", KnownBits::srem(L, R))
  B("umax", KnownBits::umax(L, R)) B("umin", KnownBits::umin(L, R))
  B("smax", KnownBits::smax(L, R)) B("smin", KnownBits::smin(L, R))
  B("abdu", KnownBits::abdu(L, R)) B("abds", KnownBits::abds(L, R))
  B("shl", KnownBits::shl(L, R)) B("shlNuw", KnownBits::shl(L, R, true, false))
  B("shlNsw", KnownBits::shl(L, R, false, true)) B("shlNuwNsw", KnownBits::shl(L, R, true, true))
  B("shlNonZero", KnownBits::shl(L, R, false, false, true))
  B("lshr", KnownBits::lshr(L, R)) B("lshrNonZero", KnownBits::lshr(L, R, true))
  B("lshrExact", KnownBits::lshr(L, R, false, true)) B("lshrNonZeroExact", KnownBits::lshr(L, R, true, true))
  B("ashr", KnownBits::ashr(L, R)) B("ashrNonZero", KnownBits::ashr(L, R, true))
  B("ashrExact", KnownBits::ashr(L, R, false, true)) B("ashrNonZeroExact", KnownBits::ashr(L, R, true, true))
  B("and", L & R) B("or", L | R) B("xor", L ^ R)
  B("clmul", KnownBits::clmul(L, R)) B("pext", KnownBits::pext(L, R)) B("pdep", KnownBits::pdep(L, R))
  B("intersect", L.intersectWith(R)) B("union", L.unionWith(R))
  std::vector<std::pair<std::string, Cmp>> cmps;
#define C(NAME, FN) cmps.push_back({NAME, [](const KnownBits &L, const KnownBits &R) { return KnownBits::FN(L, R); }});
  C("eq", eq) C("ne", ne) C("ugt", ugt) C("uge", uge) C("ult", ult) C("ule", ule)
  C("sgt", sgt) C("sge", sge) C("slt", slt) C("sle", sle)
  std::vector<std::pair<std::string, Un>> uns;
#define U(NAME, EXPR) uns.push_back({NAME, [](const KnownBits &K) { return EXPR; }});
  U("abs", K.abs()) U("absPoison", K.abs(true)) U("reduceAdd3", K.reduceAdd(3)) U("reduceAdd1", K.reduceAdd(1))
  U("blsi", K.blsi()) U("blsmsk", K.blsmsk()) U("reverseBits", K.reverseBits())
  U("anyext", K.anyext(K.getBitWidth() + 3)) U("zext", K.zext(K.getBitWidth() + 3)) U("sext", K.sext(K.getBitWidth() + 3))
  for (unsigned d = 1; d < 5; ++d) {
    // Narrowing and in-register ops at each smaller width (skipped where the width is too small).
    uns.push_back({"trunc" + std::to_string(d), [=](const KnownBits &K) { return d < K.getBitWidth() ? K.trunc(d) : K; }});
    uns.push_back({"truncSSat" + std::to_string(d), [=](const KnownBits &K) { return d < K.getBitWidth() ? K.truncSSat(d) : K; }});
    uns.push_back({"truncSSatU" + std::to_string(d), [=](const KnownBits &K) { return d < K.getBitWidth() ? K.truncSSatU(d) : K; }});
    uns.push_back({"truncUSat" + std::to_string(d), [=](const KnownBits &K) { return d < K.getBitWidth() ? K.truncUSat(d) : K; }});
    uns.push_back({"sextInReg" + std::to_string(d), [=](const KnownBits &K) { return d <= K.getBitWidth() ? K.sextInReg(d) : K; }});
    uns.push_back({"extract" + std::to_string(d), [=](const KnownBits &K) { return d < K.getBitWidth() ? K.extractBits(d, K.getBitWidth() - d) : K; }});
  }
  for (unsigned W : {1u, 2u, 3u, 4u, 5u, 8u}) {
    std::vector<KnownBits> in = inputs(W);
    for (auto &[name, f] : bins) {
      if ((name == "mulhs" || name == "mulhu") && 2 * W > 8) continue;
      for (auto &L : in)
        for (auto &R : in) out(f(L, R));
      line(name.c_str(), W);
    }
    for (auto &[name, f] : cmps) {
      for (auto &L : in)
        for (auto &R : in) {
          auto r = f(L, R);
          mix(r ? (*r ? 1 : 0) : 2);
        }
      line(name.c_str(), W);
    }
    for (auto &[name, f] : uns) {
      for (auto &K : in) out(f(K));
      line(name.c_str(), W);
    }
    // Operations with an extra scalar.
    for (unsigned a = 0; a < 2 * W; ++a) {
      for (auto &L : in)
        for (auto &R : in) {
          out(KnownBits::fshl(L, R, APInt(W, a)));
          out(KnownBits::fshr(L, R, APInt(W, a)));
        }
    }
    line("funnel", W);
    for (uint64_t v = 0; v < (1ULL << W) && v < 256; ++v) {
      for (auto &K : in) out(K.makeGE(APInt(W, v)));
    }
    line("makeGE", W);
    for (auto &L : in)
      for (auto &R : in)
        for (unsigned c = 0; c < 3; ++c) {
          KnownBits Carry(1);
          if (c == 1) Carry.Zero.setBit(0);
          if (c == 2) Carry.One.setBit(0);
          out(KnownBits::computeForAddCarry(L, R, Carry));
          out(KnownBits::computeForSubBorrow(L, R, Carry));
        }
    line("carry", W);
    for (auto &K : in) {
      mix(K.countMinTrailingZeros()); mix(K.countMinTrailingOnes()); mix(K.countMinLeadingZeros());
      mix(K.countMinLeadingOnes()); mix(K.countMinSignBits()); mix(K.countMaxSignificantBits());
      mix(K.countMaxTrailingZeros()); mix(K.countMaxTrailingOnes()); mix(K.countMaxLeadingZeros());
      mix(K.countMaxLeadingOnes()); mix(K.countMinPopulation()); mix(K.countMaxPopulation());
      mix(K.countMaxActiveBits()); mix(K.getMinValue().getZExtValue()); mix(K.getMaxValue().getZExtValue());
      mix(K.getSignedMinValue().getZExtValue()); mix(K.getSignedMaxValue().getZExtValue());
      mix(K.isNonZero()); mix(K.isStrictlyPositive()); mix(K.isNonPositive()); mix(K.isSignUnknown()); mix(K.isEven());
    }
    line("queries", W);
  }
  return 0;
}

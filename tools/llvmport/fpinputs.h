// The formats and the values the dump drivers of the floating-point classes run on: every value of the two 8-bit
// formats, and a fixed sample (edge patterns and 120 pseudo-random ones) of float, double and x87.
// The Rust side is `golden_dump.rs`; both must produce the same values in the same order.
#pragma once
#include "llvm/ADT/APFloat.h"
#include "llvm/ADT/APInt.h"
#include "llvm/ADT/ArrayRef.h"
#include <cstdint>
#include <vector>

using namespace llvm;
typedef unsigned __int128 u128;

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

// The structured edge values of a format (both signs); not for the 8-bit formats, which use every value.
static std::vector<u128> fixedPatterns(const Fmt &f) {
  std::vector<u128> v;
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
  return v;
}

static std::vector<u128> inputs(const Fmt &f) {
  std::vector<u128> v;
  if (f.tiny) {
    for (unsigned i = 0; i < 256; ++i) v.push_back(i);
    return v;
  }
  v = fixedPatterns(f);
  u128 bias = ((u128)1 << (f.eb - 1)) - 1, top = ones(f.eb), fr = ones(f.fb);
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


// The 18 values the range drivers build ranges from, in increasing order (-0 is below +0).
static std::vector<u128> keyPatterns(const Fmt &f) {
  u128 bias = ((u128)1 << (f.eb - 1)) - 1, top = ones(f.eb), fr = ones(f.fb), half = (u128)1 << (f.fb - 1);
  unsigned x = f.expl ? 1 : 0;
  return {pattern(f, 1, top, 0, x),      pattern(f, 1, top - 1, fr, x), pattern(f, 1, bias + 1, 0, x),
          pattern(f, 1, bias, 0, x),     pattern(f, 1, 1, 0, x),        pattern(f, 1, 0, fr, 0),
          pattern(f, 1, 0, 1, 0),        pattern(f, 1, 0, 0, 0),        pattern(f, 0, 0, 0, 0),
          pattern(f, 0, 0, 1, 0),        pattern(f, 0, 0, fr, 0),       pattern(f, 0, 1, 0, x),
          pattern(f, 0, bias - 1, half, x), pattern(f, 0, bias, 0, x),  pattern(f, 0, bias, half, x),
          pattern(f, 0, bias + 1, 0, x), pattern(f, 0, top - 1, fr, x), pattern(f, 0, top, 0, x)};
}

static std::vector<Fmt> formats() {
  return {
      {"e5m2", &APFloat::Float8E5M2(), 5, 2, false, 8, true},
      {"e4m3", &APFloat::Float8E4M3(), 4, 3, false, 8, true},
      {"single", &APFloat::IEEEsingle(), 8, 23, false, 32, false},
      {"double", &APFloat::IEEEdouble(), 11, 52, false, 64, false},
      {"x87", &APFloat::x87DoubleExtended(), 15, 63, true, 80, false},
  };
}

// Writes one hash per FPClassTest / DenormalMode operation over every mask and every mode: the golden file
// crates/ir/llrm-mir/src/fpclass_golden.txt, which `fpclass_tests.rs` recomputes.
// A line is `operation hash distinct`: the hash of every answer, and how many different answers there were.
//   tools/llvmport/fpclass_dump.sh > crates/ir/llrm-mir/src/fpclass_golden.txt
#include "llvm/Config/llvm-config.h"
#include "llvm/ADT/FloatingPointMode.h"
#include "llvm/CodeGen/CodeGenCommonISel.h"
#include "llvm/Support/raw_ostream.h"
#include <cstdint>
#include <cstdio>
#include <set>
#include <string>
#include <vector>

using namespace llvm;

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
static void hstr(const std::string &s) {
  hadd(s.size());
  for (char c : s) hadd((unsigned char)c);
}
static void hend() {
  D.insert(C);
  mix(C);
}
static void line(const char *name) {
  printf("%s %016llx %zu\n", name, (unsigned long long)H, D.size());
  H = 0xcbf29ce484222325ULL;
  D.clear();
}

static FPClassTest T(unsigned v) { return static_cast<FPClassTest>(v); }

int main(int argc, char **argv) {
  if (argc > 1 && std::string(argv[1]) == "--version") {
    printf("LLVM %s\n", LLVM_VERSION_STRING);
    return 0;
  }
  H = 0xcbf29ce484222325ULL;

  auto unary = [&](const char *name, auto fn) {
    for (unsigned m = 0; m < 1024; ++m) {
      hbegin();
      fn(T(m));
      hend();
    }
    line(name);
  };
  unary("fneg", [](FPClassTest m) { hadd(fneg(m)); });
  unary("inverse_fabs", [](FPClassTest m) { hadd(inverse_fabs(m)); });
  unary("unknown_sign", [](FPClassTest m) { hadd(unknown_sign(m)); });
  unary("not", [](FPClassTest m) { hadd(~m); });
  unary("print", [](FPClassTest m) {
    std::string s;
    raw_string_ostream OS(s);
    OS << m;
    hstr(s);
  });
  for (int z = 0; z < 2; ++z) {
    std::string a = "orderedStrictlyLess" + std::to_string(z), b = "orderedStrictlyGreater" + std::to_string(z);
    unary(a.c_str(), [&](FPClassTest m) { hadd(orderedStrictlyLess(m, z)); });
    unary(b.c_str(), [&](FPClassTest m) { hadd(orderedStrictlyGreater(m, z)); });
  }
  for (int f = 0; f < 2; ++f) {
    std::string a = "invertIfSimpler" + std::to_string(f);
    unary(a.c_str(), [&](FPClassTest m) { hadd(invertFPClassTestIfSimpler(m, f)); });
  }
  for (int z = 0; z < 2; ++z) {
    struct Op {
      const char *name;
      bool (*fn)(FPClassTest, FPClassTest, bool);
    } ops[] = {{"cannotOrderStrictlyGreater", cannotOrderStrictlyGreater},
               {"cannotOrderStrictlyGreaterEq", cannotOrderStrictlyGreaterEq},
               {"cannotOrderStrictlyLess", cannotOrderStrictlyLess},
               {"cannotOrderStrictlyLessEq", cannotOrderStrictlyLessEq}};
    for (const Op &op : ops) {
      for (unsigned l = 0; l < 1024; ++l)
        for (unsigned r = 0; r < 1024; ++r) {
          hbegin();
          hadd(op.fn(T(l), T(r), z));
          hend();
        }
      std::string name = std::string(op.name) + std::to_string(z);
      line(name.c_str());
    }
  }

  // Rounding modes and denormal modes.
  for (int r = -1; r < 8; ++r) {
    hbegin();
    hstr(spell(static_cast<RoundingMode>(r)).str());
    hend();
  }
  line("spell");
  std::vector<DenormalMode> modes;
  for (int o = -1; o < 4; ++o)
    for (int i = -1; i < 4; ++i)
      modes.push_back(DenormalMode(static_cast<DenormalMode::DenormalModeKind>(o),
                                   static_cast<DenormalMode::DenormalModeKind>(i)));
  for (const DenormalMode &m : modes) {
    hbegin();
    hadd(m.Output + 1); hadd(m.Input + 1); hadd(m.isSimple()); hadd(m.isValid()); hadd(m.inputsAreZero());
    hadd(m.inputsMayBeZero()); hadd(m.outputsAreZero()); hadd(m.outputsMayBeZero());
    hadd(m.inputsMayBePositiveZero()); hadd(m.outputsMayBePositiveZero());
    if (m.isValid()) hadd(m.toIntValue());
    hstr(m.str());
    std::string s;
    for (int legacy = 0; legacy < 2; ++legacy)
      for (int omit = 0; omit < 2; ++omit) {
        s.clear();
        raw_string_ostream OS(s);
        m.print(OS, legacy, omit);
        OS.flush();
        hstr(s);
      }
    hend();
  }
  line("denormalMode");
  for (const DenormalMode &a : modes)
    for (const DenormalMode &b : modes) {
      hbegin();
      DenormalMode c = a.mergeCalleeMode(b);
      hadd(c.Output + 1); hadd(c.Input + 1); hadd(a == b); hadd(a != b);
      hend();
    }
  line("merge");
  for (unsigned i = 0; i < 16; ++i) {
    hbegin();
    DenormalMode m = DenormalMode::createFromIntValue(i);
    hadd(m.Output + 1); hadd(m.Input + 1);
    hend();
  }
  line("createFromIntValue");
  const char *names[] = {"", "ieee", "preservesign", "preserve-sign", "positivezero", "positive-zero", "dynamic",
                         "bogus", "ieee,ieee", "dynamic,ieee", "preserve-sign,positive-zero", "ieee,bogus", "bogus,ieee",
                         ",", "ieee,"};
  for (const char *n : names) {
    hbegin();
    DenormalMode m = parseDenormalFPAttribute(n);
    hadd(m.Output + 1); hadd(m.Input + 1);
    hadd(parseDenormalFPAttributeComponent(n) + 1);
    hend();
  }
  line("parse");
  for (int k = -1; k < 4; ++k)
    for (int legacy = 0; legacy < 2; ++legacy) {
      hbegin();
      hstr(denormalModeKindName(static_cast<DenormalMode::DenormalModeKind>(k), legacy).str());
      hend();
    }
  line("kindName");

  // DenormalFPEnv over the valid modes and the "invalid float mode" that takes the default's.
  std::vector<DenormalMode> envModes;
  for (const DenormalMode &m : modes)
    if (m.isValid() || (m.Output == DenormalMode::Invalid && m.Input == DenormalMode::Invalid)) envModes.push_back(m);
  for (const DenormalMode &base : envModes) {
    if (!base.isValid()) continue;
    for (const DenormalMode &fm : envModes) {
      hbegin();
      DenormalFPEnv e(base, fm);
      hadd(e.DefaultMode.Output + 1); hadd(e.DefaultMode.Input + 1);
      hadd(e.F32Mode.Output + 1); hadd(e.F32Mode.Input + 1);
      hadd(e.toIntValue());
      DenormalFPEnv back = DenormalFPEnv::createFromIntValue(e.toIntValue());
      hadd(back == e);
      for (int omit = 0; omit < 2; ++omit) {
        std::string s;
        raw_string_ostream OS(s);
        e.print(OS, omit);
        OS.flush();
        hstr(s);
      }
      DenormalFPEnv other(fm.isValid() ? fm : base, base);
      DenormalFPEnv merged = e.mergeCalleeMode(other);
      hadd(merged.DefaultMode.Output + 1); hadd(merged.DefaultMode.Input + 1);
      hadd(merged.F32Mode.Output + 1); hadd(merged.F32Mode.Input + 1);
      hend();
    }
  }
  line("denormalFPEnv");
  hbegin();
  DenormalFPEnv d = DenormalFPEnv::getDefault();
  hadd(d.toIntValue());
  hend();
  line("envDefault");
  return 0;
}

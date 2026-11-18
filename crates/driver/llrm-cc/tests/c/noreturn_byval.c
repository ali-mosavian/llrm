typedef struct { unsigned char code; int at, line, user[4]; } Failure;
typedef struct Fault { void (*raise)(struct Fault *, const Failure *); } Fault;

[[noreturn]] static inline void raise_it(Fault *f, const Failure e) {
  f->raise(f, &e);
  __builtin_unreachable();
}
[[noreturn, gnu::cold, gnu::noinline]] void fault_error(Fault *f, unsigned char code) {
  raise_it(f, (Failure){.code = code, .at = -1, .line = -1});
}

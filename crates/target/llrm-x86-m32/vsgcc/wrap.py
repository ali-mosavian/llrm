"""wrap.py PROG SOURCE: SOURCE with the kernel `bench_PROG` called from `main` through a volatile pointer.

Every compiler then keeps the kernel a call (nothing can inline it into `main` or clone it for `main`'s constants) and still
inlines everything else, so the table measures inlining. `__attribute__((noinline))` does the same for gcc and clang but llrm-c's
front end (Watcom's) does not take it.
"""
import re
import sys


def wrapped(program: str, source: str) -> str:
    kernel = f"bench_{program}"
    definition = re.search(rf"^([a-z][a-z ]*[ *])({kernel})\(([^)]*)\)", source, re.M)
    main = re.search(r"^int\s+main\s*\(", source, re.M)
    if definition is None or main is None:
        raise SystemExit(f"wrap.py: no `{kernel}` definition and `main` in the source")
    via = f"{kernel}_via"
    before, after = source[: main.start()], source[main.start():]
    after = after.replace(f"{kernel}(", f"(*{via})(")
    return f"{before}{definition.group(1).rstrip()} (*volatile {via})({definition.group(3)}) = {kernel};\n{after}"


if __name__ == "__main__":
    print(wrapped(sys.argv[1], open(sys.argv[2]).read()), end="")

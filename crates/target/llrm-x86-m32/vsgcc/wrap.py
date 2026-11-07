"""wrap.py PROG SOURCE: SOURCE with the kernel `bench_PROG` called from `main` through a volatile pointer.

Every compiler then keeps the kernel a call (nothing can inline it into `main` or clone it for `main`'s constants) and still
inlines everything else, so the table measures inlining. `__attribute__((noinline))` does the same for gcc and clang but llrm-cc's
front end (Watcom's) does not take it.
"""
import re
import sys


def wrapped(program: str, source: str) -> str:
    kernel = f"bench_{program}"
    if program.startswith("run_"):
        return wrapped_run(program, source)
    definition = re.search(rf"^([a-z][a-z ]*[ *])({kernel})\(([^)]*)\)", source, re.M)
    main = re.search(r"^int\s+main\s*\(", source, re.M)
    if definition is None or main is None:
        raise SystemExit(f"wrap.py: no `{kernel}` definition and `main` in the source")
    via = f"{kernel}_via"
    before, after = source[: main.start()], source[main.start():]
    after = after.replace(f"{kernel}(", f"(*{via})(")
    return f"{before}{definition.group(1).rstrip()} (*volatile {via})({definition.group(3)}) = {kernel};\n{after}"


def wrapped_run(program: str, source: str) -> str:
    """A tests/run/c cell has only `main`: it becomes the kernel `bench_run_NAME`, and a new `main` calls it through the pointer."""
    main = re.search(r"^int\s+main\s*\(([^)]*)\)", source, re.M)
    if main is None or main.group(1).strip() not in ("", "void"):
        raise SystemExit("wrap.py: a tests/run/c cell needs `int main(void)`")
    kernel = f"bench_{program}"
    body = source[: main.start()] + f"int {kernel}(void)" + source[main.end():]
    return f"{body}\nint (*volatile {kernel}_via)(void) = {kernel};\nint main(void)\n{{\n    return (*{kernel}_via)();\n}}\n"


if __name__ == "__main__":
    print(wrapped(sys.argv[1], open(sys.argv[2]).read()), end="")

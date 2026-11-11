"""The files a bench program opens, and the emulation of the file API stub.s holds (bench_io_shim.h, grep's `input_read`).

A program's `// data:` line names them as tools/m68k_exec_sweep.py reads it: `@signal` (SIGNAL, generated), `@dickens` (standard input,
the first `ci_bytes` of bench.toml: the cut the default bench times, with `NAME.ci.out` as its answer), else a path relative to the source.
"""
import hashlib
import re
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
STDOUT = 1
FIRST_HANDLE = 3
CALLS = ("input_read", "vs_open_read", "vs_create", "vs_read", "vs_write", "vs_close")


def inputs(source: Path) -> tuple[dict[str, bytes], bytes | None]:
    """(files by the name the program opens, bytes of standard input) from the `// data:` line of SOURCE."""
    found = re.search(r"^//\s*data:\s*(\S+)", source.read_text(), re.M)
    if not found:
        return {}, None
    sys.path.insert(0, str(REPO / "tools/dosbatch"))
    import corpus
    what = found[1]
    if what == "@signal":
        return {"SIGNAL": corpus.path("signal").read_bytes()}, None
    if what == "@dickens":
        cut = tomllib.loads((source.parent / "bench.toml").read_text())["ci_bytes"]
        return {}, corpus.path("dickens").read_bytes()[:cut]
    target = (source.parent / what).resolve()
    return {target.name.upper(): target.read_bytes()}, None


def uses_files(source: Path) -> bool:
    return bool(re.search(r"^//\s*data:", source.read_text(), re.M))


def answer_name(prog: str, directory: Path) -> str:
    """The `.out` file a run is checked against: `NAME.ci.out` for a program fed a cut of its corpus."""
    toml = directory / "bench.toml"
    settings = tomllib.loads(toml.read_text()) if toml.is_file() else {}
    return f"{prog}.ci.out" if settings.get("ci_bytes") else f"{prog}.out"


class Files:
    """The program's file system: reads from `files` and `stdin`, writes kept (md5 per name), a miss is a handle below zero."""

    def __init__(self, files: dict[str, bytes], stdin: bytes | None):
        self.files, self.stdin, self.at = dict(files), stdin, 0
        self.open: dict[int, list] = {}   # handle -> [name, position, writing]
        self.written: dict[str, bytearray] = {}
        self.stdout = bytearray()
        self.opens = self.failed = self.read_bytes = self.calls = 0

    def cstring(self, uc, address: int) -> str:
        out = bytearray()
        while (b := bytes(uc.mem_read(address + len(out), 1))) != b"\0":
            out += b
        return out.decode()

    def call(self, uc, name: str, args: list[int]) -> int:
        """Run one stub call; the value for EAX. ARGS are the stack words after the return address."""
        self.calls += 1
        if name == "input_read":
            chunk = (self.stdin or b"")[self.at:self.at + args[1]]
            self.at += len(chunk)
            uc.mem_write(args[0], chunk)
            self.read_bytes += len(chunk)
            return len(chunk)
        if name in ("vs_open_read", "vs_create"):
            self.opens += 1
            wanted = self.cstring(uc, args[0])
            if name == "vs_open_read" and wanted not in self.files:
                self.failed += 1
                return -1 & 0xFFFFFFFF
            handle = FIRST_HANDLE + len(self.open) + self.opens
            self.open[handle] = [wanted, 0, name == "vs_create"]
            if name == "vs_create":
                self.written[wanted] = bytearray()
            return handle
        if name == "vs_close":
            return 0 if self.open.pop(args[0], None) is not None else 0xFFFFFFFF
        if name == "vs_read":
            entry = self.open.get(args[0])
            if entry is None or entry[2]:
                return 0xFFFFFFFF
            chunk = self.files[entry[0]][entry[1]:entry[1] + args[2]]
            entry[1] += len(chunk)
            uc.mem_write(args[1], chunk)
            self.read_bytes += len(chunk)
            return len(chunk)
        if name == "vs_write":
            data = bytes(uc.mem_read(args[1], args[2]))
            if args[0] == STDOUT:
                self.stdout += data
                return len(data)
            entry = self.open.get(args[0])
            if entry is None or not entry[2]:
                return 0xFFFFFFFF
            self.written[entry[0]] += data
            return len(data)
        raise AssertionError(name)

    def summary(self) -> dict:
        return dict(calls=self.calls, opens=self.opens, failed_opens=self.failed, read=self.read_bytes,
                    written={k: (len(v), hashlib.md5(v).hexdigest()[:12]) for k, v in sorted(self.written.items())}, stdout=len(self.stdout))

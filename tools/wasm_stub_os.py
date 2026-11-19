#!/usr/bin/env python3
"""wasm_stub_os.py FILE.c: C source of the `llrm_os_*` layer for the wasm_corpus.sh stub, with the program's `// data:` file in
memory (bench/<prog>/<path>, or `@signal`: tools/dosbatch/signal_gen), so a program that reads PHOTO.JPG or SIGNAL runs instead
of trapping on an unresolved import. Both compilers get the same bytes; what a program writes is folded into the stub's `report`
hash at close, so the rows compare its output. Prints nothing for a program that does not use the layer."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def data_of(source: Path):
    text = source.read_text()
    if not re.search(r"\b(llrm_os_|bench_open_read|bench_create)", text):
        return None
    found = re.search(r"^//\s*data:\s*(\S+)", text, re.M)
    if not found:
        return ("", b"")  # the layer, no input file
    what = found[1]
    if what == "@signal":
        sys.path.insert(0, str(ROOT / "tools" / "dosbatch"))
        import signal_gen

        return ("SIGNAL", signal_gen.signal())
    if what.startswith("@"):
        return None  # another corpus (grep's @dickens): the stub's input_read serves it
    path = (source.parent / what).resolve()
    return (path.name.upper(), path.read_bytes())


def render(source: Path) -> str:
    got = data_of(source)
    if got is None:
        return ""
    name, data = got
    body = ",".join(str(byte) for byte in data) or "0"
    return f"""
#include <stddef.h>
#include <stdint.h>
/* The OS layer of wasm_corpus.sh: one input file in memory, writes hashed into `report` at close. */
static const unsigned char os_file[] = {{{body}}};
static const char os_name[] = "{name}";
static size_t os_size = {len(data)}, os_pos;
static unsigned long os_out;
static int os_equal(const char *a, const char *b)
{{
    for (; *a && *b; ++a, ++b)
        if ((*a | 32) != (*b | 32)) return 0;
    return *a == *b;
}}
int llrm_os_open(const char *name, unsigned char mode)
{{
    (void)mode;
    os_pos = 0;
    return os_name[0] && os_equal(name, os_name) ? 3 : -1;
}}
int llrm_os_create(const char *name)
{{
    (void)name;
    os_out = 0;
    return 4;
}}
int32_t llrm_os_read(int handle, unsigned char *data, size_t count)
{{
    size_t n = os_size - os_pos < count ? os_size - os_pos : count;
    (void)handle;
    for (size_t i = 0; i < n; ++i) data[i] = os_file[os_pos + i];
    os_pos += n;
    return (int32_t)n;
}}
int32_t llrm_os_seek(int handle, int32_t offset, unsigned char whence)
{{
    (void)handle;
    os_pos = whence == 0 ? (size_t)offset : whence == 1 ? os_pos + (size_t)offset : os_size + (size_t)offset;
    return (int32_t)os_pos;
}}
int32_t llrm_os_write_file(int handle, const unsigned char *data, size_t count)
{{
    (void)handle;
    for (size_t i = 0; i < count; ++i) os_out = os_out * 131u + data[i];
    return (int32_t)count;
}}
intptr_t llrm_os_close(int handle)
{{
    if (handle == 4) report((long)os_out);
    return 0;
}}
"""


if __name__ == "__main__":
    sys.stdout.write(render(Path(sys.argv[1])))

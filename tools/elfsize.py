"""Code bytes of an ELF object from `size -A`: every `.text` and `.text.*` section (`.text.startup` holds `main`, `.text.unlikely` and `.text.hot` the split cold and hot parts)."""


def is_text(name: str) -> bool:
    return name == ".text" or name.startswith(".text.")


def text_bytes(size_output: str) -> int:
    rows = (line.split() for line in size_output.splitlines())
    return sum(int(row[1]) for row in rows if len(row) > 1 and is_text(row[0]))


SHF_ALLOC, SHF_EXECINSTR = 0x2, 0x4
SHT_NOBITS = 8


def sections(body: bytes) -> list[tuple[str, int, int, int]]:
    """(name, type, flags, size) of each section of a little-endian ELF64 object, from its section headers."""
    import struct

    shoff = struct.unpack_from("<Q", body, 0x28)[0]
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", body, 0x3A)
    headers = [struct.unpack_from("<IIQQQQIIQQ", body, shoff + i * shentsize) for i in range(shnum)]
    names = headers[shstrndx][4]

    def name(at: int) -> str:
        return body[names + at : body.index(b"\0", names + at)].decode()

    return [(name(h[0]), h[1], h[2], h[5]) for h in headers]


def code_bytes(path) -> int:
    """Bytes of the object's executable sections (SHF_EXECINSTR): the program's code, whatever the section is called. Not `.eh_frame`."""
    with open(path, "rb") as file:
        return sum(size for _, _, flags, size in sections(file.read()) if flags & SHF_EXECINSTR)


def data_bytes(path) -> tuple[int, int]:
    """(initialised, uninitialised) bytes of the object's allocated non-executable sections, without `.eh_frame` (unwind tables and the `.note*` the toolchain adds are not the program's data)."""
    with open(path, "rb") as file:
        found = [s for s in sections(file.read()) if s[2] & SHF_ALLOC and not s[2] & SHF_EXECINSTR and s[0] != ".eh_frame" and not s[0].startswith(".note")]
    return sum(size for _, kind, _, size in found if kind != SHT_NOBITS), sum(size for _, kind, _, size in found if kind == SHT_NOBITS)

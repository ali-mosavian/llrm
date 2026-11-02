"""Code bytes of an ELF object from `size -A`: every `.text` and `.text.*` section (`.text.startup` holds `main`, `.text.unlikely` and `.text.hot` the split cold and hot parts)."""


def is_text(name: str) -> bool:
    return name == ".text" or name.startswith(".text.")


def text_bytes(size_output: str) -> int:
    rows = (line.split() for line in size_output.splitlines())
    return sum(int(row[1]) for row in rows if len(row) > 1 and is_text(row[0]))


SHF_ALLOC, SHF_EXECINSTR = 0x2, 0x4
SHT_NOBITS = 8


def headers(body: bytes) -> tuple[list[tuple[int, int, int, int, int]], int, int]:
    """((name offset, type, flags, file offset, size) of each section header, the index of the name table, address bits) of a
    little-endian ELF32 or ELF64 object."""
    import struct

    if body[4] == 1:  # ELFCLASS32
        shoff = struct.unpack_from("<I", body, 0x20)[0]
        shentsize, shnum, shstrndx = struct.unpack_from("<HHH", body, 0x2E)
        rows = [struct.unpack_from("<IIIIIIIIII", body, shoff + i * shentsize) for i in range(shnum)]
        return [(h[0], h[1], h[2], h[4], h[5]) for h in rows], shstrndx, 32
    shoff = struct.unpack_from("<Q", body, 0x28)[0]
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", body, 0x3A)
    rows = [struct.unpack_from("<IIQQQQIIQQ", body, shoff + i * shentsize) for i in range(shnum)]
    return [(h[0], h[1], h[2], h[4], h[5]) for h in rows], shstrndx, 64


def sections(body: bytes) -> list[tuple[str, int, int, int]]:
    """(name, type, flags, size) of each section of a little-endian ELF32 or ELF64 object, from its section headers."""
    rows, shstrndx, _ = headers(body)
    names = rows[shstrndx][3]

    def name(at: int) -> str:
        return body[names + at : body.index(b"\0", names + at)].decode()

    return [(name(h[0]), h[1], h[2], h[4]) for h in rows]


def executable_sections(path) -> list[tuple[bytes, int]]:
    """(bytes, address bits) of each executable section (SHF_EXECINSTR) of an ELF object."""
    with open(path, "rb") as file:
        body = file.read()
    rows, _, bits = headers(body)
    return [(body[h[3] : h[3] + h[4]], bits) for h in rows if h[2] & SHF_EXECINSTR and h[1] != SHT_NOBITS]


def executable_contents(path) -> list[bytes]:
    """The bytes of each executable section of an ELF object (SHF_EXECINSTR)."""
    return [content for content, _ in executable_sections(path)]


def code_size(path, padding) -> tuple[int, int, int]:
    """(code bytes without alignment padding, the padding, bytes that start no instruction) of the object's executable sections; `padding(content, bits)` is sizes.padding,
    each section read from its own start (its alignment is the section's) as 32- or 64-bit code, whichever the object is."""
    code = align = unread = 0
    for content, bits in executable_sections(path):
        found, skipped = padding(content, bits)
        code += len(content) - found
        align += found
        unread += skipped
    return code, align, unread


def code_bytes(path) -> int:
    """Bytes of the object's executable sections (SHF_EXECINSTR): the program's code, whatever the section is called. Not `.eh_frame`."""
    with open(path, "rb") as file:
        return sum(size for _, _, flags, size in sections(file.read()) if flags & SHF_EXECINSTR)


def data_bytes(path) -> tuple[int, int]:
    """(initialised, uninitialised) bytes of the object's allocated non-executable sections, without `.eh_frame` (unwind tables and the `.note*` the toolchain adds are not the program's data)."""
    with open(path, "rb") as file:
        found = [s for s in sections(file.read()) if s[2] & SHF_ALLOC and not s[2] & SHF_EXECINSTR and s[0] != ".eh_frame" and not s[0].startswith(".note")]
    return sum(size for _, kind, _, size in found if kind != SHT_NOBITS), sum(size for _, kind, _, size in found if kind == SHT_NOBITS)

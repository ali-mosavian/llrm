"""Code bytes of an ELF object from `size -A`: every `.text` and `.text.*` section (`.text.startup` holds `main`, `.text.unlikely` and `.text.hot` the split cold and hot parts)."""


def is_text(name: str) -> bool:
    return name == ".text" or name.startswith(".text.")


def text_bytes(size_output: str) -> int:
    rows = (line.split() for line in size_output.splitlines())
    return sum(int(row[1]) for row in rows if len(row) > 1 and is_text(row[0]))

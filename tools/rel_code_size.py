"""Code bytes of an sdas `.rel` object: the `A <area> size <hex>` lines, not the file's length."""
import re


def rel_code_size(path):
    total = 0
    for line in open(path, errors="replace"):
        m = re.match(r"^A\s+(\S+)\s+size\s+([0-9A-Fa-f]+)\s", line)
        if m and m.group(1).startswith("_CODE"):
            total += int(m.group(2), 16)
    return total

"""wasm_cells.py BASE NEW: compare two `tools/wasm_corpus.sh` runs (one "## -LEVEL" header, then the table, per level).

A cell is WORSE when its code bytes or its instructions executed grew, BROKEN when it compared in BASE and not in NEW.
Exit 1 on either; the last line says how many cells were compared of how many.
"""
import sys


def read(path):
    cells, level = {}, None
    for line in open(path):
        if line.startswith("## "):
            level = line[3:].strip()
            continue
        fields = line.split()
        if len(fields) >= 6 and fields[1].isdigit() and fields[3].isdigit():
            cells[(level, fields[0])] = (int(fields[1]), int(fields[3]))
        elif len(fields) > 1 and "FAIL" in line:
            cells[(level, fields[0])] = None
    return cells


def main(base_path, new_path):
    base, new = read(base_path), read(new_path)
    bad, compared = [], 0
    for key in sorted(base):
        if base[key] is None:
            continue
        if new.get(key) is None:
            bad.append(f"BROKEN {key[0]} {key[1]}")
            continue
        compared += 1
        (bytes_b, instrs_b), (bytes_n, instrs_n) = base[key], new[key]
        if bytes_n > bytes_b or instrs_n > instrs_b:
            bad.append(f"WORSE {key[0]} {key[1]} bytes {bytes_b} -> {bytes_n}, instrs {instrs_b} -> {instrs_n}")
    print("\n".join(bad))
    print(f"compared {compared} of {len(base)} cells")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:3]))

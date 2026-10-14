#!/usr/bin/env python3
"""Exit 0 when a doc comment under the given roots holds a code fence, or an indented block, that rustdoc would run as a test, else 1.

`cargo test --doc` runs rustdoc over every crate (456 CPU-s of a 4,340 CPU-s gate) and the tree has no doctest: its fences are all
```text. The gate runs that step only when this finds one."""
import re
import sys
from pathlib import Path

# An info string with none of these names a language rustdoc leaves alone (text, sh, c, ...).
RUNS = {"", "rust", "ignore", "no_run", "should_panic", "compile_fail", "test_harness"}
FENCE = re.compile(r"^\s*//[/!]\s?(\s*)```(.*)$")


def runs(info: str) -> bool:
    tokens = [t.strip() for t in re.split(r"[,\s]+", info.strip()) if t.strip()]
    return all(t in RUNS or t.startswith("edition") for t in tokens)


DOC = re.compile(r"^\s*//[/!] ?(.*)$")
ITEM = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s")


def has_doctests(roots: list[Path]) -> bool:
    for root in roots:
        for path in root.rglob("*.rs"):
            if "target" in path.parts:
                continue
            text = path.read_text(errors="replace")
            if re.search(r"doc\s*=\s*include_str!", text):
                return True
            inside = blank = listed = False
            for line in text.splitlines():
                doc = DOC.match(line)
                if not doc:
                    inside = blank = listed = False
                    continue
                body = doc.group(1)
                match = FENCE.match(line)
                if match:
                    blank = False
                    if inside:
                        inside = False
                    else:
                        inside = True
                        if runs(match.group(2)):
                            return True
                    continue
                if inside:
                    continue
                if not body.strip():
                    blank = True
                    continue
                # Four spaces after a blank line, outside a list item, is a code block rustdoc compiles as Rust (#738).
                if blank and not listed and re.match(r" {4}\S", body):
                    return True
                listed = bool(ITEM.match(body)) or (listed and bool(re.match(r"\s+\S", body)))
                blank = False
    return False


if __name__ == "__main__":
    sys.exit(0 if has_doctests([Path(p) for p in sys.argv[1:]] or [Path(".")]) else 1)

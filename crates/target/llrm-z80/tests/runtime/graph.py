#!/usr/bin/env python3
"""The byte counts of the corpus rows charge a program for the runtime routines it reaches. usage: graph.py z80_rt.s

`__modsi3` falls into `__udivmodsi4` and `__udivdi3` jumps into `udivmoddi` under `__moddi3`: both were missing from
the graph, so a program that took a long `%` was counted without the 1.3 KB divide (loop_interchange: 1362 B both
before and after the divide was rewritten)."""
import os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
src = open(os.path.join(HERE, "..", "kernels", "measure.py")).read().split("def main():")[0]
ns = {"__file__": os.path.join(HERE, "..", "kernels", "measure.py"), "__name__": "m"}
exec(src, ns)
graph = ns["runtime_calls"](sys.argv[1])
ok = True
for routine, reached in (
    ("__modsi3", "__udivmodsi4"),  # falls through
    ("__umodsi3", "__udivmodsi4"),  # jr
    ("__udivdi3", "__moddi3"),  # jumps to a local label that sits under __moddi3
    ("__divdi3", "__moddi3"),
    ("__ashldi3", "__ashrdi3"),
):
    if reached not in graph.get(routine, ()):
        print(f"{routine} does not reach {reached}: {sorted(graph.get(routine, ()))}")
        ok = False
if "__memmove" in graph.get("__memcpy", ()):
    print("__memcpy ends in ret and does not reach __memmove")
    ok = False
# the link map cuts a name at 9 characters: `__udivmodsi4` is `__udivmod` there, and its 388 B were not counted
import tempfile
with tempfile.TemporaryDirectory() as w:
    open(f"{w}/x.map", "w").write("00000786  __umodsi3 |    0000078E  __modsi3 |    00000790  __udivmod |    00000914  __udivdi3\n")
    size = ns["runtime_bytes"](w, {"__modsi3": {"__udivmodsi4"}}, "call __modsi3")
    if size != (0x790 - 0x78E) + (0x914 - 0x790):
        print(f"__modsi3 counted as {size} B, the divide under its truncated map name is missing")
        ok = False
print("ALL OK" if ok else "FAILED")
sys.exit(0 if ok else 1)

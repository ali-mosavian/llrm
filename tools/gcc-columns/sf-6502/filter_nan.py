"""usage: filter_nan.py : rewrite vec.inc without the records whose first input is NaN; print how many were dropped.
The 6502 config leaves NaN to int undefined (SF_FIX_NAN_UNDEF in config/6502/config.h), so such records have no expected value."""
import re

out, drop = [], 0
for l in open("vec.inc").read().split("\n"):
    m = re.match(r"\s*\.dword\s+\$([0-9a-f]{8})", l)
    if m and (int(m.group(1), 16) << 1) & 0xFFFFFFFF > 0xFF000000:
        drop += 1
        continue
    out.append(l)
n = sum(1 for l in out if l.strip().startswith(".dword"))
open("vec.inc", "w").write("\n".join(re.sub(r"^nrec = \d+", f"nrec = {n}", l) for l in out) + "\n")
print(drop)

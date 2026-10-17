"""No hashed collection of the compiler is keyed by SipHash.

llrm-mir's own `HashMap` was `BuildHasherDefault<DefaultHasher>` (fixed keys for a stable order, but SipHash): 1% of a
d_faces compile in hashing alone. Its stable-order replacement is Fx; a collection built on `DefaultHasher` or
`RandomState` again would bring both costs back.
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SIPHASHED = re.compile(r"BuildHasherDefault<\s*(?:[\w:]*::)?DefaultHasher\s*>|\bRandomState\b|HashMap<[^;]*,\s*DefaultHasher")


def test_no_hash_map_or_set_is_keyed_by_siphash():
    found = []
    for source in sorted((ROOT / "crates").rglob("*.rs")):
        if "target" in source.relative_to(ROOT).parts:
            continue
        for number, line in enumerate(source.read_text().splitlines(), 1):
            if SIPHASHED.search(line) and not line.lstrip().startswith("//"):
                found.append(f"{source.relative_to(ROOT)}:{number}: {line.strip()}")
    assert not found, "SipHash-keyed collections:\n" + "\n".join(found)

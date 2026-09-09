# Constants across runtime calls

Constant propagation now consults raised write effects for established calls,
instead of discarding all memory facts solely because the call can write.
Unknown calls, barriers and writing calls without effects still clear facts.

The existing `MemRef.beyond` escape set names addresses, not object extents.
It cannot prove that a byte fragment above an escaped address is unreachable.
The first implementation retained bytes 1–3 of an escaped long after a possible
write; the fail-first regression in `tests/test_constant_call_memory.py` catches
each byte. Constant propagation therefore ignores nonempty escape exclusions
until the raise supplies trustworthy extents. Empty escape sets remain usable.

Before: even a proven no-escape call discarded a complete constant.
After: that constant survives; an escaped long loses every byte fact.
The PDS/G2 fixture emission comparison against HEAD showed no changed objects.
This is not a measured speedup, and the experimental NOTS/NEGNOT improvements
from trusting nonempty escape exclusions are withdrawn.

Follow-up: supply object-range reachability in the raise, not machine knowledge
in constant propagation. Do not infer object ends from individual operand
addresses: a reference can name a field or the high half of the same object.

Focused verification also found the three compiler variants of
`test_spill_uses_the_known_seven_as_an_immediate` failing with both HEAD's and
the revised constant invalidation. They remain unchanged and unresolved.

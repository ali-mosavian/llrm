"""Every family of cases, by name."""

from __future__ import annotations

from cases import classics, concurrent

FAMILIES = {
    "classic": lambda quick: classics.cases(),
    "concurrent": concurrent.cases,
}
QUICK = ("classic", "concurrent")


def load(names: list[str] | None, quick: bool = False) -> list:
    chosen = names or (QUICK if quick else list(FAMILIES))
    out = []
    for name in chosen:
        out += FAMILIES[name](quick)
    seen = set()
    for case in out:
        if case.name in seen:
            raise ValueError(f"two cases named {case.name}")
        seen.add(case.name)
    return out

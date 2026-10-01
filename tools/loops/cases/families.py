"""Every family of cases, by name."""

from __future__ import annotations

from cases import classics, concurrent, cross, fuzz, adversarial

FAMILIES = {
    "classic": lambda quick: classics.cases(),
    "concurrent": concurrent.cases,
    "cross": cross.cases,
    "fuzz": fuzz.cases,
    "adversarial": adversarial.cases,
}
SEED = fuzz.SEED
QUICK = ("classic", "concurrent", "cross", "adversarial")


def load(names: list[str] | None, quick: bool = False, seed: int = SEED) -> list:
    chosen = names or (QUICK if quick else list(FAMILIES))
    out = []
    for name in chosen:
        out += FAMILIES[name](quick) if name != "fuzz" else fuzz.cases(quick, seed)
    seen = set()
    for case in out:
        if case.name in seen:
            raise ValueError(f"two cases named {case.name}")
        seen.add(case.name)
    return out

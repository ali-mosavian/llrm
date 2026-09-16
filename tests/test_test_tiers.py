"""The fast test command must not grow into an accidental corpus run."""

from typing import cast
from pathlib import Path
from types import SimpleNamespace
from collections.abc import Iterator

import pytest
from conftest import tier_of
from conftest import FAST_MODULES
from conftest import pytest_ignore_collect


class Item:
    def __init__(
        self,
        *,
        markers: tuple[str, ...] = (),
        fixtures: tuple[str, ...] = (),
        params: dict[str, object] | None = None,
        module: str = "test_sccp.py",
    ) -> None:
        self._markers = [SimpleNamespace(name=name) for name in markers]
        self.fixturenames = fixtures
        self.path = Path("tests") / module
        if params is not None:
            self.callspec = SimpleNamespace(params=params)

    def iter_markers(self) -> Iterator[SimpleNamespace]:
        return iter(self._markers)


class Config:
    def __init__(self, full: bool) -> None:
        self.full = full

    def getoption(self, name: str) -> bool:
        assert name == "--full"
        return self.full


def _tier(item: Item) -> str:
    return tier_of(cast(pytest.Item, item))


def test_tier_policy_keeps_focused_tests_fast_and_exhaustive_tests_full() -> None:
    """A new corpus parametrization once made the host suite take minutes."""
    assert _tier(Item()) == "fast"
    assert _tier(Item(params={"tag": "p-g2"})) == "fast"
    assert _tier(Item(markers=("corpus",))) == "full"
    assert _tier(Item(markers=("e2e",))) == "full"
    assert _tier(Item(module="test_mir.py")) == "full"
    assert _tier(Item(fixtures=("obj",))) == "full"
    assert _tier(Item(params={"path": Path("fixtures/omf/hotlop-p-g2.obj")})) == "full"


def test_fast_collection_does_not_import_full_modules() -> None:
    """Deselection must happen before a 499-case parametrization is built."""
    fast = Path("tests/test_sccp.py")
    full = Path("tests/test_mir.py")
    assert pytest_ignore_collect(fast, cast(pytest.Config, Config(False))) is False
    assert pytest_ignore_collect(full, cast(pytest.Config, Config(False))) is True
    assert pytest_ignore_collect(full, cast(pytest.Config, Config(True))) is None


def test_fast_manifest_covers_each_compiler_layer() -> None:
    """The time bound must not turn Tier 1 into a parser-only token suite."""
    required = {
        "test_omfwrite.py",  # both frontends and the fresh object writer
        "test_sccp.py",  # MIR optimization
        "test_lower_conditions.py",  # lowering boundary
        "test_parcopy.py",  # allocation
        "test_memory_folding.py",  # final machine folding
    }
    assert required <= FAST_MODULES

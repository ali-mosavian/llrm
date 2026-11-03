from pathlib import Path
from types import SimpleNamespace

import pytest

import run


def args(tmp_path, cache_text):
    build = tmp_path / "build"
    build.mkdir(exist_ok=True)
    (build / "CMakeCache.txt").write_text(cache_text)
    return SimpleNamespace(pin_src=tmp_path / "src", pin_build=build, out=tmp_path / "out", host_triple="x86_64-unknown-linux-gnu")


def test_features_follow_the_pin_build_not_a_guess(tmp_path):
    on = site_values(args(tmp_path, "LLVM_ENABLE_ASSERTIONS:BOOL=ON\nLLVM_TARGETS_TO_BUILD:STRING=X86;M68k\n"))
    assert on["llrm_asserts"] is True and on["llrm_targets"] == ["X86", "M68k"]
    off = site_values(args(tmp_path, "LLVM_ENABLE_ASSERTIONS:BOOL=OFF\nLLVM_TARGETS_TO_BUILD:STRING=X86\n"))
    assert off["llrm_asserts"] is False and off["llrm_targets"] == ["X86"]


def site_values(a):
    config = SimpleNamespace()
    scope = {"config": config, "lit_config": SimpleNamespace(load_config=lambda *_: None)}
    exec(run.site(a, Path("/bin-dir")), scope)
    return vars(config)


def test_a_cache_without_the_keys_is_assertions_off_and_no_targets(tmp_path):
    values = site_values(args(tmp_path, "OTHER:STRING=1\n"))
    assert values["llrm_asserts"] is False and values["llrm_targets"] == []

"""QB tooling refuses a freestanding llrm-cc row by its flags, never by 20 clang errors about PATH_MAX."""

import os
import stat
import sys

import pytest

sys.path.insert(0, os.path.dirname(__file__))
import qbrow  # noqa: E402


def fake_cc(tmp_path, hosted):
    cc = tmp_path / "llrm-cc"
    cc.write_text(f"#!/bin/sh\ncat >/dev/null\necho '#define __STDC_HOSTED__ {hosted}'\necho \"#define ROW $LLRM_CC_TARGET\"\n")
    cc.chmod(cc.stat().st_mode | stat.S_IEXEC)
    return str(cc)


def test_a_freestanding_row_is_refused_with_the_row_error(tmp_path):
    with pytest.raises(SystemExit) as refused:
        qbrow.require_hosted(fake_cc(tmp_path, 0), "arm64-linux")
    assert "wrong row for QB (__STDC_HOSTED__ 0)" in str(refused.value) and "arm64-linux" in str(refused.value)


def test_a_hosted_row_passes(tmp_path):
    qbrow.require_hosted(fake_cc(tmp_path, 1), "arm64-glibc")

import pytest

import x87probe

TABLE = x87probe.EXPECTED.read_text().split("\n")[:-1]


def test_one_perturbed_value_in_the_table_is_named_by_the_verdict():
    """A host FPU that rounds one result differently must show as that line, not as 'all match'."""
    got = list(TABLE)
    got[40] = got[40][:-1] + ("0" if got[40][-1] != "0" else "1")
    summary, bad = x87probe.verdict(got, TABLE)
    assert summary == "1 of 92 lines differ" and bad == [f"line 41: got {got[40]}, table {TABLE[40]}"]


def test_the_recorded_table_is_the_probes_whole_output_and_starts_at_one():
    assert len(TABLE) == 92 and TABLE[0] == "A0 3fff8000000000000000" and x87probe.verdict(TABLE, TABLE)[0] == "all 92 lines match"


@pytest.mark.slow_dos
def test_the_probe_runs_in_the_emulator_and_prints_every_line(tmp_path):
    """Not its values (a different CPU may differ: x87probe.py says so in the log): the program ran to its end and printed each line's tag."""
    got = x87probe.run(tmp_path)
    assert [line.split()[0] for line in got] == [line.split()[0] for line in TABLE]

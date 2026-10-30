"""firstdiff: a step only one build made is where the two bodies part."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import firstdiff  # noqa: E402

BODY = "define i16 @f(i16 %0) {\n  %1 = add i16 %0, 1\n  ret i16 %1\n}\n"


def dump(root: Path, files: dict[str, str]) -> Path:
    for name, text in files.items():
        (root / name).parent.mkdir(parents=True, exist_ok=True)
        (root / name).write_text(text)
    return root


def test_a_step_only_the_new_build_made_is_the_first_difference(tmp_path):
    """The old script compared the base's files only: the new slot's own step was skipped and a later snapshot named the cause."""
    base = dump(tmp_path / "base", {"02/001-r01-sroa@1179.ll": BODY, "02-pipeline.ll": BODY})
    new = dump(
        tmp_path / "new",
        {"02/001-r01-sroa@1179.ll": BODY, "02/002-r01-combine@1464.ll": BODY, "02-pipeline.ll": BODY.replace("add", "sub")},
    )
    stage, why = firstdiff.first_difference(str(base), str(new))
    assert stage == "02/002-r01-combine@1464.ll" and "new build only" in why


def test_value_numbers_do_not_count_and_equal_dumps_agree(tmp_path):
    base = dump(tmp_path / "base", {"01-globalopt.ll": BODY})
    new = dump(tmp_path / "new", {"01-globalopt.ll": BODY.replace("%1", "%7")})
    assert firstdiff.first_difference(str(base), str(new)) is None


def test_the_steps_of_a_run_come_before_its_snapshot(tmp_path):
    base = dump(tmp_path / "base", {"05/001-r01-dead@1.ll": BODY, "05-pipeline.ll": BODY})
    new = dump(tmp_path / "new", {"05/001-r01-dead@1.ll": BODY.replace("add", "sub"), "05-pipeline.ll": BODY.replace("add", "sub")})
    assert firstdiff.first_difference(str(base), str(new))[0] == "05/001-r01-dead@1.ll"

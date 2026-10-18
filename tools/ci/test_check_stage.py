import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_stage  # noqa: E402


def test_a_staged_link_whose_target_was_not_copied_is_named(tmp_path):
    """24 of gcc's links (libasan.so, libgomp.so ...) pointed into /usr/lib and dangled in /opt/llrm-pin: a -fsanitize or -fopenmp link failed there, late."""
    (tmp_path / "lib").mkdir()
    (tmp_path / "lib/libgcc.a").write_text("x")
    (tmp_path / "lib/libgcc.so").symlink_to("libgcc.a")
    (tmp_path / "lib/libasan.so").symlink_to("../../x86_64-linux-gnu/libasan.so.8")
    assert [p.name for p in check_stage.dangling(tmp_path)] == ["libasan.so"]

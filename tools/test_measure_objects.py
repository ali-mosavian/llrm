"""A cost measured with `-o /dev/null` leaves out the encoder and the object write (emit 4 M of 22 M Ir on d_faces). The measuring tools take an object."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402


def test_the_object_placeholder_is_an_obj_path_of_one_length_whatever_the_directory():
    """A longer argv moved an instruction count by 1.6e-4 to 5e-4 (see llrmbin.pinned): the path must not vary between runs."""
    lengths = set()
    for _ in range(3):
        with llrmbin.with_object(["llrm-cc", "-o", llrmbin.OBJECT, "x.c"]) as command:
            path = Path(command[2])
            assert path.suffix == ".obj" and path.parent.is_dir()
            lengths.add(len(command[2]))
    assert lengths == {llrmbin.OBJECT_BYTES}
    assert not path.parent.exists(), "the scratch directory is removed"


def test_a_command_with_no_placeholder_is_left_alone():
    with llrmbin.with_object(["gcc", "-c", "-o", "/dev/null", "x.c"]) as command:
        assert command == ["gcc", "-c", "-o", "/dev/null", "x.c"]

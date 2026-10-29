"""The ladder's generator is imported under a name no other tool has."""
import importlib
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def test_the_ladder_generator_does_not_share_the_name_gen():
    """`import gen` from tools/ladder found tools/intrinsics/gen.py once pytest had collected tools/intrinsics/test_gen.py (one process
    runs the changed tools' tests): `AttributeError: module 'gen' has no attribute 'generate'` refused the runner's python batches
    (5975 5990 6005). tools/ladder/ladder_gen.py is the generator; its importers say so."""
    sys.path.insert(0, str(HERE / "intrinsics"))
    importlib.import_module("gen")
    sys.path.insert(0, str(HERE / "ladder"))
    ladder = importlib.import_module("ladder_gen")
    assert ladder.generate("mem", 4).startswith("void f(")
    for name in ("test_alloc_scaling.py", "ladder/test_ladder.py"):
        text = (HERE / name).read_text()
        assert "\n    import gen\n" not in text and "\nimport gen\n" not in text, f"{name} still imports the name gen"

"""The inventory names the test targets the landing gate never runs; greedy_inline was one and sat red on parity for a day."""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_inventory as inventory  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def artifact(name: str, kind: str, src: str) -> str:
    return json.dumps({"reason": "compiler-artifact", "package_id": "x", "profile": {"test": True}, "executable": f"/t/{name}", "target": {"name": name, "kind": [kind], "src_path": str(ROOT / src)}})


def test_a_root_integration_target_is_never_run_by_the_gate_and_a_scanner_is():
    rows = inventory.targets("\n".join([artifact("greedy_inline", "test", "tests/greedy_inline.rs"), artifact("docs_links", "test", "tests/docs_links.rs"), "not json"]))
    assert [(r["name"], inventory.gate_runs(r, inventory.package_name(r))) for r in rows] == [("greedy_inline", "never"), ("docs_links", "scanner")]


def test_a_lib_target_is_run_by_the_gate():
    rows = inventory.targets(artifact("llrm_codegen", "lib", "crates/codegen/llrm-codegen/src/lib.rs"))
    assert inventory.gate_runs(rows[0], inventory.package_name(rows[0])) == "lib"

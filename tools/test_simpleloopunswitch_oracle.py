"""The unswitch oracle compares what opt and llrm did to a loop, not how the module numbers its metadata, and runs llrm where opt priced the loop."""
import importlib.util
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.argv = ["simpleloopunswitch_oracle.py"]
spec = importlib.util.spec_from_file_location("slu_oracle", HERE / "simpleloopunswitch_oracle.py")
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def function(tbaa):
    return f"define i16 @f(ptr %p) {{\nb0:\n  store i16 1, ptr %p, align 2, !tbaa !{tbaa}\n  ret i16 0\n}}\n"


def canon(tbaa, type_name="int"):
    """@f stored through node `tbaa`, which says `type_name`."""
    body = function(tbaa)
    ir = body + f'\n!{tbaa} = !{{!9, !9, i64 0}}\n!9 = !{{!"{type_name}"}}\n'
    return oracle.canonical(ir, dict(re.findall(r"^!(\d+) = (.*)$", ir, re.M)), body)


def test_a_module_numbering_its_tbaa_nodes_otherwise_is_not_a_change():
    """opt adds the loop id of an unswitched loop, which shifted !tbaa !3 to !5 in demo-oimad's other functions: BENCHMARK was reported unswitched."""
    assert canon(3) == canon(5)


def test_a_different_tbaa_node_is_a_change():
    assert canon(3, "int") != canon(3, "float")


def test_the_target_triple_line_is_not_handed_to_llrm():
    """With the target flags opt prints `target triple`, which llrm-mir refuses: every file was 'missing'."""
    assert "target triple" not in oracle.to_mir('target triple = "i486-unknown-linux-gnu"\ndefine void @f() {\n  ret void\n}\n')


def test_llrm_runs_at_the_target_opt_priced():
    """llrm-mir priced on Neutral while opt priced for i486: three loops were decided differently."""
    command = oracle.llrm_command("x.ll")
    assert command[command.index("--mode") + 1] == oracle.MODE and command[-1] == "x.ll"

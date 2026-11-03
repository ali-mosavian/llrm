"""#6556 was refused with 'no tests ran': test_codebytes.py ran its checks at import and defined no test function, and test_cmp/test_host
called unittest.main() at import, so pytest collected nothing from them. Every test_*.py here must define a pytest test function."""
import ast
import glob
import os
import tempfile


def files_without_tests(directory):
    out = []
    for path in sorted(glob.glob(f"{directory}/test_*.py")):
        tree = ast.parse(open(path).read())
        if not any(isinstance(n, ast.FunctionDef) and n.name.startswith("test_") for n in tree.body):
            out.append(os.path.basename(path))
    return out


def test_every_test_file_here_defines_a_test_function():
    assert files_without_tests(os.path.dirname(os.path.abspath(__file__))) == []


def test_a_script_style_test_file_is_caught():
    with tempfile.TemporaryDirectory() as d:
        open(f"{d}/test_script.py", "w").write("assert 1 + 1 == 2\nprint('PASS')\n")
        assert files_without_tests(d) == ["test_script.py"]

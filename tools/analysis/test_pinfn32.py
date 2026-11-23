"""pinfn32.sh compared the pin llc on IR that still carried SSE after the strip: a Pentium 4 module against an i486 one, a false gap. The guard must refuse such IR."""
import re, subprocess, tempfile, os
script = open(os.path.join(os.path.dirname(__file__), 'pinfn32.sh')).read()
guard = re.search(r"python3 - .*? <<'P' \|\| exit 1\n(.*?)\nP\n", script, re.S).group(1)

def run(ir):
    d = tempfile.mkdtemp(); i, o = d + '/i.ll', d + '/o.ll'
    open(i, 'w').write(ir)
    return subprocess.run(['python3', '-c', guard, i, o], capture_output=True, text=True), o

def test_sse_ir_is_refused():
    r, _ = run('define i32 @f() {\n ret i32 0\n}\n!0 = !{!"target-features", !"+sse2"}\n')
    assert r.returncode != 0 and 'SSE' in r.stderr

def test_i486_ir_passes_and_is_dso_local():
    r, o = run('define i32 @f() {\n ret i32 0\n}\n')
    assert r.returncode == 0 and 'define dso_local i32 @f' in open(o).read()

"""stft_fixed read the little-endian signal file straight into `short`s: on a big-endian target each sample came out byte-swapped and the
checksum was 20496096 (m68k, ours and gcc alike) for the .out's 30825279. A big-endian read is emulated on the host by swapping each pair
in the shim's read, as the memory of a big-endian machine would hold them."""
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools/dosbatch"))
import signal_gen  # noqa: E402

SHIM = r"""
#include <stdio.h>
#include <stdint.h>
static FILE *fs[8]; static int nf;
static intptr_t llrm_os_open(const char *n, int m){ fs[nf]=fopen("SIGNAL","rb"); return fs[nf]?nf++:-1; }
static intptr_t llrm_os_create(const char *n){ fs[nf]=fopen("/dev/null","wb"); return nf++; }
static intptr_t llrm_os_read(int h, unsigned char *b, int c){ int r=(int)fread(b,1,c,fs[h]);
#ifdef EMULATE_BE
  for(int i=0;i+1<r;i+=2){unsigned char t=b[i];b[i]=b[i+1];b[i+1]=t;}
#endif
  return r; }
static intptr_t llrm_os_write_file(int h, const unsigned char *b, int c){ return c; }
static intptr_t llrm_os_close(int h){ return fclose(fs[h]); }
"""
MAIN = '#include <stdio.h>\nvoid report(long v){printf("%ld\\n",v);}\n'


def run(big_endian: bool) -> str:
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        (d / "inc").mkdir()
        (d / "inc/llrm_os.h").write_text(SHIM)
        (d / "h.c").write_text(MAIN)
        (d / "SIGNAL").write_bytes(signal_gen.signal())
        flags = ["-DEMULATE_BE", "-D__BYTE_ORDER__=4321", "-D__ORDER_BIG_ENDIAN__=4321"] if big_endian else []
        subprocess.run(["gcc", "-m32", "-O1", "-w", "-D__INT_MAX__=1", *flags, f"-I{d / 'inc'}", f"-I{ROOT / 'bench'}",
                        str(ROOT / "bench/stft_fixed/stft_fixed.c"), str(d / "h.c"), "-o", str(d / "a")], check=True)
        return subprocess.run([str(d / "a")], cwd=d, capture_output=True, text=True, check=True).stdout.strip()


def test_a_big_endian_read_gives_the_same_checksum_as_the_little_endian_one():
    expected = (ROOT / "bench/stft_fixed/stft_fixed.out").read_text().strip()
    assert run(False) == expected
    assert run(True) == expected

import re, subprocess, sys
# usage: count.py qemu elf  -> executed instruction count (TB trace x TB length)
q, elf = sys.argv[1], sys.argv[2]
objd = 'aarch64' in q   # qemu prints no disassembly for aarch64: count fixed 4-byte insns from the OBJD-T hex
log = elf + '.qlog'
subprocess.run([q, '-d', 'in_asm,exec,nochain', '-D', log, elf], check=False, stdout=subprocess.DEVNULL)
n_tb = {}; cur = None; execs = 0; total = 0
for l in open(log, errors='replace'):
    if l.startswith('IN:'): cur = None
    m = re.match(r'(0x[0-9a-f]+):\s', l)
    if m and not l.startswith('Trace'):
        a = int(m.group(1), 16)
        if cur is None: cur = a; n_tb[cur] = 0
        if not objd: n_tb[cur] += 1
        continue
    if objd and l.startswith('OBJD-T:') and cur is not None:
        n_tb[cur] += len(l.split(':', 1)[1].strip()) // 8
        continue
    m = re.match(r'Trace \d+: 0x[0-9a-f]+ \[[0-9a-f]+/([0-9a-f]+)/', l)
    if m:
        pc = int(m.group(1), 16); execs += 1; total += n_tb.get(pc, 0)
print(total)

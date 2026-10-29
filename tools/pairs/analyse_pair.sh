#!/bin/bash
# analyse_pair.sh DIR BASE_BINDIR NEW_BINDIR  (dirs holding llrm-cc and llrm-llc; DIR under PAIR_SCRATCH, default ~/scratch): d_faces runner, QCport bytes, drop broken rows,
# analyse.py -> DIR/report.txt, with the arms line (binary paths and md5) first. Run from the checkout whose tools/ it should use (its own).
R=$(cd "$(dirname "$0")/../.." && pwd); S=${PAIR_SCRATCH:-$HOME/scratch}; W=$S/$1; export ROWS_WORK=$W DFACES_SRC=${DFACES_SRC:-$S/qbq-llrm/src}
touch $2/llrm-* $3/llrm-* $2/nibfront $3/nibfront 2>/dev/null
cd $R
printf "arms: base=%s llrm-llc md5 %s; new=%s llrm-llc md5 %s (base is the first binary, new the second)\n" "$2" "$(md5sum $2/llrm-llc | cut -c1-12)" "$3" "$(md5sum $3/llrm-llc | cut -c1-12)" > $W/arms.txt
for n in base main; do d=$([ $n = base ] && echo $2 || echo $3)
  VSGCC_WORK=$W/dfw-$n LLRM_BIN=$d nice -n 10 uv run -q --project tools python crates/target/llrm-x86-m32/vsgcc/dfaces/run.py --levels O1 O2 O3 Os --json $W/df-$n.json > $W/df-$n.log 2>&1; done
BASE_LLRM=$2/llrm-cc MAIN_LLRM=$3/llrm-cc nice -n 10 python3 tools/rows/qcport.py > $W/qcport.log 2>&1
python3 - "$W" <<'PY'
import json,collections,os,sys
W=sys.argv[1]+'/'
q=json.load(open(W+'qcport.json')); json.dump(q,open(W+'qcport.json.orig','w'))
badm={(m,l) for n,m,l,t in q if t is None}
json.dump([r for r in q if (r[1],r[2]) not in badm],open(W+'qcport.json','w'))
V={'llrm','llrmO1','llrmO3','llrmOs'}
rows={n:[json.loads(l) for l in open(W+n+'/results.jsonl') if l.startswith('{')] for n in ('base','main')}
have={n:collections.defaultdict(set) for n in rows}
for n,rs in rows.items():
    for r in rs:
        if r['variant'] in V: have[n][r['prog']].add(r['variant'])
progs={r['prog'] for rs in rows.values() for r in rs}
bad={p for p in progs for n in rows if have[n].get(p)!=V}
open(W+'dropped.txt','w').write(f"dropped programs (a missing llrm row): {sorted(bad)}\nQCport modules failing on either side: {sorted(badm)}\n")
for n,rs in rows.items():
    os.replace(W+n+'/results.jsonl',W+n+'/results.jsonl.orig')
    open(W+n+'/results.jsonl','w').write("\n".join(json.dumps(r) for r in rs if r['prog'] not in bad)+"\n")
PY
python3 tools/rows/analyse.py --allow-failed > $W/report.txt 2>&1
echo done > $W/analysed

#!/bin/sh
# Fail-first for cells_worse.py: a cell whose clocks alone regress (bytes and instructions equal) must be flagged, an equal or better one must not.
cd "$(dirname "$0")" || exit 1; T=$(mktemp -d); trap 'rm -r $T' EXIT
echo '{"c/O2":{"code_bytes":100,"clocks":1000,"instructions":500},"d/O2":{"code_bytes":100,"clocks":1000,"instructions":500}}' > $T/a.json
echo '{"c/O2":{"code_bytes":100,"clocks":1001,"instructions":500},"d/O2":{"code_bytes":90,"clocks":900,"instructions":400}}' > $T/clk.json
echo '{"c/O2":{"code_bytes":100,"clocks":1000,"instructions":500},"d/O2":{"code_bytes":90,"clocks":900,"instructions":400}}' > $T/ok.json
python3 cells_worse.py $T/a.json $T/clk.json > $T/o1 && { echo "FAIL: a clocks-only regression was not flagged"; exit 1; }
grep -q 'WORSE c/O2 clocks' $T/o1 || { echo "FAIL: wrong cell flagged"; cat $T/o1; exit 1; }
python3 cells_worse.py $T/a.json $T/ok.json > /dev/null || { echo "FAIL: an equal or better cell was flagged"; exit 1; }
echo "ok: clocks-only regression flagged, better cells not"

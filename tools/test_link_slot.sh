#!/bin/sh
# Regression: 8 parallel links with 2 slots must never overlap more than twice (unbounded links saturated the disk).
set -eu
t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
mkdir "$t/bin" "$t/run"
cat > "$t/bin/cc" <<EOF
#!/bin/sh
touch "$t/run/\$\$"
n=\$(ls "$t/run" | wc -l)
echo "\$n" >> "$t/seen"
sleep 0.3
rm "$t/run/\$\$"
EOF
chmod +x "$t/bin/cc"
d=$(dirname "$0")
for i in 1 2 3 4 5 6 7 8; do
    PATH="$t/bin:$PATH" TMPDIR="$t" LLRM_LINK_SLOTS=2 "$d/${LINKER:-link_slot.sh}" x &
done
wait
max=$(sort -n "$t/seen" | tail -1)
[ "$(wc -l < "$t/seen")" -eq 8 ] || { echo "FAIL: $(wc -l < "$t/seen") of 8 links ran"; exit 1; }
[ "$max" -le 2 ] || { echo "FAIL: $max links overlapped, limit 2"; exit 1; }
echo "ok: max overlap $max of 8"

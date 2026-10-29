#!/usr/bin/env python3
"""Classes for a lit run through llrm-opt, judged against the same run through the pin's opt.

    triage.py LLRM.json PIN.json > triage.md

Classes: pass; not-yet-ported (exit 77, names the pass or flag); input-gap
(exit 78, IR our parser cannot read); target-specific (lit marked it
UNSUPPORTED, or a target subdirectory we have no Tti for); bug (fails where
the pin's opt passes); test-assumption (fails with the pin's opt too).
"""
import collections
import json
import re
import sys

OURS = ("X86", "AArch64")


def load(path):
    return {t["name"]: t for t in json.load(open(path))["tests"]}


def where(name):
    return "/".join(name.split(" :: ")[1].split("/")[:-1])


def classify(name, ours, pin):
    code = ours["code"]
    if code in ("PASS", "XFAIL"):
        return "pass", ""
    if code == "UNSUPPORTED":
        return "target-specific", ""
    output = ours.get("output", "")
    if re.search(r"unsupported: input", output):
        return "input-gap", re.search(r"unsupported: input: ([^\n]*)", output).group(1)[:80]
    found = re.search(r"unsupported: ([^\n:]*)", output)
    if found:
        return "not-yet-ported", found.group(1)
    sub = name.split(" :: ")[1].split("/")
    if len(sub) > 3 and sub[2] not in OURS and pin.get(name, {}).get("code") != "PASS":
        return "target-specific", sub[2]
    if pin.get(name, {}).get("code") in ("PASS", "XFAIL"):
        return "bug", ""
    return "test-assumption", ""


def main(llrm_path, pin_path):
    llrm, pin = load(llrm_path), load(pin_path)
    table, listed = collections.defaultdict(collections.Counter), []
    for name, ours in sorted(llrm.items()):
        kind, why = classify(name, ours, pin)
        table[where(name)][kind] += 1
        if kind != "pass":
            listed.append((kind, name.split(" :: ")[1], why))
    kinds = ["pass", "not-yet-ported", "input-gap", "target-specific", "bug", "test-assumption"]
    print("| directory | " + " | ".join(kinds) + " |\n|---|" + "---|" * len(kinds))
    total = collections.Counter()
    for directory, counts in sorted(table.items()):
        total.update(counts)
        print(f"| {directory} | " + " | ".join(str(counts[k]) for k in kinds) + " |")
    print("| **total** | " + " | ".join(str(total[k]) for k in kinds) + " |\n")
    for kind, test, why in sorted(listed):
        print(f"- {kind}: {test} {why}".rstrip())


if __name__ == "__main__":
    main(*sys.argv[1:3])

"""IR for the pin's llc from llrm-cc's last mid-end stage: attributes only llrm knows are removed, and the
IR is refused while any is left (the pin would read the module differently from ours, or not at all)."""

import re
import sys

LLRM_ONLY = re.compile(r'(?<![\w"=-])llrm-[a-z][a-z-]*(?![\w"=-])')


def stripped(text):
    return "".join(LLRM_ONLY.sub("", line) if line.startswith("define") else line for line in text.splitlines(True))


def left_over(text):
    return sorted(set(LLRM_ONLY.findall(text)))


if __name__ == "__main__":
    out = stripped(open(sys.argv[1]).read())
    left = left_over(out)
    if left:
        sys.exit("REFUSED: llrm-only attributes left in the IR: %s" % left)
    open(sys.argv[2], "w").write(out)

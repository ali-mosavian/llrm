"""Classify a failed compile of a sweep: NA when the source cannot be compiled for the target by any backend (DOS-only keywords, a missing system header), FAIL when a backend refused it.

A sweep that counts NA as a pass hides gaps, so only the clang front end's own rejections are NA; anything else from llrm (a Legalize error, `outside MIR's subset`) stays FAIL.
"""
import re
import sys

NA = re.compile(r"error: (expected|use of undeclared|call to undeclared|unknown type name)|fatal error: .*file not found")
BACKEND = re.compile(r"llrm-[a-z0-9-]*llc: ")


def classify(stderr: str) -> str:
    """NA if the first diagnostic is a front-end rejection of the source, else FAIL."""
    for line in stderr.splitlines():
        if BACKEND.match(line):
            return "FAIL"
        if NA.search(line):
            return "NA"
    return "FAIL"


if __name__ == "__main__":
    print(classify(sys.stdin.read()))

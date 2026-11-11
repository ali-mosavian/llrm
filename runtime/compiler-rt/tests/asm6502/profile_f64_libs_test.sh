#!/bin/bash
# profile_f64.py listed lib/int/int64.c, which no longer exists (one routine per file): a bench dir could not be profiled.
# Its member list must come from the same fp_sources.sh groups as bench_f64.py.
cd "$(dirname "$0")" || exit 2
grep -q "fp_sources.sh" profile_f64.py && ! grep -q "int/int64.c" profile_f64.py && echo "profile_f64 libs ok" || { echo "FAIL: profile_f64.py member list is not bench_f64.py's"; exit 1; }

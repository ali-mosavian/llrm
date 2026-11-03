# Inlines the `#include "codec.inc"`, `"arith.inc"` and `"core_end.inc"` lines from lib/fp/: the flatteners concatenate sources into one freestanding file.
/^#include "(codec|arith|core_end)\.inc"/ { n = $2; gsub(/"/, "", n); while ((getline line < ("lib/fp/" n)) > 0) print line; close("lib/fp/" n); next }
{ print }

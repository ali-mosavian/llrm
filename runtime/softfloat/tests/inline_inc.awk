# Inlines `#include "sf_*.inc"` lines from src/ (recursively not needed): the flatteners concatenate sources into one freestanding file.
/^#include "sf_[a-z_]+\.inc"/ { n = $2; gsub(/"/, "", n); while ((getline line < ("src/" n)) > 0) print line; close("src/" n); next }
{ print }

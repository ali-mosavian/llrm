# Sourced by vsgcc/build.sh and run.sh: the reference compilers of the rows are pinned and named in every header.
# CLANG/GCC override the program (VSGCC_CLANG, VSGCC_GCC); a version other than the pin refuses.
PIN_CLANG_HASH=d1106deb71cc
PIN_GCC_VERSION=13.4.0
pin_references() {
    CLANG=${VSGCC_CLANG:-clang}
    GCC=${VSGCC_GCC:-gcc}
    local c g
    c=$("$CLANG" --version 2>&1 | head -n 1)
    g=$("$GCC" --version 2>&1 | head -n 1)
    case $c in *"$PIN_CLANG_HASH"*) ;; *) echo "rows: clang is '$c', not the pinned $PIN_CLANG_HASH (set VSGCC_CLANG)" >&2; return 1;; esac
    case $g in *"$PIN_GCC_VERSION"*) ;; *) echo "rows: gcc is '$g', not the pinned $PIN_GCC_VERSION (set VSGCC_GCC)" >&2; return 1;; esac
    echo "references: $g; $c"
}

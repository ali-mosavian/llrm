# Sourced by the sweep drivers: fresh_compile OUT CMD...  runs CMD (which writes OUT) and returns its status, or 97 when it
# succeeded without leaving a non-empty OUT. OUT is removed first, so a failed or silent compile never leaves the previous
# cell's object to be read as this cell's (x87: an unsupported -march left the old .obj and the sweep read SAME).
fresh_compile() {
  _out=$1; shift
  rm -f "$_out"
  "$@"; _rc=$?
  [ "$_rc" -ne 0 ] || [ -s "$_out" ] || _rc=97
  return $_rc
}

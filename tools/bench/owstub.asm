; Open Watcom marks every object with a `main` as wanting its start-up code. The reference runs use
; tools/loops/runtime/crt.asm instead, so these names only have to exist.
.model medium
.386
public _cstart_
public _big_code_
_big_code_ equ 0
.code
_cstart_ proc far
    ret
_cstart_ endp
end

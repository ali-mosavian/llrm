; Startup and termination for a QB45 compiled module.  BC_SA's first far
; address is its module header; user code begins at the fixed offset 30h.
.model medium
.386

extrn _llrm_os_exit:far

public _main
public B$CEND
public B$CENP

.stack 4096

; The compiler's zero-length BC_SAB segment immediately precedes BC_SA.  By
; contributing this zero-length label to the same public segment, the label
; denotes BC_SA's first far module-header address without inventing an EXTDEF.
BC_SAB segment word public 'BC_SEGS'
qb_bc_sa label byte
BC_SAB ends
DGROUP group BC_SAB

QB_TEXT segment para public 'CODE'
assume ds:DGROUP
_main proc far
    mov ax, DGROUP
    mov ds, ax
    mov es, ax
    ; QB's near frame addresses are passed as DS offsets.  Preserve the
    ; loader-selected physical stack while rebasing SS into that same group.
    mov dx, ss
    sub dx, ax
    shl dx, 4
    cli
    mov ss, ax
    add sp, dx
    sti
    mov ax, word ptr qb_bc_sa+2
    mov es, ax
    ; The module's code segment is the first BC_SA far address.  Its header
    ; starts at offset zero and user code at the measured fixed offset 30h.
    mov bx, 30h
    ; 8086 has no far call through an ES:offset register pair.  Construct the
    ; return address and transfer as the far return would.
    push cs
    push offset startup_return
    push es
    push bx
    ; llrm-qb's INTEGER-to-LONG sequence retains DX's incoming low word.
    ; QB45 enters a module with it clear, so make that initial register state
    ; part of this startup boundary rather than inheriting DOS's value.
    xor dx, dx
    retf
startup_return:
    push 0
    call far ptr _llrm_os_exit
_main endp

B$CEND proc far
B$CENP label far
    push 0
    call far ptr _llrm_os_exit
B$CEND endp

QB_TEXT ends
end _main

; Startup for a QB45 compiled module (QB: crt0 and rtinit.asm).
; DGROUP is the static data, then STACK, then one dynamic region from qb_atopsp to the top of
; the group (rtinit.asm:244-300). DOSSEG makes the linker put STACK last.
.model medium
.386
dosseg

extrn _qb_start:far
extrn _llrm_os_psp:word

public _main
public _qb_atopsp
public _qb_asizds
public _qb_xi_begin
public _qb_xi_end

; The initializer segments bracket every module's XI contribution (rmacros.inc INITIALIZER).
; Each C module declares them in this order too (xi.h), so the first to appear fixes it.
XIB segment word public 'DATA'
XIB ends
XI segment word public 'DATA'
XI ends
XIE segment word public 'DATA'
XIE ends

_DATA segment word public 'DATA'
; The last usable word of DGROUP, set below from the PSP: 64K, or the memory DOS gave.
_qb_asizds dw 0
_qb_xi_begin dw offset DGROUP:XIB
_qb_xi_end dw offset DGROUP:XIE
_DATA ends

_BSS segment word public 'BSS'
_BSS ends

STACK segment para stack 'STACK'
    db 2048 dup (?)
_qb_atopsp label byte
STACK ends

; The compiler's zero-length BC_SAB precedes BC_SA.  A label contributed to the same public segment
; is BC_SA's first far address, its module header, without an EXTDEF of its own.
BC_SAB segment word public 'BC_SEGS'
qb_bc_sa label byte
BC_SAB ends

DGROUP group _DATA, _BSS, XIB, XI, XIE, STACK, BC_SAB

.code
assume ds:DGROUP
_main proc far
    mov bx, es                      ; the PSP, before DS leaves it
    mov ax, DGROUP
    mov ds, ax
    ; The linker gives SS the STACK segment's own frame.  Rebase it to DGROUP, keeping the physical
    ; stack, so a near pointer to a frame cell reaches it through DS.
    mov dx, ss
    sub dx, ax
    shl dx, 4
    cli
    mov ss, ax
    add sp, dx
    sti
    mov _llrm_os_psp, bx
    ; PSP:2 is the first paragraph past the program's memory.
    mov es, bx
    mov cx, es:[2]
    sub cx, ax
    mov dx, 0fffeh
    cmp cx, 1000h
    jae short sized
    shl cx, 4
    mov dx, cx
    dec dx
    and dl, 0feh
sized:
    mov _qb_asizds, dx
    ; The linker stores no BSS: zero it, from its start to the stack's.
    mov ax, ds
    mov es, ax
    mov di, offset DGROUP:_BSS
    mov cx, offset DGROUP:STACK
    sub cx, di
    xor ax, ax
    cld
    rep stosb
    call far ptr _qb_start
    ; Enter the module: its code segment is BC_SA, user code at the fixed offset 30h.
    mov ax, word ptr qb_bc_sa+2
    mov es, ax
    mov bx, 30h
    push cs
    push offset hang
    push es
    push bx
    xor dx, dx
    retf
hang:
    jmp short hang                  ; a module ends through B$CENP or B$CEND
_main endp

end _main

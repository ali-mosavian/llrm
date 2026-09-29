; BASIC probe. trash: a worst-case callee. arm/verify: bracket one call from
; a BC caller and record what the callee failed to keep (built -DNOARM they
; do nothing; armX/verifyX always work). put*/getB: 32-byte slots the modules keep values in. rec*:
; record the stack a BASIC caller built. a*/x*: return known values, pascal
; and cdecl.
.model medium
.386
.data
savedbp dw ?
savedsp dw ?
masks   dw 64 dup (0)
slots   db 128*32 dup (0)
keptdesc dw 0
sngval  dd 1.5
dblval  dq -2.25
fac     dq ?
.code PROBE_TEXT
public TRASH, ARM, VERIFY, ARMX, VERIFYX, BADREGS, PROBEMASK, PUTI, PUTL, PUTF, PUTD, PUTY, PUTC, GETB, RECVAL, RECSEG, _reccdl
public RECCDL, XINT, XLNG, XSNG, XDBL  ; llrm-qb's spelling of the CDECL names
public AINT, ALNG, ASNG, ADBL, KEEPDESC, ASTR, _xint, _xlng, _xsng, _xdbl

TRASH proc far
    mov ax, 0BAD0h
    mov es, ax
    ror esi, 16
    mov si, 0BAD1h
    ror esi, 16
    ror edi, 16
    mov di, 0BAD2h
    ror edi, 16
    ror ebp, 16
    mov bp, 0BAD3h
    ror ebp, 16
    mov eax, 0DEAD0BADh
    mov ebx, 0DEAD1BADh
    mov ecx, 0DEAD2BADh
    mov edx, 0DEAD3BADh
    ret
TRASH endp

ARM proc far
ifndef NOARM
    jmp ARMX
endif
    ret
ARM endp

ARMX proc far
    mov savedbp, bp
    mov ax, sp
    add ax, 4
    mov savedsp, ax
    mov si, 5A5Ah
    mov di, 0A5A5h
    ret
ARMX endp

; VERIFY(BYVAL k): masks[k] |= bit 0 SI, 1 DI, 2 BP, 3 SP, 4 DS, 5 DF, 6 SS.
VERIFY proc far
ifndef NOARM
    jmp VERIFYX
endif
    ret 2
VERIFY endp

VERIFYX proc far
    xor cx, cx
    cmp si, 5A5Ah
    je @F
    or cx, 1
@@: cmp di, 0A5A5h
    je @F
    or cx, 2
@@: mov ax, ds
    mov bx, DGROUP
    mov ds, bx
    cmp bp, savedbp
    je @F
    or cx, 4
@@: mov dx, sp
    add dx, 6
    cmp dx, savedsp
    je @F
    or cx, 8
@@: cmp ax, bx
    je @F
    or cx, 16
@@: mov ax, ss
    cmp ax, bx
    je @F
    or cx, 64
@@: pushf
    pop ax
    test ax, 400h
    jz @F
    or cx, 32
    cld
@@: mov bx, sp
    mov bx, ss:[bx+4]
    and bx, 63
    add bx, bx
    or masks[bx], cx
    ret 2
VERIFYX endp

; Loses SI, DI and DF: the harness checks that VERIFYX sees it.
BADREGS proc far
    mov si, 1
    mov di, 2
    std
    ret
BADREGS endp

PROBEMASK proc far
    mov bx, sp
    mov bx, ss:[bx+4]
    and bx, 63
    add bx, bx
    mov ax, masks[bx]
    ret 2
PROBEMASK endp

;  PUT*(BYVAL k, BYVAL v): v's CX bytes into slot k.
putn proc near
    push si
    push di
    push es
    mov si, sp
    lea si, [si+6+2+4]
    mov bx, cx
    mov di, ss:[si+bx]
    and di, 127
    shl di, 5
    add di, offset slots
    push ds
    pop es
    rep movs byte ptr es:[di], byte ptr ss:[si]
    pop es
    pop di
    pop si
    ret
putn endp

PUTI proc far
    mov cx, 2
    call putn
    ret 4
PUTI endp

PUTL proc far
    mov cx, 4
    call putn
    ret 6
PUTL endp

PUTF proc far
    mov cx, 4
    call putn
    ret 6
PUTF endp

PUTD proc far
    mov cx, 8
    call putn
    ret 10
PUTD endp

PUTY proc far
    mov cx, 8
    call putn
    ret 10
PUTY endp

; PUTC(BYVAL k, BYVAL i, BYVAL c): byte i of slot k.
PUTC proc far
    mov bx, sp
    mov al, ss:[bx+4]
    mov cx, ss:[bx+6]
    mov bx, ss:[bx+8]
    and bx, 127
    shl bx, 5
    add bx, cx
    mov slots[bx], al
    ret 6
PUTC endp

; GETB(BYVAL k, BYVAL i)
GETB proc far
    mov bx, sp
    mov cx, ss:[bx+4]
    mov bx, ss:[bx+6]
    and bx, 127
    shl bx, 5
    add bx, cx
    xor ax, ax
    mov al, slots[bx]
    ret 4
GETB endp

; Copies CX bytes of arguments, from the caller's SP+4 up, into slot DI.
copyargs proc near
    push si
    push di
    push es
    mov si, sp
    lea si, [si+6+2+2+4]
    shl di, 5
    add di, offset slots
    push ds
    pop es
    rep movs byte ptr es:[di], byte ptr ss:[si]
    pop es
    pop di
    pop si
    ret
copyargs endp

RECVAL proc far
    push di
    mov di, 90
    mov cx, 18
    call copyargs
    pop di
    ret 18
RECVAL endp

_reccdl proc far
RECCDL::
    push di
    mov di, 92
    mov cx, 14
    call copyargs
    pop di
    ret
_reccdl endp

; RECSEG(SEG a AS INTEGER, SEG b AS DOUBLE): *a, *b, a.seg-ds, b.seg-ds; then *a += 1.
RECSEG proc far
    push bp
    mov bp, sp
    push si
    push di
    push es
    mov di, offset slots + 91*32
    les si, [bp+10]
    mov ax, es:[si]
    mov [di], ax
    inc word ptr es:[si]
    mov ax, es
    mov dx, ds
    sub ax, dx
    mov [di+10], ax
    les si, [bp+6]
    mov ax, es:[si]
    mov [di+2], ax
    mov ax, es:[si+2]
    mov [di+4], ax
    mov ax, es:[si+4]
    mov [di+6], ax
    mov ax, es:[si+6]
    mov [di+8], ax
    mov ax, es
    sub ax, dx
    mov [di+12], ax
    pop es
    pop di
    pop si
    pop bp
    ret 8
RECSEG endp

AINT proc far
    mov ax, -1235h
    ret
AINT endp

ALNG proc far
    mov ax, 0A987h
    mov dx, 0EDCBh
    ret
ALNG endp

; Hidden near result pointer, last argument: store, return it in AX.
ASNG proc far
    mov bx, sp
    mov bx, ss:[bx+4]
    mov eax, sngval
    mov [bx], eax
    mov ax, bx
    ret 2
ASNG endp

ADBL proc far
    mov bx, sp
    mov bx, ss:[bx+4]
    mov eax, dword ptr dblval
    mov [bx], eax
    mov eax, dword ptr dblval+4
    mov [bx+4], eax
    mov ax, bx
    ret 2
ADBL endp

; KEEPDESC(s AS STRING): keeps the descriptor's address for ASTR to return.
KEEPDESC proc far
    mov bx, sp
    mov ax, ss:[bx+4]
    mov keptdesc, ax
    ret 2
KEEPDESC endp

ASTR proc far
    mov ax, keptdesc
    ret
ASTR endp

_xint proc far
XINT::
    mov ax, -1235h
    ret
_xint endp

_xlng proc far
XLNG::
    mov ax, 0A987h
    mov dx, 0EDCBh
    ret
_xlng endp

; Microsoft C: the value in a static, its address in AX.
_xsng proc far
XSNG::
    mov eax, sngval
    mov dword ptr fac, eax
    mov ax, offset fac
    ret
_xsng endp

_xdbl proc far
XDBL::
    mov eax, dword ptr dblval
    mov dword ptr fac, eax
    mov eax, dword ptr dblval+4
    mov dword ptr fac+4, eax
    mov ax, offset fac
    ret
_xdbl endp
end

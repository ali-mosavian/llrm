; trash: a worst-case Borland callee. arm/verify: bracket one call from a
; BCC caller built -r-, and report what the callee failed to keep.
.model medium
.386
extrn _probe_failed:far
.data
savedbp dw ?
savedsp dw ?
.code PROBE_TEXT
public _trash, _arm, _verify

_trash proc far
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
    std
    cld
    ret
_trash endp

_arm proc far
    mov savedbp, bp
    mov ax, sp
    add ax, 4
    mov savedsp, ax
    mov si, 5A5Ah
    mov di, 0A5A5h
    ret
_arm endp

; Bit 0 SI, 1 DI, 2 BP, 3 SP, 4 DS, 5 DF.
_verify proc far
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
@@: pushf
    pop ax
    test ax, 400h
    jz @F
    or cx, 32
    cld
@@: jcxz done
    mov bx, sp
    push cx
    push word ptr ss:[bx+4]
    call _probe_failed
    add sp, 4
done:
    ret
_verify endp
end

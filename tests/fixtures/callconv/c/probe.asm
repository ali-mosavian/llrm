; trash: a worst-case Borland callee. arm/verify: bracket one call from a
; BCC caller built -r-, and report what the callee failed to keep.
.model medium
.386
extrn _probe_failed:far
.data
savedbp dw ?
savedsp dw ?
handler dd ?
after dw ?
.code PROBE_TEXT
public _trash, _arm, _verify, _intcall

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
; intcall(handler, before, after): each register set, `handler` entered
; as an interrupt, what each held on return. Order AX BX CX DX SI DI BP DS
; ES in both arrays; DS stays DGROUP.
_intcall proc far
    push bp
    mov bp, sp
    push si
    push di
    push es
    mov bx, [bp+10]
    mov word ptr [bx], 0A1A1h
    mov word ptr [bx+2], 0B2B2h
    mov word ptr [bx+4], 0C3C3h
    mov word ptr [bx+6], 0D4D4h
    mov word ptr [bx+8], 5151h
    mov word ptr [bx+10], 0D1D1h
    mov word ptr [bx+12], 0B0B0h
    mov word ptr [bx+14], ds
    mov word ptr [bx+16], 0E5E5h
    mov ax, [bp+6]
    mov word ptr handler, ax
    mov ax, [bp+8]
    mov word ptr handler+2, ax
    mov ax, [bp+12]
    mov after, ax
    mov savedbp, bp
    mov ax, 0E5E5h
    mov es, ax
    mov ax, 0A1A1h
    mov bx, 0B2B2h
    mov cx, 0C3C3h
    mov dx, 0D4D4h
    mov si, 5151h
    mov di, 0D1D1h
    mov bp, 0B0B0h
    pushf
    call dword ptr handler
    push bx
    mov bx, after
    mov [bx], ax
    pop word ptr [bx+2]
    mov [bx+4], cx
    mov [bx+6], dx
    mov [bx+8], si
    mov [bx+10], di
    mov [bx+12], bp
    mov [bx+14], ds
    mov [bx+16], es
    mov bp, savedbp
    pop es
    pop di
    pop si
    pop bp
    ret
_intcall endp
end

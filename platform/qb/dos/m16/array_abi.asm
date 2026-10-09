; QB's variable-arity Pascal array entries around portable regparm3 C.
.model medium
.386

extrn _qb_array_dim@3:near
extrn _qb_array_erase@3:near
extrn _qb_array_bounds:word
extrn _llrm_os_more:far

public B$DDIM
public B$ERAS
public _qb_array_more@3

RUNTIME_TEXT segment para public 'CODE'
assume ds:DGROUP

; pAd, dimensions/type, element bytes, then one upper/lower pair per dimension.
B$DDIM proc far
    push bp
    mov bp, sp
    push si
    push di
    mov ax, word ptr [bp+6]
    mov dx, word ptr [bp+8]
    mov cx, word ptr [bp+10]
    lea si, [bp+12]
    mov _qb_array_bounds, si
    call _qb_array_dim@3
    or ax, ax
    jz short cleanup
    mov di, word ptr [bp+6]
    mov bx, word ptr [di]
    sub word ptr [di+10], bx
    mov word ptr [di], 0
    mov ax, bx
    shr ax, 4
    mov cx, ds
    add ax, cx
    mov word ptr [di+2], ax
cleanup:
    mov cx, word ptr [bp+8]
    and cx, 255
    shl cx, 1
    shl cx, 1
    add cx, 6
    mov array_cleanup, cx
    pop di
    pop si
    pop bp
    pop word ptr array_return
    pop word ptr array_return+2
    add sp, array_cleanup
    jmp dword ptr array_return
B$DDIM endp

B$ERAS proc far
    push bp
    mov bp, sp
    mov ax, word ptr [bp+6]
    call _qb_array_erase@3
    pop bp
    retf 2
B$ERAS endp

; The shared OS layer is cdecl16 even when portable QB code is regparm3.
_qb_array_more@3 proc near
    push ax
    call far ptr _llrm_os_more
    add sp, 2
    ret
_qb_array_more@3 endp

RUNTIME_TEXT ends

_DATA segment word public 'DATA'
array_cleanup dw ?
array_return dw 2 dup (?)
_DATA ends

end

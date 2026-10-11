.386
.model flat

public bench_particle_
public _main
.data
.data?
_vx label byte
    db 202 dup (?)
_vy label byte
    db 202 dup (?)
_vz label byte
    db 202 dup (?)
_px label byte
    db 202 dup (?)
_py label byte
    db 202 dup (?)
_pz label byte
    db 202 dup (?)
extern report_:near
.code PARTICLE_TEXT
bench_particle_ proc near
    push ebx
    push ecx
    push edx
    push esi
L0_0:
    mov ax, -50
    mov cx, -100
    mov edx, -50
L0_2:
    mov word ptr _px+100[edx*2], ax
    mov word ptr _py+100[edx*2], cx
    mov bx, ax
    neg bx
    mov word ptr _pz+100[edx*2], bx
    mov ebx, eax
    and ebx, 7
    mov word ptr _vx+100[edx*2], bx
    mov ebx, edx
    and ebx, 3
    mov esi, 3
    sub esi, ebx
    mov word ptr _vy+100[edx*2], si
    mov word ptr _vz+100[edx*2], 1
    inc ax
    add cx, 2
    inc edx
    cmp ax, 51
    jne L0_2
L0_1:
    mov ax, -100
L0_35:
    mov ecx, -202
L0_40:
    mov dx, word ptr _vx+202[ecx]
    add word ptr _px+202[ecx], dx
    mov dx, word ptr _vy+202[ecx]
    add word ptr _py+202[ecx], dx
    mov dx, word ptr _vz+202[ecx]
    add word ptr _pz+202[ecx], dx
    add ecx, 2
    jne L0_40
L0_37:
    inc ax
    jne L0_35
L0_34:
    xor eax, eax
    mov ecx, -202
L0_76:
    movsx edx, word ptr _px+202[ecx]
    add eax, edx
    movsx edx, word ptr _py+202[ecx]
    lea edx, [edx+edx*2]
    add eax, edx
    movsx edx, word ptr _pz+202[ecx]
    lea ebx, [edx+edx*2]
    lea edx, [edx+ebx*2]
    add eax, edx
    add ecx, 2
    jne L0_76
L0_74:
    pop esi
    pop edx
    pop ecx
    pop ebx
    ret
bench_particle_ endp
_main proc near
L1_0:
    call bench_particle_
    call report_
    xor eax, eax
    ret
_main endp
end

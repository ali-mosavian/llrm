; TIMEIT PROGRAM.EXE [args]: runs the program and prints "TSC n", the RDTSC cycles it took. Under a DOSBox
; with a fixed cycle count that is a time: cycles / (cycles per ms). Never the PIT, which ticks at 18.2 Hz.
.model tiny
.586
.code
org 100h
start:
    mov sp, offset stack_top
    mov bx, offset stack_top
    shr bx, 4
    inc bx
    mov ah, 4Ah                 ; give back what we do not use: the child needs it
    int 21h
    mov si, 81h
skip:
    lodsb
    cmp al, ' '
    je skip
    dec si
    mov di, offset fname
name_char:
    lodsb
    cmp al, 0Dh
    je name_end
    cmp al, ' '
    je name_end
    stosb
    jmp name_char
name_end:
    xor al, al
    stosb
    mov di, offset ctail + 1    ; the rest of the line is the child's command tail, with its leading blank
    mov byte ptr [di], ' '
    inc di
    xor cx, cx
    cmp byte ptr [si-1], 0Dh
    je tail_done
tail_char:
    lodsb
    stosb
    inc cx
    cmp al, 0Dh
    jne tail_char
    dec cx
tail_done:
    mov byte ptr [di], 0Dh
    inc cx
    mov byte ptr [ctail], cl
    mov word ptr [pblock + 2], offset ctail
    mov word ptr [pblock + 4], cs
    mov word ptr [pblock + 8], cs
    mov word ptr [pblock + 12], cs
    mov saved_ss, ss
    mov saved_sp, sp
    rdtsc
    mov t0lo, eax
    mov ax, 4B00h
    mov dx, offset fname
    mov bx, offset pblock
    int 21h
    cli
    mov ss, saved_ss
    mov sp, saved_sp
    sti
    rdtsc
    sub eax, t0lo               ; one 32-bit delta: a program under a minute at 75 MHz
    mov t0lo, eax
    mov dx, offset label_text
    mov ah, 9
    int 21h
    mov eax, t0lo
    call print_eax
    mov ax, 4C00h
    int 21h

; EAX in decimal and a newline, to standard output
print_eax proc
    mov edi, offset digits + 12
    mov byte ptr [edi-1], 10
    mov byte ptr [edi-2], 13
    sub edi, 2
    mov ebx, 10
more:
    xor edx, edx
    div ebx
    add dl, '0'
    dec edi
    mov [edi], dl
    test eax, eax
    jnz more
    mov dx, di
    mov cx, offset digits + 12
    sub cx, di
    mov bx, 1
    mov ah, 40h
    int 21h
    ret
print_eax endp

label_text db 'TSC $'
fname db 80 dup (0)
ctail db 130 dup (0)
pblock dw 0, 0, 0, 5Ch, 0, 6Ch, 0
t0lo dd 0
saved_ss dw 0
saved_sp dw 0
digits db 12 dup (?)
    db 256 dup (0)
stack_top label byte
end start

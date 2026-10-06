; The externals the C run tests call, in cdecl32: `report` prints a signed decimal and a
; newline on stdout (handle 1), as m16's runtime does.
.386
.model flat

extrn _llrm_os_open:near
extrn _llrm_os_read:near
extrn _llrm_os_write_file:near
extrn _llrm_os_exit:near

.data
digits  db 12 dup (?)
inhandle dd 0FFFFFFFFh
inname  db 'DICKENS', 0
stkmsg  db 'Stack Overflow!', 13, 10

public __STKOVERFLOW

.code
; Open Watcom's stack overflow (clib stk086.asm `__STKOVERFLOW`): the message and exit status 1,
; which a checked function enters (-fsanitize=stack). On stdout, which a test captures.
__STKOVERFLOW proc
    push 17
    push offset stkmsg
    push 1
    call _llrm_os_write_file
    push 1
    call _llrm_os_exit
__STKOVERFLOW endp

; void report(long v)
public _report
_report proc
    push ebp
    mov ebp, esp
    push ebx
    push edi
    mov eax, dword ptr [ebp+8]
    mov edi, offset digits + 12
    mov byte ptr [edi-1], 10
    mov byte ptr [edi-2], 13
    sub edi, 2
    xor ecx, ecx
    test eax, eax
    jns positive
    neg eax
    inc ecx
positive:
    mov ebx, 10
more:
    xor edx, edx
    div ebx
    add dl, '0'
    dec edi
    mov [edi], dl
    test eax, eax
    jnz more
    test ecx, ecx
    jz unsigned
    dec edi
    mov byte ptr [edi], '-'
unsigned:
    mov ecx, offset digits + 12
    sub ecx, edi
    push ecx
    push edi
    push 1
    call _llrm_os_write_file
    add esp, 12
    pop edi
    pop ebx
    pop ebp
    ret
_report endp

; int input_read(char *buffer, int count): up to count bytes of DICKENS (opened on the first call, in
; the current directory), 0 at its end or on an error. bench/grep reads its input through this.
public _input_read
_input_read proc
    push ebp
    mov ebp, esp
    cmp inhandle, 0FFFFFFFFh
    jne opened
    push 0
    push offset inname
    call _llrm_os_open
    add esp, 8
    movsx eax, ax
    test eax, eax
    js failed
    mov inhandle, eax
opened:
    push dword ptr [ebp+12]
    push dword ptr [ebp+8]
    push inhandle
    call _llrm_os_read
    add esp, 12
    test eax, eax
    jns done
failed:
    xor eax, eax
done:
    pop ebp
    ret
_input_read endp

; int keep(int x), long keep32(long x): opaque to the optimizer, which cannot see through a call.
public _keep
_keep proc
    mov eax, dword ptr [esp+4]
    ret
_keep endp

public _keep32
_keep32 proc
    mov eax, dword ptr [esp+4]
    ret
_keep32 endp

end

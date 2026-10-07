; The externals the C run tests call, in watcall32, the default convention, so that the tests declare them as
; they would any function: arguments in EAX and EDX, a callee that keeps every register but EAX and its
; argument registers. `report` prints a signed decimal and a newline on standard output, as m16's runtime does.
; The OS layer's `_llrm_os_*` stay in cdecl32, which these call.
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
    push DOS_STDOUT
    call _llrm_os_write_file
    push 1
    call _llrm_os_exit
__STKOVERFLOW endp

; void report(long v): v in EAX.
public report_
report_ proc
    push ebx
    push ecx
    push edx
    push edi
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
    push DOS_STDOUT
    call _llrm_os_write_file
    add esp, 12
    pop edi
    pop edx
    pop ecx
    pop ebx
    ret
report_ endp

; int input_read(char *buffer, int count): buffer in EAX, count in EDX. Up to count bytes of DICKENS (opened on
; the first call, in the current directory), 0 at its end or on an error. bench/grep reads its input through this.
public input_read_
input_read_ proc
    push ebx
    push ecx
    push esi
    push edi
    mov esi, eax
    mov edi, edx
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
    push edi
    push esi
    push inhandle
    call _llrm_os_read
    add esp, 12
    test eax, eax
    jns done
failed:
    xor eax, eax
done:
    pop edi
    pop esi
    pop ecx
    pop ebx
    ret
input_read_ endp

; int keep(int x), long keep32(long x): opaque to the optimizer, which cannot see through a call. x is in EAX,
; where the answer goes.
public keep_
keep_ proc
    ret
keep_ endp

public keep32_
keep32_ proc
    ret
keep32_ endp

; The same routines for a program in the stack convention (-mabi=sysv): arguments on the stack, which the caller removes.
public _report
_report proc
    mov eax, dword ptr [esp+4]
    jmp report_
_report endp

public _input_read
_input_read proc
    mov eax, dword ptr [esp+4]
    mov edx, dword ptr [esp+8]
    jmp input_read_
_input_read endp

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

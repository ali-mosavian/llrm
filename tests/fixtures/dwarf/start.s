# The entry a freestanding i386 Linux program needs: exit with main's result.
.globl _start
_start:
    call main
    mov %eax, %ebx
    mov $1, %eax
    int $0x80

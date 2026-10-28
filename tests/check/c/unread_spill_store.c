// RUN: llrm-cc %s -m16 -march=i486 -Os -S -o /dev/stdout
// A spill slot nothing reads keeps no store: huge_copy's loop stored the byte it had just
// loaded to [bp-2] every trip (the forwarding had dropped the reload), 5.1M clocks.
// CHECK-LABEL: {{_copy(@3)?}} proc
// CHECK: {{mov [a-d]l, byte ptr es:}}
// CHECK-NOT: {{mov byte ptr \[bp-[0-9]+\], [a-d]l}}
// CHECK: {{mov byte ptr es:\[[a-z]+\], [a-d]l}}
unsigned char __huge big[140000];
unsigned char __huge src[140000];

void copy(unsigned long to, unsigned long from, unsigned long n)
{
    unsigned long i;
    for (i = 0; i < n; i++) big[to + i] = src[from + i];
}

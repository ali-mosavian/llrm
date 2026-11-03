/* The 6502 configuration: the sticky right shift as byte then bit steps. The shift libcall (llvm-mos runtime) loops one bit per iteration, ~530 cycles for an average shift.
 * f32 add through llrm-cc -Os: 260781 -> 220446 cycles over 64 operands, reached code 2458 -> 2441 B; mul and div unchanged in bytes and 0.3-0.4% faster. */
#ifndef SF_6502_H
#define SF_6502_H
#define SF_SHIFT_STEPS 1
#endif

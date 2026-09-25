#include <stdint.h>
#include <stdio.h>

/*
 * Value-only reference for the fixed_sqrt.py experiment.
 *
 * Inputs are unsigned little-endian 64-bit radicands smaller than INT64_MAX.
 * Each result is the same radicand converted with FILD, square-rooted with
 * FSQRT, and converted to the nearest integer with FISTP.  The driver sets
 * the x87 control word explicitly, so this does not inherit a caller's
 * precision or rounding mode.
 */
static uint64_t
fsqrt_nearest(uint64_t radicand)
{
    uint64_t result;

    __asm__ volatile(
        "fildq %1\n\t"
        "fsqrt\n\t"
        "fistpq %0"
        : "=m"(result)
        : "m"(radicand)
        : "st");
    return result;
}

int
main(void)
{
    uint64_t input[4096];
    uint64_t output[4096];
    unsigned short original_control;
    unsigned short nearest_extended;
    size_t count;

    __asm__ volatile("fnstcw %0" : "=m"(original_control));
    /* Mask exceptions, extended precision, round to nearest/even. */
    nearest_extended = (unsigned short)((original_control | 0x033fU) & ~0x0c00U);
    __asm__ volatile("fldcw %0" : : "m"(nearest_extended));

    while ((count = fread(input, sizeof(input[0]), 4096, stdin)) != 0) {
        size_t index;

        for (index = 0; index < count; ++index)
            output[index] = fsqrt_nearest(input[index]);
        if (fwrite(output, sizeof(output[0]), count, stdout) != count) {
            __asm__ volatile("fldcw %0" : : "m"(original_control));
            return 2;
        }
    }

    __asm__ volatile("fldcw %0" : : "m"(original_control));
    return ferror(stdin) ? 1 : 0;
}

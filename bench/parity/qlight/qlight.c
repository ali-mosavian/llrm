// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
extern void report(long value);

#define LS_NEUTRAL 120

/* From qb-qrender d_surf.bas and qcport render/ls.c. */
short ls_scale_byte(short raw, short sval)
{
    long value;

    value = (long)raw * sval / LS_NEUTRAL;
    if (value > 255) value = 255;
    if (value < 0) value = 0;
    return (short)value;
}

long bench_qlight(void)
{
    return (long)ls_scale_byte(200, 120) * 1000000L
         + (long)ls_scale_byte(200, 60) * 1000L
         + ls_scale_byte(200, 240)
         + ls_scale_byte(-20, 120);
}

int main(void)
{
    report(bench_qlight());
    return 0;
}

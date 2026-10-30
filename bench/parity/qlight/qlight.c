// flags: -O2 -march=i486 | -O2 -march=i486 -m32
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

/* Read through volatile so no build folds the kernel to a constant. */
static volatile short input_raw = 200;
static volatile short input_low = -20;
static volatile short input_s1 = 120;
static volatile short input_s2 = 60;
static volatile short input_s3 = 240;

long bench_qlight(void)
{
    return (long)ls_scale_byte(input_raw, input_s1) * 1000000L
         + (long)ls_scale_byte(input_raw, input_s2) * 1000L
         + ls_scale_byte(input_raw, input_s3)
         + ls_scale_byte(input_low, input_s1);
}

int main(void)
{
    report(bench_qlight());
    return 0;
}

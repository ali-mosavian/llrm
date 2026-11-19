/* A private callee called three times: its body goes if all three are inlined, but a copy carries the
   argument and float-operand placement of its site. QCport pl_trace.c: `pl_box_axis`, 21 size units,
   ~70 B a copy against a ~34 B body, inlined three times was +181 B at -Os. */
typedef struct { float x, y, z; } Vec3;

static short near box_axis(float lo, float hi, float start, float delta, short axis, float *enter, float *leave,
                           short *enter_axis, float *enter_sign)
{
    float a, b, sign;
    if (delta > -0.0001f && delta < 0.0001f) return (short)(start >= lo && start < hi);
    a = (lo - start) / delta;
    b = (hi - start) / delta;
    sign = -1.0f;
    if (a > b) {
        float swap = a;
        a = b;
        b = swap;
        sign = 1.0f;
    }
    if (a >= *enter) {
        *enter = a;
        *enter_axis = axis;
        *enter_sign = sign;
    }
    if (b < *leave) *leave = b;
    return (short)(*enter <= *leave);
}

short trace_box(Vec3 *start, Vec3 *lo, Vec3 *hi, Vec3 *delta, float *out)
{
    float enter = 0.0f, leave = 1.0f, sign = 0.0f;
    short axis = -1;
    if (!box_axis(lo->x, hi->x, start->x, delta->x, 0, &enter, &leave, &axis, &sign) ||
        !box_axis(lo->y, hi->y, start->y, delta->y, 1, &enter, &leave, &axis, &sign) ||
        !box_axis(lo->z, hi->z, start->z, delta->z, 2, &enter, &leave, &axis, &sign))
        return 0;
    out[0] = enter;
    out[1] = sign;
    return axis + 1;
}

/* The control: a callee smaller than the work around it is inlined at every site. */
static double small_axis(double a, double b, double c, double d, double e, double f, double g, double h, double i)
{
    return (a - b) * (c - d) + (e - f) * (g - h) * i;
}

double trace_small(double *p, double *q, double *r)
{
    return small_axis(p[0], p[1], p[2], p[3], q[0], q[1], q[2], q[3], r[0]) +
           small_axis(p[1], p[2], p[3], p[0], q[1], q[2], q[3], q[0], r[1]) +
           small_axis(p[2], p[3], p[0], p[1], q[2], q[3], q[0], q[1], r[2]);
}

/* r_portal's far stack in a segment of its own; d_faces's const table in CONST2. */
static short far stack[8];
static const float table[2] = { 1.0f, 2.0f };

int peek(int i)
{
    stack[i] = i;
    return stack[i + 1] + (int) table[i];
}

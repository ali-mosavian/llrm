/* harness-only __multi3: 128x128 -> low 128, from 32-bit limbs (no 128-bit multiply). */
typedef unsigned long long u64; typedef unsigned u32;
void __multi3(u64 *r, u64 al, u64 ah, u64 bl, u64 bh)
{
    u32 a[4] = {(u32)al, (u32)(al >> 32), (u32)ah, (u32)(ah >> 32)}, b[4] = {(u32)bl, (u32)(bl >> 32), (u32)bh, (u32)(bh >> 32)};
    u32 o[4] = {0, 0, 0, 0};
    for (int i = 0; i < 4; i++) {
        u64 carry = 0;
        for (int j = 0; i + j < 4; j++) {
            u64 t = (u64)a[i] * b[j] + o[i + j] + carry;
            o[i + j] = (u32)t; carry = t >> 32;
        }
    }
    r[0] = o[0] | ((u64)o[1] << 32); r[1] = o[2] | ((u64)o[3] << 32);
}

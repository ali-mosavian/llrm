// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// The FFT stage of stft_single on an i486: the pre-RA scheduler counted no x87 value, so it loaded ar and ai first, ran the x87 stack to its
// eighth slot and spilled one (`fstp dword ptr [esp+12]`, +11 clocks a trip; stft_single bq32 O2 6.3M clocks). The pin tracks the FP stack as a
// seven-register set and loads ar and ai after the products that wait for them.
float tw_re[128], tw_im[128], re[256], im[256];

// CHECK-LABEL: stage
// CHECK-NOT: fstp dword ptr [esp+
void stage(int i, int half, int stride)
{
    int j;
    float ar, ai, br, bi, tr, ti;
    for (j = 0; j < half; ++j) {
        ar = re[i + j];
        ai = im[i + j];
        br = re[i + j + half];
        bi = im[i + j + half];
        tr = tw_re[j * stride] * br - tw_im[j * stride] * bi;
        ti = tw_re[j * stride] * bi + tw_im[j * stride] * br;
        re[i + j] = (ar + tr) * 0.5f;
        im[i + j] = (ai + ti) * 0.5f;
        re[i + j + half] = (ar - tr) * 0.5f;
        im[i + j + half] = (ai - ti) * 0.5f;
    }
}

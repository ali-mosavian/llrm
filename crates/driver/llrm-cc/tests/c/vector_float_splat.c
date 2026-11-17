/* A pair of int counters converted at once: the `<2 x i32>` splat of `k` widens to `<4 x i32>` with undefined upper lanes.
   Built lane by lane it went through the stack (`mov [rsp-24], edi` twice, then a `movaps` load); the pin's
   `BuildVectorSDNode::getSplatValue` ignores undefined lanes: `movd`, `pshufd`. */
double A[24];
void f(long i)
{
    int k = (int)i;
    A[0] = (k + 0) * 0.5; A[1] = (k + 1) * 0.5;
    A[2] = (k + 2) * 0.5; A[3] = (k + 3) * 0.5;
}

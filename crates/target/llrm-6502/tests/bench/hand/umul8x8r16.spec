name: umul8x8r16
hand: -
kernel: k_umul8x8r16
args: B(200), B(123)
wide: 1
rel: -
expect: 24600
note: 8x8->16 unsigned multiply; hand takes LHS in A and RHS in ptr1 (not drivable directly), bench measures C side only

name: imul8x8r16
hand: -
kernel: k_imul8x8r16
args: B(156), B(123)
wide: 1
rel: -
expect: 53236
note: 8x8->16 signed multiply (156 = -100 as signed char; -12300 = 53236 as 16 bits); hand takes LHS in A and RHS in ptr1 (not drivable directly), bench measures C side only

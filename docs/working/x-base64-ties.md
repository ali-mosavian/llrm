# x_base64 -O1 (m32, i486): the two ties that flip in #4009

First stage that differs between the binary before #4009 and the one after is TwoAddress (`ISEL_DUMP=1`):

```
-      0x001f  v27 := mov v26
-      0x001f  v27 := or v27, v24
+      0x001f  v27 := mov v24
+      0x001f  v27 := or v27, v26
-      0x001b  v24 := mov v23
-      0x001b  v24 := or v24, v21
+      0x001b  v24 := mov v21
+      0x001b  v24 := or v24, v23
```

`isProfitableToCommute` reads the earlier tied pairs already split. The pin's own -O1 output for the kernel has two `or`s of another shape, so it is no comparison.

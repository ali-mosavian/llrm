' A 6000-byte frame outgrew the 2 KB stack the runtime links (BC crashes); the object adds the stack.
DECLARE SUB big ()
big
SUB big
  DIM s AS STRING * 6000
  MID$(s, 5999, 1) = "x"
  PRINT MID$(s, 5999, 1)
END SUB

' bc: /O
DECLARE SUB big ()
big
SUB big
  DIM s AS STRING * 2000
  MID$(s, 1999, 1) = "x"
  PRINT MID$(s, 1999, 1)
END SUB

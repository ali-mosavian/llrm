' PRINT USING with numeric fields: digits, points, commas, signs, $$, **,
' **$, exponents, and numbers that do not fit.
PRINT USING "###"; 5; 42; 123; 1234; -5; -42; -123; 0
PRINT USING "###.##"; 1.005; 2.675; 3.14159; -0.5; 0; 123.456; 1234.5
PRINT USING ".##"; .5; .125; 0; 1.5; -.5
PRINT USING "#.##"; .5; 0; 12; -3.14159
PRINT USING "#,###,#00"; 12; 1234; 12345
PRINT USING "#,###,#"; 12; 1234; 1234567; 12345678
PRINT USING "+###.#"; 5; -5; 12.34; 0
PRINT USING "###.#+"; 5; -5; 12.34
PRINT USING "###.#-"; 5; -5; 12.34
PRINT USING "$$###.##"; 5; 123.456; -5; 12345.6
PRINT USING "**###.##"; 5; 123.456; -5; 12345.6
PRINT USING "**$###.##"; 5; 123.456; -5
PRINT USING "###^^^^"; 12345; 0; 5
PRINT USING "#.##^^^^"; 12345; .000123; -1.5; 0
PRINT USING "##.#^^^^^"; 12345; 1E-100
PRINT USING "###"; 1E10; 99999; 1000
PRINT USING "# #.# #"; 1; 2.5; 3; 4; 5.5; 6
PRINT USING "Value: ##.# units"; 3
PRINT USING "[##] "; 1; 2; 3
PRINT USING "_##_.#"; 5
PRINT USING "#"; 4.5; 5.5; 0.5; 1.5; 2.5; 9.5; 99.5
PRINT USING "##.##"; 1.005!; 2.675!; 0.125!; 1D-3
PRINT USING "#######.##"; 1234567.891#; 123456789#; 1234567.894#
PRINT USING "###"; 5%; -5%; 32767%; 70000&

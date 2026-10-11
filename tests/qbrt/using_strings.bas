' PRINT USING with string fields, text between fields, the format used again,
' and a statement left open by a semicolon.
PRINT USING "!"; "hello"; "x"; ""
PRINT USING "\  \"; "ab"; "abcdef"; "abcd"; ""
PRINT USING "&"; "whole"; "x"; ""
PRINT USING "\\"; "xy"; "z"
PRINT USING "Name: ! Score: ##"; "Bob"; 5; "Al"; 77
PRINT USING "##: \    \|"; 1; "one"; 2; "two"; 3; "three"
PRINT USING "## \ \ ###.#"; 5; "ab"; 2.5
PRINT USING "##%"; 50; 100
PRINT USING "###"; 1; 2; 3;
PRINT "next"
PRINT USING "<&>"; "a"; "bc";
PRINT USING "[#]"; 7
PRINT USING "[#]"; 8;
PRINT USING "-#"; 5
PRINT USING "#-"; 5
PRINT USING "# # #"; 1; 2
PRINT USING "no fields #"; 1; 2

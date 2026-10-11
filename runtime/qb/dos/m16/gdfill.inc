FillBox         struc
                dst             dword   ?
                rows            word    ?
                middle          word    ?
                steps           word    2 dup (?)
                phase           word    ?
                edges           word    ?
                ;; the and in the low byte, the xor in the high
                left            word    ?
                right           word    ?
FillBox         ends

FillLine        struc
                dst             dword   ?
                ;; pixels: the longer side and one
                count           word    ?
                decision        word    ?
                minor4          word    ?
                step4           word    ?
                ;; bytes to the next row (CGA: 80, down; -80, up)
                ystep           word    ?
                ;; the 16 bits of the line style, the first pixel's the high bit
                style           word    ?
                ;; 1 along x, 0 along y, 2 straight down or up
                x_major         word    ?
                ;; the first pixel's mask in its byte (EGA: one bit; CGA: its
                ;; pixel's bits)
                pmask           word    ?
                bpp             word    ?               ;; CGA: bits a pixel
                ;; the byte of the colour: CGA all its pixels, EGA the nibble
                color           word    ?
FillLine        ends

FillXfer        struc
                screen          dword   ?
                array           dword   ?
                rows            word    ?
                sbytes          word    ?
                abytes          word    ?
                stride          word    ?
                shift           word    ?
                first           word    ?
                last            word    ?
                step            word    ?
                bank            word    ?
FillXfer        ends

FillScan        struc
                at              dword   ?
                count           word    ?
                c1              word    ?
                c2              word    ?
                flags           word    ?
                first           word    ?
                last            word    ?
                middle          word    ?
                found           word    ?
                hits            word    ?
                x               word    ?
                to              word    ?
                left            word    ?
                right           word    ?
                limit           word    ?
FillScan        ends

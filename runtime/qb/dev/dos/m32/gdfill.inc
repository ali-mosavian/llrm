FillBox         struc
                dst             dword   ?
                rows            dword   ?
                middle          dword   ?
                steps           dword   2 dup (?)
                phase           dword   ?
                edges           dword   ?
                ;; the and in the low byte, the xor in the next
                left            dword   ?
                right           dword   ?
FillBox         ends

FillLine        struc
                dst             dword   ?
                ;; pixels: the longer side and one
                count           dword   ?
                decision        dword   ?
                minor4          dword   ?
                step4           dword   ?
                ;; bytes to the next row (CGA: 80, down; -80, up)
                ystep           dword   ?
                ;; the 16 bits of the line style, the first pixel's the high bit
                style           dword   ?
                ;; 1 along x, 0 along y, 2 straight down or up
                x_major         dword   ?
                ;; the first pixel's mask in its byte (EGA: one bit; CGA: its
                ;; pixel's bits)
                pmask           dword   ?
                bpp             dword   ?               ;; CGA: bits a pixel
                ;; the byte of the colour: CGA all its pixels, EGA the nibble
                color           dword   ?
FillLine        ends

FillXfer        struc
                screen          dword   ?
                array           dword   ?
                rows            dword   ?
                sbytes          dword   ?
                abytes          dword   ?
                stride          dword   ?
                shift           dword   ?
                first           dword   ?
                last            dword   ?
                step            dword   ?
                bank            dword   ?
FillXfer        ends

FillScan        struc
                at              dword   ?
                count           dword   ?
                c1              dword   ?
                c2              dword   ?
                flags           dword   ?
                first           dword   ?
                last            dword   ?
                middle          dword   ?
                found           dword   ?
                hits            dword   ?
                x               dword    ?
                to              dword    ?
                left            dword    ?
                right           dword    ?
                limit           dword    ?
FillScan        ends

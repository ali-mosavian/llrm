' flags: -Os | -O2
' A SUB's locals are zero when the runtime builds the frame (B$ENRA fills it),
' wherever the frame layout puts them among the spill slots: forty longs, kept
' in memory by a by-reference call, are read before they are written and then
' summed in a loop that keeps more values live than there are registers.
DEFINT A-Z
DECLARE SUB Probe (n)
DECLARE SUB Keep (x&)
Probe 3
END

SUB Keep (x&)
END SUB

SUB Probe (n)
    PRINT "U"; v1&; v2&; v3&; v4&; v5&; v6&; v40&
    Keep v1&
    Keep v2&
    Keep v3&
    Keep v4&
    Keep v5&
    Keep v6&
    Keep v7&
    Keep v8&
    Keep v9&
    Keep v10&
    Keep v11&
    Keep v12&
    Keep v13&
    Keep v14&
    Keep v15&
    Keep v16&
    Keep v17&
    Keep v18&
    Keep v19&
    Keep v20&
    Keep v21&
    Keep v22&
    Keep v23&
    Keep v24&
    Keep v25&
    Keep v26&
    Keep v27&
    Keep v28&
    Keep v29&
    Keep v30&
    Keep v31&
    Keep v32&
    Keep v33&
    Keep v34&
    Keep v35&
    Keep v36&
    Keep v37&
    Keep v38&
    Keep v39&
    Keep v40&
    FOR i = 1 TO n
        w1& = w1& + i * 2 + v1&
        w2& = w2& + i * 3 + v2&
        w3& = w3& + i * 4 + v3&
        w4& = w4& + i * 5 + v4&
        w5& = w5& + i * 6 + v5&
        w6& = w6& + i * 7 + v6&
        w7& = w7& + i * 8 + v7&
        w8& = w8& + i * 9 + v8&
        w9& = w9& + i * 10 + v9&
        w10& = w10& + i * 11 + v10&
        w11& = w11& + i * 12 + v11&
        w12& = w12& + i * 13 + v12&
        v1& = v1& + i
        v2& = v2& + v1&
        v3& = v3& + v2&
        v4& = v4& + v3&
        v5& = v5& + v4&
        v6& = v6& + v5&
        v7& = v7& + v6&
        v8& = v8& + v7&
        v9& = v9& + v8&
        v10& = v10& + v9&
        v11& = v11& + v10&
        v12& = v12& + v11&
        v13& = v13& + v12&
        v14& = v14& + v13&
        v15& = v15& + v14&
        v16& = v16& + v15&
        v17& = v17& + v16&
        v18& = v18& + v17&
        v19& = v19& + v18&
        v20& = v20& + v19&
        v21& = v21& + v20&
        v22& = v22& + v21&
        v23& = v23& + v22&
        v24& = v24& + v23&
        v25& = v25& + v24&
        v26& = v26& + v25&
        v27& = v27& + v26&
        v28& = v28& + v27&
        v29& = v29& + v28&
        v30& = v30& + v29&
        v31& = v31& + v30&
        v32& = v32& + v31&
        v33& = v33& + v32&
        v34& = v34& + v33&
        v35& = v35& + v34&
        v36& = v36& + v35&
        v37& = v37& + v36&
        v38& = v38& + v37&
        v39& = v39& + v38&
        v40& = v40& + v39&
    NEXT
    PRINT "S"; v1&; v2&; v3&; v4&; v5&; v6&; v40&
    t& = w1& + w2& + w3& + w4& + w5& + w6& + w7& + w8& + w9& + w10& + w11& + w12& + v1& + v2& + v3& + v4& + v5& + v6& + v7& + v8& + v9& + v10& + v11& + v12& + v13& + v14& + v15& + v16& + v17& + v18& + v19& + v20& + v21& + v22& + v23& + v24& + v25& + v26& + v27& + v28& + v29& + v30& + v31& + v32& + v33& + v34& + v35& + v36& + v37& + v38& + v39& + v40&
    PRINT "T"; t&
END SUB

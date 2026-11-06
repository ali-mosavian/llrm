' flags: -O2
' Thirty-six chained sums in a loop: the closed form of the last is a
' polynomial of degree 36, whose binomial coefficients multiply at 66 bits.
' x86 has no 128-bit multiply call (__multi3 is 64-bit only), so the multiply
' is expanded by hand; it was refused ("expand multiply without a widening
' multiply").
DEFINT A-Z
DECLARE SUB Probe (n)
Probe 3
END

SUB Probe (n)
    FOR i = 1 TO n
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
    NEXT
    t& = v1& + v2& + v3& + v4& + v5& + v6& + v7& + v8& + v9& + v10& + v11& + v12& + v13& + v14& + v15& + v16& + v17& + v18& + v19& + v20& + v21& + v22& + v23& + v24& + v25& + v26& + v27& + v28& + v29& + v30& + v31& + v32& + v33& + v34& + v35& + v36&
    PRINT "T"; t&
END SUB

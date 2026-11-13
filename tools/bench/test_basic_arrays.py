import basic_arrays
import bench
import m16_owner_table as table
import pytest


def cell(code, clocks, data=0, bss=0):
    return {"code_bytes": code, "clocks": clocks, "data_bytes": data, "bss_bytes": bss}


def test_a_bound_that_is_an_expression_is_allocated_at_run_time_by_the_reference_not_by_llrm():
    """stft_fixed's `DIM SHARED re(N - 1) AS LONG` is in llrm's data and in no reference's image: 4677 B static read WORSE than bc's 161."""
    found = {one.name: one for one in basic_arrays.arrays("CONST N = 256\nDEFINT A-Z\nDIM SHARED re(N - 1) AS LONG\nDIM SHARED t(N), u(-3 TO 3), v(7)\n")}
    assert (found["re"].bytes, found["re"].simple_bounds) == (1024, False)
    assert (found["t"].bytes, found["t"].simple_bounds) == (514, True), "a bare CONST is a static bound, default type INTEGER by DEFINT"
    assert (found["u"].bytes, found["u"].simple_bounds) == (14, True)
    assert found["v"].simple_bounds


def test_dynamic_and_redim_are_run_time_for_every_side_and_procedure_arrays_are_not_static(tmp_path):
    source = tmp_path / "p.bas"
    source.write_text("DIM SHARED a(9) AS INTEGER\nREDIM b(0 TO 99) AS LONG\n'$DYNAMIC\nDIM SHARED c(9) AS INTEGER\nSUB f\n  DIM d(99) AS LONG\nEND SUB\n")
    assert basic_arrays.run_time_bytes(source, "llrm") == 400 + 20
    assert basic_arrays.run_time_bytes(source, "reference") == 400 + 20


def test_an_element_type_it_cannot_size_is_refused_not_guessed():
    with pytest.raises(ValueError):
        basic_arrays.arrays("DIM SHARED planes(1) AS Plane\n")


@pytest.mark.parametrize("program,llrm,reference", [("stft_fixed", 0, 4608), ("jpeg_fixed", 0, 24576), ("tile", 0, 0), ("nbody_fixed", 0, 0), ("particle", 1212, 1212)])
def test_the_run_time_arrays_of_the_corpus_match_the_maps(program, llrm, reference):
    """Audited against the BC maps (2026-11-13): static minus the held arrays is the scalars (13-1400 B) for bc, pds71 and vbdos on every program with arrays."""
    assert (bench.run_time_arrays(program, "bas"), bench.run_time_arrays(program, "bc")) == (llrm, reference)


def test_a_reference_that_allocates_the_arrays_llrm_holds_is_not_smaller_for_it():
    """stft_fixed bas: llrm 4677 B static against bc 161 + 4608 allocated at run time = 4769; the bare data + bss compared 4677 > 161."""
    measured = {"stft_fixed/bas/O2": cell(900, 10000, 4677), "stft_fixed/bc/O2": cell(2000, 90000, 31, 130), "stft_fixed/pds71/O2": cell(2100, 91000, 29, 130), "stft_fixed/vbdos/O2": cell(2200, 92000, 35, 136)}
    found, _ = table.rows(measured)
    row = next(one for one in found if one[:3] == ("stft_fixed", "bas", "O2"))
    assert "static" not in row[11], row

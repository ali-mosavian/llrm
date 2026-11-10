"""schedtable.py: the pin's scheduling model, or an error; never a guess."""

import importlib.util
import json
import re
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("schedtable", HERE / "schedtable.py")
schedtable = importlib.util.module_from_spec(spec)
sys.modules["schedtable"] = schedtable
spec.loader.exec_module(schedtable)

# A model with one valid class (1), one invalid (0), one variant (2) and the tables they index.
SUBTARGET = """\
extern const llvm::MCWriteProcResEntry AArch64WriteProcResTable[] = {
  { 0,  0,  0 }, // Invalid
  { 1,  1,   0}, // #1
};
extern const llvm::MCWriteLatencyEntry AArch64WriteLatencyTable[] = {
  { 0,  0}, // Invalid
  { 3,  0}, // #1 WriteI
};
extern const llvm::MCReadAdvanceEntry AArch64ReadAdvanceTable[] = {
  {0,  0,  0}, // Invalid
  {1,  0,  2}, // #1
};
static const llvm::MCSchedClassDesc TinyModelSchedClasses[] = {
  {DBGFIELD(1)  8191, false, false, false, 0, 0,  0, 0,  0, 0},
  {DBGFIELD(/*WriteI*/ 19) 1, false, false, false,  1,  1,  1, 1, 1, 1}, // #1
  {DBGFIELD(/*WriteV*/ 38) 8190, false, false, false,  0,  0,  0, 0, 0, 0}, // #2
}; // TinyModelSchedClasses
static const unsigned TinyModelProcResourceSubUnits[] = {
  0,  // Invalid
};

// {Name, NumUnits, SuperIdx, BufferSize, SubUnitsIdxBegin}
static const llvm::MCProcResourceDesc TinyModelProcResources[] = {
  {"InvalidUnit", 0, 0, 0, 0},
  {"TinyUnitALU", 2, 0, -1, nullptr}, // #1
};
extern const llvm::MCSchedModel AArch64SchedModels[] = {
{ // TinyModel
  2, // IssueWidth
  0, // MicroOpBufferSize
  MCSchedModel::DefaultLoopMicroOpBufferSize,
  3, // LoadLatency
  MCSchedModel::DefaultHighLatency,
  MCSchedModel::DefaultMispredictPenalty,
  true, // PostRAScheduler
  COMPLETE, // CompleteModel
  false, // EnableIntervals
  1, // Processor ID
  TinyModelProcResources,
  TinyModelSchedClasses,
  2,
  3,
  DBGVAL_OR_NULLPTR(&AArch64SchedClassNames), // SchedClassNames
  nullptr, // No Itinerary
  nullptr // No extra processor descriptor
  },
};
unsigned resolveVariantSchedClassImpl(unsigned SchedClass,
    const MCInst *MI, const MCInstrInfo *MCII, const MCSubtargetInfo &STI, unsigned CPUID) {
  switch (SchedClass) {
  case 2: // WriteV
    if (CPUID == 1) { // TinyModel
BODY
    }
    break;
  }
  return 0;
}
"""
BODY = """\
      if (AArch64_MC::hasShiftedReg(*MI))
        return 1; // WriteI
      return 0; // none"""

INSTRINFO = """\
    { 10,\t2,\t1,\t4,\t1,\t0,\t0,\tTinyOpInfoBase + 0,\t0,\t0, 0x0ULL },  // AAA
    { 11,\t2,\t1,\t4,\t0,\t0,\t0,\tTinyOpInfoBase + 0,\t0,\t0, 0x0ULL },  // BBB
    { 12,\t2,\t1,\t4,\t2,\t0,\t0,\tTinyOpInfoBase + 0,\t0,\t0|(1ULL<<MCID::Rematerializable), 0x0ULL },  // CCC
    { 13,\t2,\t1,\t0,\t0,\t0,\t0,\tTinyOpInfoBase + 0,\t0,\t0|(1ULL<<MCID::Pseudo), 0x0ULL },  // PPP
"""

MCSCHEDULE = """\
  static constexpr unsigned DefaultIssueWidth = 1;
  static constexpr unsigned DefaultMicroOpBufferSize = 0;
  static constexpr unsigned DefaultLoopMicroOpBufferSize = 0;
  static constexpr unsigned DefaultLoadLatency = 4;
  static constexpr unsigned DefaultHighLatency = 10;
  static constexpr unsigned DefaultMispredictPenalty = 10;
"""


def subtarget(body=BODY, complete="false"):
    return SUBTARGET.replace("BODY", body).replace("COMPLETE", complete)


def generate(sub=None, scope=("AAA", "BBB", "CCC", "PPP")):
    sub = sub or subtarget()
    models = schedtable.parse_models(sub, schedtable.parse_defaults(MCSCHEDULE))
    classes = schedtable.parse_instr_classes(INSTRINFO)
    return schedtable.render(models, {"generic": "TinyModel"}, classes, sub, {n: classes[n].number for n in scope})


def test_a_model_is_read_with_its_defaults_filled_from_mcschedule():
    model = schedtable.parse_models(subtarget(), schedtable.parse_defaults(MCSCHEDULE))["TinyModel"]
    assert (model.issue_width, model.micro_op_buffer_size, model.loop_micro_op_buffer_size) == (2, 0, 0)
    assert (model.load_latency, model.high_latency, model.mispredict_penalty) == (3, 10, 10)


def test_regenerating_gives_the_same_bytes():
    assert generate() == generate()


def test_the_opcode_with_a_variant_class_gets_its_arms():
    text = generate()
    assert "SchedPred::HasShiftedReg" in text and "VariantArm" in text


def test_a_predicate_the_generator_does_not_know_is_refused():
    """An unknown predicate in a variant class would have been guessed true or false."""
    with pytest.raises(schedtable.SchedError, match="predicate"):
        generate(subtarget("      if (STI.hasFeature(AArch64::FeatureSME))\n        return 1;\n      return 0;"))


def test_a_statement_the_generator_does_not_read_is_refused():
    with pytest.raises(schedtable.SchedError, match="variant"):
        generate(subtarget("      Foo(*MI);\n      return 1;"))


def test_a_complete_model_with_an_invalid_class_for_an_opcode_is_refused():
    """BBB is class 0, invalid: a complete model must name every opcode."""
    with pytest.raises(schedtable.SchedError, match="BBB"):
        generate(subtarget(complete="true"))


def test_an_opcode_the_complete_model_marks_unsupported_is_kept_as_unsupported():
    """The pin's models mark WriteAtomic Unsupported; refusing those would refuse every LSE atomic."""
    sub = subtarget(complete="true")
    models = schedtable.parse_models(sub, schedtable.parse_defaults(MCSCHEDULE))
    classes = schedtable.parse_instr_classes(INSTRINFO)
    numbers = {n: classes[n].number for n in ("AAA", "BBB", "CCC", "PPP")}
    text = schedtable.render(models, {"generic": "TinyModel"}, classes, sub, numbers, {"TinyModel": {"BBB"}})
    assert "SchedClass::Unsupported" in text and "SchedClass::Unmodeled" not in text


def test_the_unsupported_opcodes_are_those_with_a_write_the_model_marks():
    data = {
        "W": {"!superclasses": ["SchedWrite"]},
        "R": {"!superclasses": ["WriteRes"], "SchedModel": {"def": "M"}, "Unsupported": 1, "WriteType": {"def": "W"}},
        "O": {"!superclasses": ["WriteRes"], "SchedModel": {"def": "N"}, "Unsupported": 1, "WriteType": {"def": "W"}},
        "X": {"SchedRW": [{"def": "W"}]},
        "Y": {"SchedRW": [{"def": "Z"}]},
    }
    assert schedtable.unsupported_opcodes(data, {"M"}, ["X", "Y"]) == {"M": {"X"}}


def twin_classes():
    """A fourth class with class 1's numbers, as the pin has many: a row is stored once."""
    sub = subtarget().replace(
        "}; // TinyModelSchedClasses",
        "  {DBGFIELD(/*WriteI2*/ 50) 1, false, false, false,  1,  1,  1, 1, 1, 1}, // #3\n}; // TinyModelSchedClasses",
    )
    return sub.replace("  2,\n  3,\n  DBGVAL", "  2,\n  4,\n  DBGVAL")


def test_classes_with_the_same_numbers_share_one_row():
    """Without sharing the 820 resolved classes of the arm64 models were 354 KB of source (83 KB shared)."""
    sub = twin_classes()
    models = schedtable.parse_models(sub, schedtable.parse_defaults(MCSCHEDULE))
    classes = schedtable.parse_instr_classes(INSTRINFO + "    { 14,\t2,\t1,\t4,\t3,\t0,\t0,\tTinyOpInfoBase + 0,\t0,\t0, 0x0ULL },  // DDD\n")
    numbers = {n: classes[n].number for n in ("AAA", "DDD")}
    text = schedtable.render(models, {"generic": "TinyModel"}, classes, sub, numbers)
    assert text.count("SchedClass::Resolved(0)") == 2, "both classes are emitted"
    assert "static CLASS_ROWS: [ClassRow; 1]" in text


def test_a_slice_is_stored_once_however_many_classes_name_it():
    pool = schedtable.Pool("WriteLatency")
    assert pool.span(["a", "b"]) == (0, 2) and pool.span(["c"]) == (2, 1) and pool.span(["a", "b"]) == (0, 2)
    assert pool.items == ["a", "b", "c"]


def test_a_slice_longer_than_a_span_holds_is_refused():
    with pytest.raises(schedtable.SchedError, match="256"):
        schedtable.Pool("WriteProcRes").span(["x"] * 256)


def test_a_class_index_past_the_table_is_refused():
    broken = subtarget().replace("1,  1,  1, 1, 1, 1}, // #1", "1,  9,  1, 1, 1, 1}, // #1")
    with pytest.raises(schedtable.SchedError, match="WriteProcRes"):
        generate(broken)


def test_an_opcode_in_the_generated_instrinfo_but_in_no_pin_row_is_refused(tmp_path):
    """The generic opcodes (COPY, REG_SEQUENCE) are named by the pin's rows only; a number with no row has no class."""
    (tmp_path / "instrinfo.rs").write_text("    (10, InstrDesc { }),\n    (99, InstrDesc { }),\n")
    (tmp_path / "ids.rs").write_text("pub mod opcode {\n}\n")
    classes = schedtable.parse_instr_classes(INSTRINFO)
    with pytest.raises(schedtable.SchedError, match="99"):
        schedtable.opcode_numbers(tmp_path, classes)


def test_the_generic_opcodes_are_named_from_the_pin_rows(tmp_path):
    (tmp_path / "instrinfo.rs").write_text("    (13, InstrDesc { }),\n")
    (tmp_path / "ids.rs").write_text("pub mod opcode {\n    pub const AAA: u16 = 10;\n}\n")
    assert schedtable.opcode_numbers(tmp_path, schedtable.parse_instr_classes(INSTRINFO)) == {"PPP": 13}


def test_an_ids_number_the_pin_row_disagrees_with_is_refused(tmp_path):
    (tmp_path / "instrinfo.rs").write_text("    (10, InstrDesc { }),\n")
    (tmp_path / "ids.rs").write_text("pub mod opcode {\n    pub const AAA: u16 = 77;\n}\n")
    with pytest.raises(schedtable.SchedError, match="AAA"):
        schedtable.opcode_numbers(tmp_path, schedtable.parse_instr_classes(INSTRINFO))


def test_the_regen_script_runs_the_generator_for_a_record_that_asks():
    text = (HERE / "regen-target.sh").read_text()
    assert "schedtable.py" in text and "sched_cpus" in text


def test_the_pin_models_of_the_arm64_record_cover_what_the_dag_emits(tmp_path):
    """The checked-in tables name every opcode the DAG tables emit, against the pin's models."""
    out = HERE.parent.parent / "crates/backend/llrm-dagisel/src/generated/aarch64"
    if not (out / "sched.rs").exists():
        pytest.skip("not generated yet")
    text = (out / "sched.rs").read_text()
    emitted = len(re.findall(r"\(\d+, InstrDesc ", (out / "instrinfo.rs").read_text()))
    assert len(re.findall(r"^\s+\(\d+, \d+\),$", text, re.M)) == emitted

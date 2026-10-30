"""Imported by Python at start-up from PYTHONPATH: the base's bench runs with the `fork` start method, as it did when it was written (Python 3.13).
Python 3.14 defaults to `forkserver`; on CI the base leg's nested process pools then sat at 0 CPU for 10 min (run 6) and here ran 3x slower."""
import multiprocessing

multiprocessing.set_start_method("fork", force=True)

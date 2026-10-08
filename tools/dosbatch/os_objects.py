"""Print the OS layer's objects for a language program, made in WORK: `field=path` per line (dosbatch.os_objects).

    os_objects.py TARGET LANGUAGE WORK
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402

if len(sys.argv) != 4:
    sys.exit(__doc__)
for field, path in dosbatch.os_objects(sys.argv[1], sys.argv[2], Path(sys.argv[3])).items():
    print(f"{field}={path}")

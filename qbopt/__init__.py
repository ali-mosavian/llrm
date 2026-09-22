from pathlib import Path


def _refuse_another_checkout() -> None:
    """The shared venv's editable install names one checkout; a script run from another must not use it."""
    here = Path(__file__).resolve().parent.parent
    for folder in (Path.cwd(), *Path.cwd().parents):
        if (folder / "qbopt" / "__init__.py").is_file():
            if folder.resolve() != here:
                raise ImportError(f"qbopt from another checkout ({here}) while working in {folder}; set PYTHONPATH")
            return


_refuse_another_checkout()

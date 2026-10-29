# Test config for the LLVM pin's .ll tests run through `opt`. Loaded by the
# `lit.site.cfg.py` that `run.py` writes, which sets the `config.llrm_*` values.
import os
import re

import lit.formats

config.name = "llrm-pin-tests"
config.test_format = lit.formats.ShTest(execute_external=False)
config.suffixes = [".ll"]
config.excludes = ["Inputs", "CMakeLists.txt", "README.txt", "LICENSE.txt"]
config.test_source_root = config.llrm_test_root
config.test_exec_root = config.llrm_exec_root
config.environment["PATH"] = config.llrm_bin_dir + os.pathsep + os.environ.get("PATH", "")
config.environment["LC_ALL"] = "C"

# The tools by name, as the pin's RUN lines spell them.
for tool in ("opt", "FileCheck", "not", "count"):
    config.substitutions.append((r"(?<![\w./%-])" + tool + r"(?![\w.-])", os.path.join(config.llrm_bin_dir, tool)))

config.available_features.update(
    ["shell", "native", "system-linux", "target=" + config.llrm_host_triple, *config.llrm_features]
)
if config.llrm_asserts:
    config.available_features.add("asserts")
for target in config.llrm_targets:
    config.available_features.add(target.lower() + "-registered-target")

"""A fast batch took every queued PR and left out the ones that conflicted with an earlier member: a full smoke for the others each time.
`assemble` keeps the PRs that merge one after another, and the rest wait."""

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_squash as ps  # noqa: E402


def sh(repo: Path, *args: str) -> str:
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return done.stdout.strip()


def branch(repo: Path, name: str, path: str, text: str) -> str:
    sh(repo, "checkout", "-q", "-b", name, "main")
    (repo / path).write_text(text)
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", name)
    return sh(repo, "rev-parse", "HEAD")


def test_the_pr_that_conflicts_with_an_earlier_member_waits_and_the_third_joins(tmp_path):
    """Three PRs, the first two edit one line of one file: before, the batch took all three and #2 was left out after the smoke."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "shared.txt").write_text("one\ntwo\nthree\n")
    (repo / "other.txt").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {
        "1": branch(repo, "a", "shared.txt", "one\nTWO\nthree\n"),
        "2": branch(repo, "b", "shared.txt", "one\n2\nthree\n"),
        "3": branch(repo, "c", "other.txt", "y\n"),
    }
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(["1", "2", "3"], heads, base="main", cwd=repo)
    assert kept == ["1", "3"]
    assert list(waits) == ["2"] and "shared.txt" in waits["2"]


def test_the_limit_counts_kept_prs_only(tmp_path):
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "f").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {str(n): branch(repo, f"b{n}", f"f{n}", "y\n") for n in range(1, 5)}
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(list(heads), heads, base="main", cwd=repo, limit=2)
    assert kept == ["1", "2"] and not waits


def test_a_pr_whose_build_fails_alone_is_dropped_before_assembly_and_checked_once(tmp_path):
    """The 15:34 batch spent 29.6 of 48 minutes rerunning PRs that failed alone: `alone` drops them with the reason, and a head already judged is not built again."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    good, bad = branch(repo, "good", "good.txt", "ok\n"), branch(repo, "bad", "bad.txt", "broken\n")
    built = []

    def refusals(head: str, base: str) -> list[str]:
        built.append(head)
        return ["does not build as a whole: error[E0425]"] if head == bad else []

    heads = {"1": good, "2": bad}
    kept, dropped = ps.alone(["1", "2"], heads, refusals=refusals, cache=tmp_path / "cache", base_sha="b" * 40)
    assert kept == ["1"] and "E0425" in dropped["2"] and "2" not in kept
    ps.alone(["1", "2"], heads, refusals=refusals, cache=tmp_path / "cache", base_sha="b" * 40)
    assert built == [good, bad], built


def test_a_pr_that_merges_but_does_not_build_alone_is_refused_with_the_compiler_text(tmp_path):
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "bad", "b.rs", "fn(\n")
    sh(repo, "checkout", "-q", "main")
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "error: expected identifier", differs=lambda *_: set(), scanners=lambda _: set())
    assert why and "does not build as a whole" in why[0] and "expected identifier" in why[0], why


def test_a_did_not_run_scanner_red_alone_names_its_cause(tmp_path, monkeypatch):
    """Eleven PRs were dropped with `integration-did-not-run` and no reason: the cause was recorded but the alone refusal never printed it."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "ok", "b.txt", "b\n")
    sh(repo, "checkout", "-q", "main")
    monkeypatch.setitem(ps.parity_smoke_module().CAUSES, "integration-did-not-run", "No space left on device")
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), scanners=lambda _: {"integration-did-not-run"})
    assert why and "No space left on device" in why[0], why


def test_a_deleted_test_file_in_the_diff_is_not_a_scanner_target(tmp_path):
    """#4784 forked before tests/fill_loop_at_os.rs landed: its diff listed the file, `--test fill_loop_at_os` found no target, and the PR was dropped as integration-did-not-run."""
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "kept.rs").write_text("")
    files = ["tests/kept.rs", "tests/fill_loop_at_os.rs", "tests/sub/x.rs", "src/a.rs"]
    assert ps.own_test_targets(files, tmp_path) == {"kept"}


def test_a_refusal_of_a_head_the_pr_has_moved_off_does_not_drop_it(tmp_path):
    """#4993 was dropped at 21:04 for a merge conflict its 21:02 head no longer had: the check had started on the 20:54 head."""
    heads = {"4993": "old", "4991": "same"}
    refuse = lambda head, base: ["conflicts in pressuresink.md"]  # noqa: E731
    kept, dropped = ps.alone(["4993", "4991"], heads, refusals=refuse, current=lambda number: "new" if number == "4993" else "same")
    assert kept == ["4993"] and list(dropped) == ["4991"], (kept, dropped)


def test_a_docs_check_that_only_counts_lint_hits_is_not_a_failed_regeneration():
    """#4993 was refused for `4 problem(s)`: the baseline `no # Example` hits, with every page current."""
    lint = "crates/x/remidiom.rs: no # Example\n4 problem(s)\n"
    stale = "generated docs are out of date; run python3 tools/docs/docsbuild/build.py --sync\n  docs/optimizations/backend/a.md\n"
    assert ps.docs_stale_lines(lint) == [] and ps.docs_stale_lines(stale) == ["docs/optimizations/backend/a.md"]


def test_a_long_alone_refusal_is_written_whole_to_a_file_the_log_line_names(tmp_path):
    """#4991's refusal printed three failing tests and cut the third mid-name; the author could not tell which others failed."""
    why = "unit tests fail alone: " + ", ".join(f"unit llrm-qb::test_{i}_a_long_enough_name_to_need_cutting" for i in range(12))
    line = ps.alone_refusal_line("4991", why, "9c833bc12abc", tmp_path)
    assert len(line) < 400
    path = next(tmp_path.glob("4991-*.txt"))
    assert path.read_text().strip() == why and str(path) in line and f"{len(why)} chars" in line
    assert ps.alone_refusal_line("1", "short", "abc", tmp_path) == "1: REFUSED: alone: short"
def test_a_pr_that_leaves_generated_docs_stale_alone_is_refused_but_not_for_what_the_base_has_stale(tmp_path):
    """#4972 landed with 411 stale pages: LAND_FAST skips the precheck and the alone layer had no docs check."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "ok", "b.txt", "b\n")
    sh(repo, "checkout", "-q", "main")
    kw = dict(base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), scanners=lambda _: set(), units=lambda _: set())
    why = ps.alone_refusals(head, docs=lambda root: {"docs/a.md", "docs/b.md"}, docs_on_base=lambda: {"docs/a.md"}, **kw)
    assert why and "docs/b.md" in why[0] and "docs/a.md" not in why[0], why
    assert ps.alone_refusals(head, docs=lambda root: {"docs/a.md"}, docs_on_base=lambda: {"docs/a.md"}, **kw) == []


def test_an_alone_check_that_dies_with_system_exit_is_a_refusal_not_a_silent_pass():
    """A thread drops SystemExit without a word: #5008 got no verdict, was kept as checked (`4 checked, 0 dropped`) and its unreachable doc broke the 21:53 batch."""
    def refuse(head, base):
        if head == "bad":
            raise SystemExit("no failure set is cached for the head")
        return []

    kept, dropped = ps.alone(["1", "2"], {"1": "ok", "2": "bad"}, refusals=refuse, jobs=2)
    assert kept == ["1"] and "alone check crashed: SystemExit" in dropped["2"], (kept, dropped)


def test_a_batch_scanner_refusal_can_name_the_markdown_each_pr_adds(tmp_path):
    """#5008 added docs/working/combine-forward-load-rows.md and nothing linked it: the refusal said `docs_links` and named no PR or doc."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "readme.md").write_text("r\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    page = branch(repo, "page", "new.md", "x\n")
    other = branch(repo, "other", "src.rs", "y\n")
    assert ps.batch_blame({"5008": page, "5007": other}, "main", repo) == ["  #5008: A new.md"]
def test_a_head_failure_is_named_every_cycle_and_refused_once_it_is_two_hours_old(tmp_path, capsys):
    """docs_in_sync was red on the parity tip for hours behind `1 red`: the head's reds excused every PR without a name or an owner."""
    seen = tmp_path / "seen.json"
    red = "integration docs_in_sync::test_no_generated_page_is_stale"
    assert ps.excused_head_failures({red}, seen, 1000.0) == set()
    assert f"excused head failure: {red} (red on the head for 0.0 h" in capsys.readouterr().out
    assert ps.excused_head_failures({red}, seen, 1000.0 + 3600) == set()
    assert ps.excused_head_failures({red}, seen, 1000.0 + 3 * 3600) == {red}
    assert ps.excused_head_failures(set(), seen, 1000.0 + 4 * 3600) == set()
    assert ps.excused_head_failures({red}, seen, 1000.0 + 5 * 3600) == set(), "a name that went green is forgotten"


def test_two_prs_adding_a_test_of_one_name_do_not_share_a_batch(tmp_path):
    """#4678 and #4680 added the same test in different places: no conflict, so both joined, and the batch failed 'defined multiple times'."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "t.rs").write_text("fn first() {}\n\n\n\n\n\n\n\n\nfn last() {}\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    sh(repo, "checkout", "-qb", "a")
    (repo / "t.rs").write_text("fn first() {}\nfn a_frame_cell_is_not_encodable() {}\n\n\n\n\n\n\n\n\nfn last() {}\n")
    sh(repo, "commit", "-qam", "a")
    heads = {"1": sh(repo, "rev-parse", "HEAD")}
    sh(repo, "checkout", "-q", "main")
    sh(repo, "checkout", "-qb", "b")
    (repo / "t.rs").write_text("fn first() {}\n\n\n\n\n\n\n\n\nfn last() {}\nfn a_frame_cell_is_not_encodable() {}\n")
    sh(repo, "commit", "-qam", "b")
    heads["2"] = sh(repo, "rev-parse", "HEAD")
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(["1", "2"], heads, base="main", cwd=repo)
    assert kept == ["1"] and "a_frame_cell_is_not_encodable" in waits["2"] and "#1" in waits["2"]


def test_the_scanners_of_an_alone_check_run_in_the_scratch_tree_not_the_runners(tmp_path, monkeypatch):
    """The dry run printed `scanners: 2 targets, 0 red (tree 4d319defc)` for every PR: integration_failures defaulted to ROOT, so a PR red in its own tree passed."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "pr", "b.txt", "b\n")
    sh(repo, "checkout", "-q", "main")
    seen = []
    smoke = ps.parity_smoke_module()
    monkeypatch.setattr(smoke, "integration_failures", lambda targets, root=None, **_: seen.append(root) or set())
    monkeypatch.setattr(smoke, "workspace_graph", lambda root: ({}, {}))
    monkeypatch.setattr(smoke, "integration_map", lambda root=None: {})
    assert ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), units=lambda _: set()) == []
    assert seen == [repo], seen


def test_alone_counts_the_verdicts_it_read_from_the_cache(tmp_path):
    """The hit rate of the cache decides whether the filter pays: the second round reads every verdict of the first."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {"1": branch(repo, "x", "x.txt", "x\n")}
    ps.HITS[0] = 0
    ps.alone(["1"], heads, refusals=lambda h, b: [], cache=tmp_path / "cache")
    assert ps.HITS[0] == 0
    ps.alone(["1"], heads, refusals=lambda h, b: [], cache=tmp_path / "cache")
    assert ps.HITS[0] == 1


def test_a_build_refusal_names_the_error_of_the_prs_own_alone_build_not_the_batchs(tmp_path):
    """The 16:56 batch refused #4743 with `floatassign.rs: unused import live`, the error of #4648 in the same batch; #4743 alone built clean. The first PR breaks the build, the second is clean."""
    current = []

    def merge_alone(number: str) -> bool:
        current[:] = [number]
        return True

    def check() -> str:
        return "floatassign.rs:124:38: unused import: `live`" if current == ["4648"] else ""

    errors: dict[str, str] = {}
    assert ps.build_offenders(["4648", "4743"], merge_alone, check, errors) == ["4648"]
    assert errors == {"4648": "floatassign.rs:124:38: unused import: `live`"}


def test_a_pr_whose_only_issue_is_a_fmt_difference_is_kept(tmp_path):
    """The first `alone` precheck dropped #4736 and #4744 for `fmt.sh would change selectiondag/emit.rs`; the squash formats every PR, so the s3 pair started its batch without them."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "pr", "emit.rs", "fn  f( ){}\n")
    sh(repo, "checkout", "-q", "main")
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: {"emit.rs"}, scanners=lambda _: set(), units=lambda _: set())
    assert why == [], why


def test_a_test_the_base_fails_too_blames_no_pr_and_probes_none(tmp_path):
    """Batch 2 (20:07) merged 16 PRs on a base red in two llrm-codegen tests and probed every PR alone: each fails the base's test, so each looked guilty."""
    probed = []
    by_package = {"llrm-codegen": {"dag_function_calls::a_red_test"}}
    found = ps.unit_offenders(
        [str(n) for n in range(1, 17)],
        by_package,
        merge_alone=lambda number: probed.append(number) or True,
        failing=lambda package, tests: True,
        base_red=lambda wanted: {("llrm-codegen", "dag_function_calls::a_red_test")},
    )
    assert found == [] and probed == [], (found, probed)


def test_a_pr_is_probed_only_for_a_package_it_can_change_and_a_new_red_test_is_still_blamed(tmp_path):
    graph = {"llrm-codegen": {"llrm-mir"}, "llrm-mir": set(), "llrm-x86": {"llrm-codegen"}, "llrm-nib": {"llrm-mir"}}
    packages = {name: Path("/r/crates") / name for name in graph}
    files = {"1": ["crates/llrm-nib/src/a.rs"], "2": ["crates/llrm-mir/src/b.rs"], "3": ["Cargo.toml"]}
    root = Path("/r")
    affects = lambda number, package: ps.can_affect(files[number], package, packages, graph, root)  # noqa: E731
    probed = []
    found = ps.unit_offenders(
        ["1", "2", "3"],
        {"llrm-codegen": {"t"}},
        merge_alone=lambda number: probed.append(number) or True,
        failing=lambda package, tests: probed[-1] == "2",
        affects=affects,
    )
    assert probed == ["2", "3"], probed  # 1 changes llrm-nib, which llrm-codegen does not use; 3 is a file no package owns
    assert found == ["2"], found


def test_a_diff_with_non_utf8_bytes_is_read_not_a_crash(tmp_path):
    """The 20:07 batch ran unassembled: `added_names` died with UnicodeDecodeError 0xdb on a PR whose diff held CP437 bytes (a .BAS), and the runner fell back to the first 16."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    sh(repo, "checkout", "-q", "-b", "pr")
    (repo / "prog.bas").write_bytes(b"PRINT \"\xdb\xdb\xb0\"\n")
    (repo / "t.rs").write_text("fn a_new_test() {}\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "pr")
    assert ps.added_names("pr", "main", repo) == {"a_new_test"}
    assert ps.git("diff", "-U0", "main...pr", cwd=repo).count("PRINT") == 1


def test_alone_checks_run_in_parallel_and_the_unfinished_at_the_cap_are_kept_unfiltered(tmp_path):
    """Forty PRs at 3-5 minutes each, one after another, held the batch for hours: 4 checks of 0.4 s take about 0.4 s, and one still running at the cap goes to the batch."""
    import time

    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {str(n): branch(repo, f"b{n}", f"f{n}.txt", f"{n}\n") for n in range(1, 6)}

    def refusals(head: str, base: str) -> list[str]:
        time.sleep(30 if head == heads["5"] else 0.4)
        return ["does not build"] if head == heads["2"] else []

    started = time.monotonic()
    kept, dropped = ps.alone(list(heads), heads, refusals=refusals, jobs=5, cap=2)
    took = time.monotonic() - started
    assert took < 4, took
    assert kept == ["1", "3", "4", "5"], kept  # 5 never finished: unfiltered
    assert list(dropped) == ["2"], dropped


def _clean_pr(tmp_path):
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "pr", "x.rs", "fn x() {}\n")
    sh(repo, "checkout", "-q", "main")
    return repo, head


def test_a_unit_test_that_fails_on_the_pr_and_passes_on_the_base_blames_the_pr_alone(tmp_path):
    """Round 3 died on llrm-nib::test_pipeline::test_a_frame_larger_than_the_start_stack_adds_a_stack_segment and no PR was blamed: alone ran a build and scanners, no unit test."""
    repo, head = _clean_pr(tmp_path)
    red = "unit llrm-nib::test_pipeline::test_a_frame_larger_than_the_start_stack_adds_a_stack_segment"
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), scanners=lambda _: set(), units=lambda files: {red}, on_base=lambda wanted: set())
    assert why and red in why[0], why


def test_a_unit_test_the_base_fails_too_does_not_blame_the_pr(tmp_path):
    repo, head = _clean_pr(tmp_path)
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), scanners=lambda _: set(), units=lambda files: {"unit llrm-nib::t"}, on_base=lambda wanted: {("llrm-nib", "t")})
    assert why == [], why


def test_a_batch_that_fails_with_no_pr_blamed_is_halved_until_the_pair_is_named():
    """Two PRs that only fail together: no offender is named, the halves are tried, and the second of the pair is the one refused."""
    landed, refused = [], []
    prs = ["1", "2", "3", "4"]

    def passes(group: list[str]) -> bool:
        return not ({"2", "3"} <= set(landed) | set(group))

    ps.batch(prs, passes, landed.append, refused.append, offenders=lambda group: [])
    assert refused == ["3"] and landed == ["1", "2", "4"], (landed, refused)


def test_a_slot_checkout_keeps_its_path_and_two_processes_never_share_a_slot(tmp_path):
    """A fresh directory per PR made cargo rebuild everything (2-3 min a PR): the slot's checkout is reused; the filter and the warm-up each hold a slot of their own."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    work = tmp_path / "slots" / "alone-0" / "w"
    ps._alone_checkout(work, "main", repo)
    (work / "stray.txt").write_text("left by the last PR\n")
    ps._alone_checkout(work, "main", repo)
    assert work.is_dir() and not (work / "stray.txt").exists()
    first, lock_a = ps._claim_slot(tmp_path / "slots", 2, reserved=0)
    second, lock_b = ps._claim_slot(tmp_path / "slots", 2, reserved=0)
    assert {first, second} == {0, 1}
    lock_a.close()
    lock_b.close()


def test_the_alone_command_prints_the_surviving_numbers_on_stdout_and_nothing_else(monkeypatch, capsys):
    """The scanners' `scanners: 2 targets, 0 red (tree ...)` reached stdout, the runner read it as PR numbers, and no batch formed for 70 minutes."""

    def fake(numbers, heads, **_):
        print("scanners: 2 targets, 0 red (tree abc123)")
        return list(numbers), {}

    monkeypatch.setattr(ps, "alone", fake)
    monkeypatch.setattr(ps, "git", lambda *args, **_: "deadbeef")
    monkeypatch.setattr(sys, "argv", ["parity_squash.py", "alone", "11", "12"])
    assert ps.main() == 0
    out = capsys.readouterr()
    assert out.out.split() == ["11", "12"], out.out


def alone_scratch(tmp_path):
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "pr", "b.txt", "b\n")
    sh(repo, "checkout", "-q", "main")
    return repo, head


def test_an_alone_check_runs_every_scanner_not_the_two_always_on(tmp_path, monkeypatch):
    """Batch 18 died on llrm-mir's ratchet and selectiondag_registry in batches 16 and 17, and the alone pass passed their PRs: it ran docs_links and no_local_paths only."""
    repo, head = alone_scratch(tmp_path)
    seen = []
    smoke = ps.parity_smoke_module()
    monkeypatch.setattr(smoke, "integration_failures", lambda targets, root=None, **_: seen.extend(targets) or set())
    monkeypatch.setattr(smoke, "workspace_graph", lambda root: ({}, {}))
    monkeypatch.setattr(smoke, "integration_map", lambda root=None: {})
    ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), units=lambda _: set())
    assert {"selectiondag_registry", "no_todo", "docs_links"} <= set(seen), seen


def test_an_alone_check_runs_the_source_scanning_lib_tests_of_a_crate_upstream_of_the_change(tmp_path, monkeypatch):
    """#4871 added three id-keyed maps in llrm-transforms; the test that forbids them lives in llrm-mir, which transforms depends on, so a dependants run never ran it."""
    repo, head = alone_scratch(tmp_path)
    seen = []
    smoke = ps.parity_smoke_module()
    monkeypatch.setattr(smoke, "workspace_graph", lambda root: ({}, {}))
    monkeypatch.setattr(smoke, "touched_packages", lambda files, packages, root: {"llrm-transforms"})
    monkeypatch.setattr(smoke, "dependants_lib_failures", lambda packages, **_: seen.append(set(packages)) or set())
    ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), scanners=lambda f: set())
    assert seen and "llrm-mir" in seen[0], seen


def test_a_pr_that_makes_a_listed_unit_test_pass_warns_alone_and_so_does_a_listed_integration_target_it_reaches(tmp_path, monkeypatch):
    """An xpass is a warning, not a refusal (08daf86026); this test still asserted the old refusal and failed on parity alone: the 08:09 pass was refused for it. The integration target an xpass reaches is still run."""
    repo, head = alone_scratch(tmp_path)
    smoke = ps.parity_smoke_module()
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), scanners=lambda f: set(), units=lambda _: {"xpass unit llrm-x::t_a"})
    assert why == [], why

    seen = []
    import xfail

    monkeypatch.setattr(xfail, "load", lambda path=xfail.PATH: {"integration header_copy::t": {}})
    monkeypatch.setattr(smoke, "workspace_graph", lambda root: ({}, {}))
    monkeypatch.setattr(smoke, "touched_packages", lambda files, packages, root: {"llrm-codegen"})
    monkeypatch.setattr(smoke, "integration_map", lambda root=None: {"header_copy": {"llrm-codegen"}})
    monkeypatch.setattr(smoke, "integration_failures", lambda targets, root=None, **_: seen.extend(targets) or {"xpass integration header_copy::t"})
    monkeypatch.setattr(ps, "_scanners_failing_on_head", lambda targets: set())  # the real one reads and writes ~/scratch/results/headfail
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), units=lambda _: set())
    assert "header_copy" in seen and why == [], (seen, why)


def test_a_head_whose_tests_did_not_run_is_not_cached_as_its_failure_set(tmp_path):
    """The head 277c2651b0 was cached as ["integration-did-not-run"]: its twelve real header_copy reds were never recorded, so batch 17 saw them as the batch's own."""
    calls = []

    def compute():
        calls.append(1)
        return {"integration-did-not-run"} if len(calls) == 1 else {"integration header_copy::t"}

    assert ps.head_failures("abc", compute, cache=tmp_path) == {"integration-did-not-run"}
    assert ps.head_failures("abc", compute, cache=tmp_path) == {"integration header_copy::t"}
    assert ps.head_failures("abc", compute, cache=tmp_path) == {"integration header_copy::t"} and len(calls) == 2


def test_the_alone_command_runs_its_cargo_through_buildslot_with_two_jobs_whatever_the_runner_exports(monkeypatch):
    """The runner exports BUILDSLOT_BYPASS=1 and main() ran every command at the landing's priority: six of eight cargo processes at load 108 were alone-0..5 outside every slot."""
    import buildslot

    seen = {}

    def fake(numbers, heads, **kw):
        seen["slotted"] = buildslot.slotted(["cargo", "build"])
        seen["jobs"] = kw["jobs"]
        return list(numbers), {}

    monkeypatch.setenv("BUILDSLOT_BYPASS", "1")
    monkeypatch.setenv(buildslot.LANDING, "1")
    monkeypatch.delenv("ALONE_JOBS", raising=False)
    monkeypatch.setattr(buildslot, "tool", lambda: "buildslot")
    monkeypatch.setattr(ps, "alone", fake)
    monkeypatch.setattr(ps, "git", lambda *args, **_: "deadbeef")
    monkeypatch.setattr(ps, "preload_tools", lambda: None)
    monkeypatch.setattr(sys, "argv", ["parity_squash.py", "alone", "11"])
    assert ps.main() == 0
    assert seen == {"slotted": ["buildslot", "cargo", "build"], "jobs": 2}, seen


def test_each_offender_is_named_by_the_tests_that_failed_on_its_own_probe_with_its_own_panic():
    """The 10:24 batch refused #4898 #4918 #4926 #4927 #4936 with one identical list (the batch's whole failure set and a stale panic): five different failures read as one cause."""
    failed = {"1": {"a"}, "2": {"b"}, "3": set()}
    probed = []
    panics = {}

    def failing(package, tests):
        panics.clear()
        mine = failed[probed[-1]] & tests
        panics.update({t: f"panic of {t} on #{probed[-1]}" for t in mine})
        return mine

    own, reasons = {}, {}
    found = ps.unit_offenders(
        ["1", "2", "3"],
        {"llrm-x": {"a", "b", "c"}},
        merge_alone=lambda number: probed.append(number) or True,
        failing=failing,
        own=own,
        reasons=reasons,
        why=lambda names: "; ".join(panics[name.split("::", 1)[1]] for name in names),
    )
    assert found == ["1", "2"] and own == {"1": ["unit llrm-x::a"], "2": ["unit llrm-x::b"]}, (found, own)
    assert reasons == {"1": "panic of a on #1", "2": "panic of b on #2"}, reasons


def test_an_alone_check_runs_the_lib_tests_of_the_crates_that_depend_on_the_one_the_pr_changes(tmp_path, monkeypatch):
    """#4927 changed llrm-transforms, passed alone (only llrm-transforms ran), and failed four llrm-qb tests in the batch: a refusal at 10:24 and a clean precheck for the same PR."""
    repo, head = alone_scratch(tmp_path)
    smoke = ps.parity_smoke_module()
    ran = []
    monkeypatch.setattr(smoke, "workspace_graph", lambda root: ({}, {"llrm-qb": {"llrm-transforms"}, "llrm-x86": {"llrm-transforms"}, "llrm-transforms": set(), "llrm-nib": set()}))
    monkeypatch.setattr(smoke, "touched_packages", lambda files, packages, root: {"llrm-transforms"})
    monkeypatch.setattr(smoke, "dependants_lib_failures", lambda packages, **_: ran.append(set(packages)) or set())
    ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), scanners=lambda f: set())
    assert ran and {"llrm-transforms", "llrm-qb"} <= ran[0] and not {"llrm-nib", "llrm-x86"} & ran[0], ran  # qb depends on the change; nib does not; x86 does, but only qb, c and nib are probed


def test_the_last_alone_slot_is_for_urgent_prs_only(tmp_path, monkeypatch):
    import pytest  # noqa: PLC0415

    """Every alone slot was held by checks of ordinary PRs (and by orphaned warm-ups) while landing fixes waited; the last slot is reserved for the PRs of urgent.txt."""
    root = tmp_path / "slots"
    held = [ps._claim_slot(root, 3) for _ in range(2)]
    assert sorted(slot for slot, _ in held) == [0, 1]

    def sleep(_):
        raise TimeoutError("an ordinary PR waits for a slot")

    monkeypatch.setattr(ps.time, "sleep", sleep)
    with pytest.raises(TimeoutError):
        ps._claim_slot(root, 3)
    slot, lock = ps._claim_slot(root, 3, urgent=True)
    assert slot == 2, "the reserved slot goes to an urgent PR"
    lock.close()
    (tmp_path / "urgent.txt").write_text("# fixes\n5073 5125 # docs\n5123\n")
    assert ps.urgent_numbers({"LAND_TMP": str(tmp_path)}) == {"5073", "5125", "5123"}
    assert ps.urgent_numbers({"LAND_TMP": str(tmp_path / "none")}) == set()


def test_a_pr_that_does_not_link_alone_is_refused_and_only_a_runtime_or_frontend_pr_pays_for_the_check(tmp_path):
    """#4822 and #4839 (QB frontend) broke the link of the QB runtime programs and were refused by 19 batch rounds in three batches: the alone layer never linked anything."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "ok", "b.txt", "b\n")
    sh(repo, "checkout", "-q", "main")
    kw = dict(base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set(), scanners=lambda _: set(), units=lambda _: set(), docs=lambda root: set())
    why = ps.alone_refusals(head, links=lambda files, cwd, target: ["qb arena_type: llrm-c: _line@3", "qb bc_draw: x"], **kw)
    assert why == ["programs do not link: link qb arena_type, link qb bc_draw"], why
    assert ps.alone_refusals(head, links=lambda files, cwd, target: [], **kw) == []

    def never(*args, **kwargs):
        raise AssertionError("a PR outside the runtime and the frontends built and linked")

    assert ps.alone_link_failures(["crates/opt/llrm-transforms/src/x.rs", "docs/a.md"], tmp_path, None, run=never) == []
    ran = []

    def run(command, **kwargs):
        ran.append(command[-1] if "link_check" in command[-2:][0] or "link_check" in command[-1] else command[0])
        (tmp_path / "link" / "release").mkdir(parents=True, exist_ok=True)
        return type("Done", (), {"returncode": 0, "stdout": "link: 2 QB + 1 C programs, 1 do not link\nqb arena_type: boom\n", "stderr": ""})()

    got = ps.alone_link_failures(["crates/frontends/llrm-qb/src/x.rs"], tmp_path, tmp_path, run=run)
    assert got == ["qb arena_type: boom"] and len(ran) == 2, (got, ran)


def test_a_pr_joined_only_by_the_tests_append_driver_waits_for_its_owner(tmp_path):
    """#5462, #5481 and #5591 appended a test to a file an earlier member appended to: the batch's driver merged both, GitHub's squash refused each as conflicting."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "tools").mkdir()
    (repo / "tools/merge_tests_append.py").write_text((Path(__file__).resolve().parent / "merge_tests_append.py").read_text())
    (repo / ".gitattributes").write_text("at_tests.rs merge=tests-append\n")
    (repo / "at_tests.rs").write_text("#[test]\nfn base() {}\n")
    (repo / "other.txt").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    base_text = (repo / "at_tests.rs").read_text()
    heads = {
        "1": branch(repo, "a", "at_tests.rs", base_text + "\n#[test]\nfn first() {}\n"),
        "2": branch(repo, "b", "at_tests.rs", base_text + "\n#[test]\nfn second() {}\n"),
        "3": branch(repo, "c", "other.txt", "y\n"),
    }
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(["1", "2", "3"], heads, base="main", cwd=repo)
    assert kept == ["1", "3"], (kept, waits)
    assert list(waits) == ["2"] and waits["2"].startswith("needs owner parity merge") and "at_tests.rs" in waits["2"], waits


def test_assemble_prints_one_progress_line_per_candidate(tmp_path, capsys):
    """The 23:03 assembly ran 13 minutes with no output: nobody could say how many of the 40 heads were merged."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "shared.txt").write_text("one\ntwo\nthree\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {
        "1": branch(repo, "a", "shared.txt", "one\nTWO\nthree\n"),
        "2": branch(repo, "b", "shared.txt", "one\n2\nthree\n"),
        "3": branch(repo, "c", "other.txt", "y\n"),
    }
    sh(repo, "checkout", "-q", "main")
    ps.assemble(["1", "2", "3"], heads, base="main", cwd=repo, limit=16)
    lines = [line for line in capsys.readouterr().err.splitlines() if line.startswith("assemble:")]
    assert [line.split(" (")[0] for line in lines] == ["assemble: #1 kept", "assemble: #2 waits", "assemble: #3 kept"], lines
    assert "(2/16 kept," in lines[-1], lines


def test_a_pr_that_adds_only_a_bench_dir_without_a_manifest_waits_and_the_batch_still_forms(tmp_path):
    """#6317 added only crates/target/llrm-z80/bench/*: the workspace glob crates/*/* failed "failed to load manifest for workspace member" for every batch holding it (LAND_FAST has no precheck
    and skips `alone`, so only assembly can drop it). It waits with its own REFUSED line; the others join."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "crates/target/llrm-z80").mkdir(parents=True)
    (repo / "crates/target/llrm-z80/Cargo.toml").write_text("[package]\nname = \"z\"\n")
    (repo / "other.txt").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    sh(repo, "checkout", "-q", "-b", "orphan", "main")
    (repo / "crates/target/llrm-z80x/bench").mkdir(parents=True)
    (repo / "crates/target/llrm-z80x/bench/a.bas").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "orphan")
    heads = {"1": sh(repo, "rev-parse", "HEAD"), "2": branch(repo, "ok", "other.txt", "y\n")}
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(["1", "2"], heads, base="main", cwd=repo)
    assert kept == ["2"]
    assert "REFUSED: crate-manifest" in waits["1"] and "llrm-z80x" in waits["1"]

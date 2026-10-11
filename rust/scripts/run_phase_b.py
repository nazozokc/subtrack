#!/usr/bin/env python3
"""Phase-B golden runner: reads 001-013 + errors 001-008 (in-scope list cases).

Reproduces the capture env (fake HOME, TZ=UTC, C.utf8, TERM, SUBSC_CLI_DB_DIR
under a temp root) and byte-compares against the committed goldens after the
manifest's normalizations ({WORK}/{FIXTURES}/{REPO}/{VERSION}/{DATETIME}/{DATE}/{STAMP}).
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

FIXTURES = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "fixtures"))
REPO = os.path.abspath(os.path.join(FIXTURES, "..", ".."))
RUST = os.path.dirname(os.path.dirname(__file__))
BIN = os.path.join(RUST, "target", "debug", "subtrack")
VERSION = "10.0.0"

IN_SCOPE = []
# Optional group filters: `run_phase_b.py legacy mutate` runs those groups fully.
WANT = set(sys.argv[1:])
with open(os.path.join(FIXTURES, "cases.jsonl")) as f:
    for line in f:
        e = json.loads(line)
        if WANT:
            if e["group"] in WANT:
                IN_SCOPE.append(e)
            continue
        if e["group"] == "reads" and 1 <= int(e["id"][:3]) <= 13:
            IN_SCOPE.append(e)
        if e["group"] == "errors":
            n = int(e["id"][:3])
            if 1 <= n <= 8:
                IN_SCOPE.append(e)
        if e["group"] == "help":
            IN_SCOPE.append(e)


def normalize(text, work):
    s = text
    s = s.replace(work, "{WORK}")
    s = s.replace(FIXTURES, "{FIXTURES}")
    s = s.replace(REPO, "{REPO}")
    s = re.sub(r"(?<![\d.])" + re.escape(VERSION) + r"(?![\d.])", "{VERSION}", s)
    s = re.sub(
        r"\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?",
        "{DATETIME}",
        s,
    )
    s = re.sub(r"\d{4}-\d{2}-\d{2}", "{DATE}", s)
    s = re.sub(r"\d{8}_\d{6}", "{STAMP}", s)
    return s


def _case_env(db_dir, home, cwd):
    return {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": home,
        "TZ": "UTC",
        "LC_ALL": "C.utf8",
        "LANG": "C.utf8",
        "TERM": "xterm-256color",
        "SUBSC_CLI_DB_DIR": db_dir,
        "SUBTRACK_TODAY": "2026-10-08",
    }


def _copy_inputs(cwd):
    for item in os.listdir(os.path.join(FIXTURES, "inputs")):
        src = os.path.join(FIXTURES, "inputs", item)
        if os.path.isdir(src):
            shutil.copytree(src, os.path.join(cwd, item))
        else:
            shutil.copy2(src, os.path.join(cwd, item))


def run_case(e, root, shared=None):
    # The seed goldens were captured by `buildPopulated` (capture.mjs), which
    # emits a `<<<step N>>>` separator before every step and runs all steps
    # against one shared db/home/cwd. `runCase` only emits the separator for
    # multi-step cases.
    sep = e["group"] == "seed" or len(e["steps"]) > 1
    if shared is not None and e["group"] == "seed":
        case_root, db_dir, home, cwd, env = shared
    else:
        case_root = os.path.join(root, "work", e["group"], e["id"])
        db_dir = os.path.join(case_root, "db")
        home = os.path.join(case_root, "home")
        cwd = os.path.join(case_root, "cwd")
        for d in (db_dir, home, cwd):
            os.makedirs(d, mode=0o700, exist_ok=True)
        populated = os.path.join(FIXTURES, "dbs", e["db"])
        if os.path.isdir(populated):
            for item in os.listdir(populated):
                src = os.path.join(populated, item)
                dst = os.path.join(db_dir, item)
                if os.path.isdir(src):
                    shutil.copytree(src, dst)
                else:
                    shutil.copy2(src, dst)
        _copy_inputs(cwd)
        env = _case_env(db_dir, home, cwd)

    outs = []
    for i, argv in enumerate(e["steps"]):
        r = subprocess.run(
            [BIN, *argv], cwd=cwd, env=env, capture_output=True, timeout=30
        )
        if sep:
            n = e.get("_seedIdx", i + 1)
            s = f"<<<step {n}>>>\n".encode()
            outs.append((r.returncode, s + r.stdout, s + r.stderr))
        else:
            outs.append((r.returncode, r.stdout, r.stderr))
    return case_root, outs


def _make_seed_shared(root):
    case_root = os.path.join(root, "work", "seed-shared")
    db_dir = os.path.join(case_root, "db")
    home = os.path.join(case_root, "home")
    cwd = os.path.join(case_root, "cwd")
    for d in (db_dir, home, cwd):
        os.makedirs(d, mode=0o700, exist_ok=True)
    _copy_inputs(cwd)
    return (case_root, db_dir, home, cwd, _case_env(db_dir, home, cwd))


def main():
    fails = 0
    with tempfile.TemporaryDirectory(prefix="subtrack-run-") as root:
        seed_shared = None
        seed_n = 0
        for e in IN_SCOPE:
            if e["group"] == "seed":
                seed_n += 1
                e["_seedIdx"] = seed_n
        if seed_n:
            seed_shared = _make_seed_shared(root)
        for e in IN_SCOPE:
            case_root, outs = run_case(e, root, seed_shared)
            g = os.path.join(FIXTURES, "goldens", e["group"], e["id"] + ".stdout")
            ge = os.path.join(FIXTURES, "goldens", e["group"], e["id"] + ".stderr")
            want_out = open(g, "rb").read().decode("utf8", "surrogateescape")
            want_err = open(ge, "rb").read().decode("utf8", "surrogateescape")
            # Binary stdout (e.g. XLSX) is captured raw and must not be
            # text-normalized — the substitution regexes would rewrite
            # version strings inside the payload and corrupt the comparison.
            if e.get("binary"):
                got_out = b"".join(o[1] for o in outs).decode("utf8", "surrogateescape")
            else:
                got_out = normalize(
                    b"".join(o[1] for o in outs).decode("utf8", "surrogateescape"),
                    case_root,
                )
            got_err = normalize(
                b"".join(o[2] for o in outs).decode("utf8", "surrogateescape"),
                case_root,
            )
            got_exit = [o[0] for o in outs]
            ok = got_out == want_out and got_err == want_err and got_exit == e["exits"]
            status = "OK " if ok else "FAIL"
            if not ok:
                fails += 1
            print(f"{status} {e['group']}/{e['id']} exits={got_exit} want={e['exits']}")
            if got_exit != e["exits"]:
                print(f"   exit mismatch")
            if got_out != want_out:
                import difflib

                d = list(
                    difflib.unified_diff(
                        want_out.splitlines(),
                        got_out.splitlines(),
                        "golden",
                        "mine",
                        lineterm="",
                        n=1,
                    )
                )
                print("   stdout diff:")
                for line in d[:14]:
                    print("   " + line)
            if got_err != want_err:
                print(f"   stderr diff (got {len(got_err)}B want {len(want_err)}B)")
                import difflib

                d = list(
                    difflib.unified_diff(
                        want_err.splitlines(),
                        got_err.splitlines(),
                        "golden",
                        "mine",
                        lineterm="",
                        n=1,
                    )
                )
                for line in d[:10]:
                    print("   " + line)
    print(
        f"\n{len(IN_SCOPE) - fails}/{len(IN_SCOPE)} passed"
        + ("" if fails == 0 else f", {fails} FAILED")
    )
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())

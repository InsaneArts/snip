#!/usr/bin/env python3
"""M5's soak test: Snip used for a long while doesn't keep growing.

In the e2e's nested Hyprland (this imports e2e.py's harness), round after
round: ten area captures with their cards, closed; a take of the test
pattern, kept; the screenshot editor and the video editor opened and
closed. snipd's memory, open files and threads are measured after two
rounds of warm-up and again at the end: none may keep growing.

    python3 tests/e2e/soak.py [--rounds N]

About four minutes: run it before a release, not in a loop (CLAUDE.md).
Thirty rounds, so a slow leak shows: the warm tier's spare buffers grew
snipd ~1.6 MB a round, and it crashed once in ~220 captures with cards up.
"""
import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import e2e  # noqa: E402
from e2e import case, finish, video_editor_window, wait_for  # noqa: E402

ROUNDS = 30


def stats(s):
    """snipd's resident memory (MB), open files and threads."""
    pid = s.proc.pid
    status = dict(l.split(":", 1) for l in open(f"/proc/{pid}/status") if ":" in l)
    rss = int(status["VmRSS"].split()[0]) // 1024
    return rss, len(list(Path(f"/proc/{pid}/fd").iterdir())), int(status["Threads"])


def one_round(s):
    # ten captures, no overlay, their cards closed
    for i in range(10):
        r = s.json("area", "--x", str(100 + i * 20), "--y", "100", "--w", "400", "--h", "300", "--monitor", e2e.OUT)
        assert r["ok"], r
    shot = s.json("list", "-n", "1")["result"][0]["id"]
    s.cli("close-all")
    # a take of the test pattern, kept
    p = s.bg("record", "area", "--x", "100", "--y", "80", "--w", "400", "--h", "300", "--monitor", e2e.OUT)
    assert wait_for(lambda: (e2e.record_state(s) or {}).get("state") == "recording", 5), e2e.record_state(s)
    time.sleep(1.0)
    s.json("record", "stop")
    r, code = finish(p, timeout=20)
    assert code == 0, r
    take = r["result"]["id"]
    s.cli("close-all")
    # the screenshot editor and the video editor, opened and closed
    s.debug("annotate", shot)
    time.sleep(1.5)
    s.cli("debug", "ui", "close-editors")
    s.debug("annotate", take)
    assert wait_for(lambda: video_editor_window(), 8), "the video editor opens"
    time.sleep(1.5)
    s.cli("debug", "ui", "close-editors")
    assert wait_for(lambda: not video_editor_window(), 5), "and closes"
    time.sleep(1.0)


@case
def soak(s, env):
    """Round after round of captures, takes and editors: snipd's memory, files and threads
    after them are what they were after the warm-up (within a margin)."""
    for _ in range(2):
        one_round(s)
    time.sleep(3.0)
    before = stats(s)
    t0 = time.time()
    for i in range(ROUNDS):
        one_round(s)
        print(f"      round {i + 1}/{ROUNDS}: {stats(s)} (MB, files, threads)", flush=True)
    time.sleep(3.0)
    after = stats(s)
    print(f"      {ROUNDS} rounds in {time.time() - t0:.0f} s: {before} → {after}")
    assert after[0] - before[0] <= 20, ("memory keeps growing (MB)", before, after)
    assert after[1] - before[1] <= 4, ("open files keep growing", before, after)
    assert after[2] - before[2] <= 4, ("threads keep growing", before, after)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--rounds", type=int, default=ROUNDS)
    a, rest = ap.parse_known_args()
    ROUNDS = a.rounds
    sys.argv = [sys.argv[0], "--only", "soak", *rest]
    sys.exit(e2e.main())

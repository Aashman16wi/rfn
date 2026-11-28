#!/usr/bin/env python3
"""End-to-end checks for rfn driven through a pseudo terminal.

Run with:  python3 tests/pty_suite.py
Every check prints PASS/FAIL and the script exits non-zero if anything failed.
"""

import os
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import termios
import fcntl
import time

RFN = os.environ.get("RFN_BIN", "/home/aashmanr/.local/bin/rfn")
ROOT = "/tmp/opencode/rfn-suite"
STATE = f"{ROOT}/state"
COLS, ROWS = 120, 40

try:
    import pyte
except ImportError:
    print("pyte is required: pip install pyte")
    sys.exit(2)

RESULTS = []


def record(name, ok, detail=""):
    RESULTS.append((name, ok, detail))
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  -- {detail}" if detail and not ok else ""))


class App:
    """A running rfn instance inside a pty."""

    def __init__(self, path=None, env=None):
        self.buffer = bytearray()
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.environ["TERM"] = "xterm-256color"
            os.environ["XDG_STATE_HOME"] = STATE
            os.environ["HOME"] = ROOT
            os.environ.pop("RFN_THEME", None)
            if env:
                os.environ.update(env)
            args = [RFN]
            if path:
                args.append(path)
            os.execv(RFN, args)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        self.drain(1.5)

    def drain(self, seconds):
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([self.fd], [], [], 0.05)
            if ready:
                try:
                    data = os.read(self.fd, 1 << 20)
                except OSError:
                    return
                if not data:
                    return
                self.buffer.extend(data)

    def drain_until_quiet(self, quiet=0.25, limit=3.0):
        """Read until rfn stops writing for a moment.

        A key can take a little longer to show up on the screen than the
        sleep after it, so the screen is only looked at once the app has
        gone quiet.
        """
        end = time.time() + limit
        last = time.time()
        while time.time() < end:
            ready, _, _ = select.select([self.fd], [], [], 0.05)
            if ready:
                try:
                    data = os.read(self.fd, 1 << 20)
                except OSError:
                    return
                if not data:
                    return
                self.buffer.extend(data)
                last = time.time()
            elif time.time() - last >= quiet:
                return

    def key(self, data, wait=0.25):
        os.write(self.fd, data)
        self.drain_until_quiet(quiet=wait)

    def screen(self):
        screen = pyte.Screen(COLS, ROWS)
        pyte.Stream(screen).feed(self.buffer.decode("utf-8", "replace"))
        return screen

    def text(self):
        return "\n".join(line.rstrip() for line in self.screen().display)

    def status(self):
        return self.screen().display[ROWS - 1].rstrip()

    def pane(self):
        """The middle column (current directory) as plain text."""
        return "\n".join(self.screen().display[1:ROWS - 2])

    def has(self, needle):
        return needle.lower() in self.text().lower()

    def alive(self):
        """True while the app is still running."""
        try:
            pid, _ = os.waitpid(self.pid, os.WNOHANG)
            return pid == 0
        except ChildProcessError:
            return False

    def close(self):
        try:
            os.write(self.fd, b"q")
            time.sleep(0.2)
        except OSError:
            pass
        try:
            os.kill(self.pid, signal.SIGKILL)
            os.waitpid(self.pid, 0)
        except (ProcessLookupError, ChildProcessError):
            pass


def tree(name, files=(), dirs=()):
    """Create a clean test tree and return its path."""
    base = os.path.join(ROOT, name)
    shutil.rmtree(base, ignore_errors=True)
    os.makedirs(base, exist_ok=True)
    for directory in dirs:
        os.makedirs(os.path.join(base, directory), exist_ok=True)
    for filename, content in files:
        path = os.path.join(base, filename)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w") as handle:
            handle.write(content)
    return base


def main():
    shutil.rmtree(ROOT, ignore_errors=True)
    os.makedirs(ROOT, exist_ok=True)
    os.makedirs(STATE, exist_ok=True)

    # ---------------------------------------------------------------- CLI
    result = subprocess.run([RFN, "--generate-config"], capture_output=True)
    record("--generate-config exits 0", result.returncode == 0, result.stderr.decode())
    record("--generate-config has ui section", b"[ui]" in result.stdout)

    result = subprocess.run([RFN, "--themes"], capture_output=True)
    themes_out = result.stdout.decode()
    record("--themes exits 0", result.returncode == 0)
    record("--themes lists all four themes", all(
        name in themes_out for name in ["tokyo-night", "catppuccin-mocha", "gruvbox-dark", "nord"]
    ))
    record("--themes marks the active theme", "(active)" in themes_out)

    result = subprocess.run([RFN, "--theme", "definitely-not-a-theme"], capture_output=True)
    record("--theme rejects unknown name", result.returncode == 2, f"rc={result.returncode}")

    result = subprocess.run(f"{RFN} --themes | head -3", shell=True, capture_output=True)
    record("--themes survives a closed pipe", result.returncode in (0, 141), f"rc={result.returncode}")

    # ---------------------------------------------------------- navigation
    # Layout: work/ contains src/ and dst/. Directories sort first, so the
    # cursor starts on "dst/" and the order is deterministic.
    work = tree("work", files=[("src/one.txt", "1"), ("src/two.txt", "2"), ("dst/keep.txt", "k")],
                dirs=["src", "dst"])
    app = App(f"{work}/src")
    record("starts in the given directory", app.has("one.txt"))
    record("shows both files", app.has("two.txt"))
    record("cursor starts on the first file", "one.txt" in app.status(), app.status())

    app.key(b"j")
    record("j moves the cursor", "two.txt" in app.status(), app.status())
    app.key(b"k")
    record("k moves back", "one.txt" in app.status(), app.status())

    app.key(b"h")
    time.sleep(0.6)
    record("h goes to the parent", app.has("dst") and app.has("src"), app.pane())
    record("cursor is on the first entry after navigation", "dst" in app.status(), app.status())

    app.key(b"l")
    time.sleep(0.6)
    record("l enters a directory", app.has("keep.txt"), app.pane())
    record("cursor resets to the first entry", "keep.txt" in app.status(), app.status())
    app.close()

    # ------------------------------------------------------------- search
    base = tree("search", files=[("alpha.txt", "a"), ("beta.txt", "b"), ("gamma.log", "g")])
    app = App(base)
    app.key(b"/")
    app.key(b"alpha")
    time.sleep(0.4)
    record("search filters the list", app.has("alpha.txt") and not app.has("beta.txt"))
    app.key(b"\x1b")
    time.sleep(0.4)
    record("escape clears the search", app.has("beta.txt"))
    app.close()

    # --------------------------------------------------- visual mode + yank
    base = tree("visual", files=[("v1.txt", "1"), ("v2.txt", "2"), ("v3.txt", "3")])
    app = App(base)
    app.key(b"v")
    app.key(b"j")
    app.key(b"j")
    app.key(b"y")
    time.sleep(0.4)
    record("visual mode + y reports the count", app.has("yanked 3"), app.status())
    app.key(b"\x1b")
    time.sleep(0.3)
    record("escape leaves visual mode", "NORMAL" in app.status(), app.status())
    app.close()

    # --------------------------------------------------- copy/paste/queue
    base = tree("copy", files=[("src/one.txt", "one"), ("dst/keep.txt", "keep")],
                dirs=["src", "dst"])
    app = App(f"{base}/src")
    app.key(b"y")
    time.sleep(0.3)
    record("yank reports the count", app.has("yanked 1"), app.status())
    app.key(b"h")
    time.sleep(0.6)
    app.key(b"l")           # dst sorts before src, so this enters dst
    time.sleep(0.7)
    record("entered the destination folder", "keep.txt" in app.pane(), app.pane())
    app.key(b"p")
    time.sleep(2.0)
    record("paste copies the file", os.path.exists(f"{base}/dst/one.txt"),
           f"dst={os.listdir(base + '/dst')}")
    record("paste leaves the source alone", os.path.exists(f"{base}/src/one.txt"))
    app.close()

    # ------------------------------------------------------------ the queue
    base = tree("queue", files=[("src/one.txt", "one"), ("dst/keep.txt", "keep")],
                dirs=["src", "dst"])
    app = App(f"{base}/src")
    app.key(b"y")
    app.key(b"h")
    time.sleep(0.6)
    app.key(b"l")
    time.sleep(0.7)
    app.key(b"p")
    time.sleep(1.5)
    app.key(b"Q")
    time.sleep(0.6)
    record("Q opens the queue manager", app.has("queue"), app.text()[-400:])
    record("queue shows the task", app.has("one.txt") or app.has("DONE") or app.has("PEND"),
           app.text()[-400:])
    app.key(b"\x1b")
    time.sleep(0.3)
    app.close()

    # -------------------------------------------------------------- move
    base = tree("move", files=[("src/one.txt", "one"), ("dst/keep.txt", "keep")],
                dirs=["src", "dst"])
    app = App(f"{base}/src")
    app.key(b"x")
    time.sleep(0.3)
    record("cut reports the count", app.has("cut 1"), app.status())
    app.key(b"h")
    time.sleep(0.6)
    app.key(b"l")
    time.sleep(0.7)
    app.key(b"p")
    time.sleep(2.0)
    record("cut + paste moves the file", os.path.exists(f"{base}/dst/one.txt")
           and not os.path.exists(f"{base}/src/one.txt"),
           f"src={os.listdir(base + '/src')} dst={os.listdir(base + '/dst')}")
    app.close()

    # ------------------------------------------------------ overwrite guard
    base = tree("overwrite", files=[("src/one.txt", "new"), ("dst/one.txt", "old")],
                dirs=["src", "dst"])
    app = App(f"{base}/src")
    app.key(b"y")
    app.key(b"h")
    time.sleep(0.6)
    app.key(b"l")
    time.sleep(0.7)
    app.key(b"p")
    time.sleep(0.5)
    record("overwrite asks for confirmation", app.has("overwrite"), app.text()[-300:])
    app.key(b"y")
    time.sleep(1.5)
    with open(f"{base}/dst/one.txt") as handle:
        content = handle.read()
    record("overwrite replaces the file", content == "new", content)
    app.close()

    # ----------------------------------------------------------- renaming
    base = tree("rename", files=[("before.txt", "x")])
    app = App(base)
    app.key(b"r")
    time.sleep(0.3)
    app.key(b"\x7f" * 10)
    app.key(b"after.txt")
    app.key(b"\r")
    time.sleep(0.8)
    record("rename works", os.path.exists(f"{base}/after.txt"), str(os.listdir(base)))
    app.close()

    # ------------------------------------------------------ new file/dir
    base = tree("create")
    app = App(base)
    app.key(b"n")
    time.sleep(0.3)
    app.key(b"made.txt")
    app.key(b"\r")
    time.sleep(0.6)
    record("new file works", os.path.exists(f"{base}/made.txt"))
    app.key(b"N")
    time.sleep(0.3)
    app.key(b"madedir")
    app.key(b"\r")
    time.sleep(0.6)
    record("new directory works", os.path.isdir(f"{base}/madedir"))
    app.close()

    # -------------------------------------------------------------- tabs
    base = tree("tabs", files=[("src/t.txt", "t")], dirs=["src"])
    app = App(f"{base}/src")
    app.key(b"t")
    time.sleep(0.5)
    app.key(b"w")
    time.sleep(0.5)
    record("a second tab can be closed", "cannot close the last tab" not in app.status().lower(),
           app.status())
    app.close()

    # ------------------------------------------------------- theme picker
    base = tree("theme")
    app = App(base)
    app.key(b"\x14")  # ctrl+t
    time.sleep(0.6)
    record("ctrl+t opens the theme picker", app.has("j/k select"), app.text()[-300:])
    app.key(b"\x1b")
    time.sleep(0.4)
    record("escape closes the theme picker", not app.has("j/k select"))
    app.close()

    # ---------------------------------------------------- bookmark picker
    base = tree("marks", dirs=["target"])
    app = App(base)
    app.key(b"m")
    time.sleep(0.3)
    app.key(b"M")
    time.sleep(0.6)
    record("M opens the bookmark picker", app.has("bookmarks"), app.text()[-300:])
    app.key(b"\x1b")
    time.sleep(0.4)
    record("escape closes the bookmark picker", not app.has("j/k move"))
    app.close()

    # ------------------------------------------------------------ sorting
    base = tree("sort", files=[("a.txt", "1"), ("b.txt", "22"), ("c.txt", "333")])
    app = App(base)
    app.key(b"s")
    time.sleep(0.5)
    record("sorting cycles without crashing", app.has("a.txt"))
    app.key(b"S")
    time.sleep(0.4)
    record("reverse sort without crashing", app.has("a.txt"))
    app.close()

    # ------------------------------------------------------- hidden files
    base = tree("hidden", files=[(".secret", "s"), ("visible.txt", "v")])
    app = App(base)
    record("hidden files are hidden by default", not app.has(".secret"))
    app.key(b".")
    time.sleep(0.6)
    record("dot toggles hidden files", app.has(".secret"))
    app.close()

    # ------------------------------------------------------------- trash
    base = tree("trash", files=[("gone.txt", "bye")])
    app = App(base)
    app.key(b"d")
    time.sleep(0.4)
    record("delete asks for confirmation", app.has("delete"), app.text()[-300:])
    app.key(b"y")
    time.sleep(1.2)
    record("delete removes the file", not os.path.exists(f"{base}/gone.txt"),
           str(os.listdir(base)))
    app.close()

    # ------------------------------------------------------ git status
    repo = tree("repo", files=[("tracked.txt", "one")])
    subprocess.run(["git", "init", "-q"], cwd=repo)
    subprocess.run(["git", "config", "user.email", "t@t.t"], cwd=repo)
    subprocess.run(["git", "config", "user.name", "t"], cwd=repo)
    subprocess.run(["git", "add", "tracked.txt"], cwd=repo)
    subprocess.run(["git", "commit", "-qm", "init"], cwd=repo)
    with open(f"{repo}/tracked.txt", "w") as handle:
        handle.write("changed")
    app = App(repo)
    time.sleep(1.0)
    record("git marks a changed file", "M" in app.status(), app.status())
    app.close()

    # -------------------------------------------------------- big folder
    big = tree("big")
    for index in range(2000):
        with open(f"{big}/file{index:05}.txt", "w") as handle:
            handle.write("x")
    start = time.time()
    app = App(big)
    elapsed = time.time() - start
    record("large folder opens quickly", elapsed < 5.0, f"{elapsed:.2f}s")
    record("large folder lists files", app.has("file00001.txt"))
    app.key(b"j" * 50)
    time.sleep(0.8)
    record("cursor moves in a large folder", app.has("file"))
    record("status bar follows the cursor", "file" in app.status(), app.status())
    app.close()

    # ------------------------------------------------------ session restore
    base = tree("session", dirs=["one", "two"])
    app = App(base)
    app.key(b"h")
    time.sleep(0.6)
    app.key(b"j")
    app.key(b"l")
    time.sleep(0.8)
    app.close()

    app = App(None)
    time.sleep(1.0)
    record("session restores the last directory", app.has("session"), app.text().splitlines()[0])
    app.close()

    # ------------------------------------------------------ trash browser
    base = tree("trash2", files=[("doomed.txt", "bye")])
    app = App(base)
    app.key(b"d")
    time.sleep(0.4)
    app.key(b"y")
    time.sleep(1.5)
    app.key(b"T")
    time.sleep(1.2)
    record("T opens the trash browser", app.has("trash"), app.text()[-400:])
    record("the deleted file is listed in the trash", app.has("doomed"), app.text()[-400:])
    record("the trash shows the original path", app.has("trash2"), app.text()[-400:])
    app.key(b"r")
    time.sleep(1.5)
    record("restore brings the file back", os.path.exists(f"{base}/doomed.txt"),
           str(os.listdir(base)))
    app.key(b"T")
    time.sleep(1.0)
    record("the trash is empty again", not app.has("doomed"), app.text()[-400:])
    app.key(b"\x1b")
    app.close()

    # ------------------------------------------------------ trash purging
    base = tree("trash3", files=[("doomed.txt", "bye")])
    app = App(base)
    app.key(b"d")
    time.sleep(0.4)
    app.key(b"y")
    time.sleep(1.5)
    app.key(b"T")
    time.sleep(1.2)
    app.key(b"d")
    time.sleep(0.5)
    record("permanent delete asks for confirmation", app.has("forever"), app.text()[-400:])
    app.key(b"y")
    time.sleep(1.5)
    record("the trash item is gone", not app.has("doomed"), app.text()[-400:])
    app.key(b"\x1b")
    app.close()

    # ----------------------------------------------------------- open with
    base = tree("openwith", files=[("target.txt", "x")])
    marker = f"{base}/opened.marker"
    app = App(base)
    app.key(b"o")
    time.sleep(0.5)
    record("o opens the open-with prompt", app.has("open") or app.has("command"),
           app.text()[-300:])
    app.key(f"touch {marker}\r".encode())
    time.sleep(1.5)
    record("open with runs the command", os.path.exists(marker))
    app.close()

    # --------------------------------------------------------- bulk rename
    base = tree("bulk", files=[("one.txt", "1"), ("two.txt", "2")])
    editor = f"{ROOT}/editor.sh"
    with open(editor, "w") as handle:
        handle.write("#!/bin/sh\nsed -i 's/\\.txt$/.renamed/' \"$1\"\n")
    os.chmod(editor, 0o755)
    app = App(base, env={"EDITOR": editor})
    app.key(b"v")
    app.key(b"j")
    app.key(b"b")
    time.sleep(2.0)
    record("bulk rename renames the marked files",
           os.path.exists(f"{base}/one.renamed") and os.path.exists(f"{base}/two.renamed"),
           str(os.listdir(base)))
    app.close()

    # ------------------------------------------------------ preview scroll
    base = tree("scroll")
    with open(f"{base}/long.txt", "w") as handle:
        handle.write("".join(f"line {index}\n" for index in range(500)))
    app = App(base)
    app.key(b"J")
    app.key(b"J")
    time.sleep(0.8)
    record("preview scroll works", app.alive())
    record("preview shows scrolled content", "line" in app.text(), app.text()[-300:])
    app.key(b"0")
    time.sleep(0.5)
    record("preview reset works", app.alive())
    app.close()

    # -------------------------------------------------------- image preview
    base = tree("image")
    png = base + "/pic.png"
    try:
        subprocess.run(
            ["python3", "-c", f"""
import base64, pathlib
data = base64.b64decode(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='
)
pathlib.Path({png!r}).write_bytes(data)
"""], check=True)
    except Exception:
        png = None
    if png:
        app = App(base)
        time.sleep(1.5)
        record("survives an image preview", app.alive())
        record("shows the image file", app.has("pic.png"))
        app.key(b"=")
        time.sleep(0.8)
        record("zoom in does not crash", app.alive())
        app.key(b"0")
        time.sleep(0.5)
        record("zoom reset does not crash", app.alive())
        app.close()

    print("\n" + "=" * 60)
    failed = [name for name, ok, _ in RESULTS if not ok]
    print(f"{len(RESULTS) - len(failed)}/{len(RESULTS)} checks passed")
    if failed:
        print("\nFAILED:")
        for name in failed:
            print(f"  - {name}")
    print("=" * 60)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

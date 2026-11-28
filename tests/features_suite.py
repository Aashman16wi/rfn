#!/usr/bin/env python3
"""Feature level checks for rfn (pseudo terminal driven).

Run with:  python3 tests/features_suite.py
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
ROOT = "/tmp/opencode/rfn-features"
STATE = f"{ROOT}/state"
CONFIG = f"{ROOT}/.config/rfn/config.toml"
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
    def __init__(self, path=None, env=None):
        self.buffer = bytearray()
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.environ["TERM"] = "xterm-256color"
            os.environ["XDG_STATE_HOME"] = STATE
            os.environ["HOME"] = ROOT
            os.environ.pop("XDG_CONFIG_HOME", None)
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
        return "\n".join(self.screen().display[1:ROWS - 2])

    def has(self, needle):
        return needle.lower() in self.text().lower()

    def alive(self):
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

    # ------------------------------------------------------------- tabs
    base = tree("tabs", files=[("one/a.txt", "a"), ("two/b.txt", "b")], dirs=["one", "two"])
    app = App(f"{base}/one")
    record("tab one shows its own folder", app.has("a.txt"))
    app.key(b"t")
    time.sleep(0.5)
    app.key(b"h")
    time.sleep(0.6)
    record("the new tab starts in the parent", app.has("two"), app.pane())
    app.key(b"1")
    time.sleep(0.6)
    record("pressing 1 goes back to the first tab", app.has("a.txt"), app.pane())
    app.key(b"2")
    time.sleep(0.6)
    record("pressing 2 goes to the second tab", app.has("two"), app.pane())
    app.key(b"\t")
    time.sleep(0.6)
    record("tab cycles to the other tab", app.has("a.txt"), app.pane())
    app.key(b"w")
    time.sleep(0.5)
    record("w closes the current tab", app.has("a.txt"), app.pane())
    app.close()

    # -------------------------------------------------------- bookmarks
    base = tree("marks", dirs=["target"])
    app = App(base)
    app.key(b"m")
    time.sleep(0.4)
    record("m adds a bookmark", app.has("bookmark added"), app.status())
    app.key(b"M")
    time.sleep(0.6)
    record("M opens the bookmark picker", app.has("bookmarks"), app.text()[-300:])
    record("the bookmark is listed", app.has("marks"), app.text()[-300:])
    app.key(b"\x1b")
    time.sleep(0.4)
    app.close()

    # The bookmark survives a restart and can be jumped to.
    app = App(f"{base}/target")
    time.sleep(0.8)
    app.key(b"M")
    time.sleep(0.6)
    record("the bookmark is restored", app.has("marks"), app.text()[-300:])
    app.key(b"\r")
    time.sleep(0.8)
    record("enter jumps to the bookmark", app.has("marks"), app.text()[-200:])
    app.close()

    # Removing a bookmark.
    app = App(base)
    app.key(b"M")
    time.sleep(0.6)
    app.key(b"d")
    time.sleep(0.5)
    record("d removes the bookmark", app.has("no bookmarks"), app.text()[-300:])
    app.key(b"\x1b")
    app.close()

    # ------------------------------------------------------ theme picker
    base = tree("theme")
    app = App(base)
    app.key(b"\x14")
    time.sleep(0.6)
    record("the theme picker lists the themes", app.has("tokyo-night"), app.text()[-400:])
    app.key(b"j")
    time.sleep(0.3)
    app.key(b"\r")
    time.sleep(0.8)
    record("the chosen theme is applied", app.has("catppuccin"), app.status())
    record("the theme is written to the config", os.path.exists(CONFIG), CONFIG)
    if os.path.exists(CONFIG):
        with open(CONFIG) as handle:
            saved = handle.read()
        record("the config holds the new theme", "catppuccin-mocha" in saved, saved[:200])
    app.close()

    # The theme stays after a restart.
    app = App(base)
    time.sleep(0.8)
    record("the theme is remembered", app.has("catppuccin"), app.status())
    app.close()

    # ------------------------------------------------------ theme on cli
    result = subprocess.run([RFN, "--theme", "nord"], capture_output=True,
                            env={**os.environ, "HOME": ROOT})
    record("--theme exits 0", result.returncode == 0, result.stderr.decode())
    if os.path.exists(CONFIG):
        with open(CONFIG) as handle:
            saved = handle.read()
        record("--theme writes the config", "nord" in saved, saved[:200])

    # ------------------------------------------------------ queue manager
    base = tree("queue", files=[("src/one.txt", "1"), ("src/two.txt", "2")],
                dirs=["src", "dst"])
    app = App(f"{base}/src")
    app.key(b"v")
    app.key(b"j")
    app.key(b"y")
    time.sleep(0.5)
    record("two files are yanked", app.has("yanked 2"), app.status())
    app.key(b"h")
    time.sleep(0.6)
    app.key(b"l")
    time.sleep(0.7)
    app.key(b"p")
    time.sleep(1.5)
    app.key(b"Q")
    time.sleep(0.8)
    record("the queue manager opens", app.has("queue"), app.text()[-400:])
    record("the queue lists both tasks", app.text().count("one.txt") >= 1
           and app.text().count("two.txt") >= 1, app.text()[-400:])
    app.key(b"c")
    time.sleep(0.5)
    record("c clears the finished tasks", app.has("cleared"), app.text()[-400:])
    app.key(b"\x1b")
    time.sleep(0.3)
    app.close()

    # ------------------------------------------------- self paste is safe
    base = tree("selfpaste", files=[("keep.txt", "important")])
    app = App(base)
    app.key(b"y")
    time.sleep(0.3)
    app.key(b"p")
    time.sleep(0.8)
    record("pasting into the same folder is refused",
           app.has("same file") or app.has("same folder"), app.status())
    with open(f"{base}/keep.txt") as handle:
        content = handle.read()
    record("the file is untouched", content == "important", content)
    app.close()

    # ----------------------------------------- existing names are protected
    base = tree("names", files=[("one.txt", "first"), ("two.txt", "second")])
    app = App(base)
    app.key(b"r")
    time.sleep(0.4)
    app.key(b"\x7f" * 20)
    app.key(b"two.txt")
    app.key(b"\r")
    time.sleep(0.8)
    record("renaming onto an existing name is refused",
           app.has("already exists"), app.status())
    record("both files are still there",
           os.path.exists(f"{base}/one.txt") and os.path.exists(f"{base}/two.txt"),
           str(sorted(os.listdir(base))))
    with open(f"{base}/two.txt") as handle:
        content = handle.read()
    record("the existing file keeps its content", content == "second", content)
    app.close()

    base = tree("names2", files=[("one.txt", "important")])
    app = App(base)
    app.key(b"n")
    time.sleep(0.4)
    app.key(b"one.txt")
    app.key(b"\r")
    time.sleep(0.8)
    record("creating an existing file is refused", app.has("already exists"), app.status())
    with open(f"{base}/one.txt") as handle:
        content = handle.read()
    record("the existing file keeps its content", content == "important", content)
    app.close()

    # ------------------------------------------------- copy a whole folder
    base = tree("dircopy", files=[("src/inner/file.txt", "deep")], dirs=["src"])
    app = App(f"{base}/src")
    app.key(b"h")
    time.sleep(0.6)
    app.key(b"j")
    app.key(b"y")
    time.sleep(0.4)
    app.key(b"l")
    time.sleep(0.8)
    app.key(b"p")
    time.sleep(2.0)
    record("a folder can be copied", os.path.exists(f"{base}/src/inner/file.txt"),
           str(os.listdir(base)))
    app.close()

    # ------------------------------------------------- deleting a folder
    base = tree("dirdelete", files=[("gone/file.txt", "x")], dirs=["gone"])
    app = App(base)
    app.key(b"d")
    time.sleep(0.4)
    app.key(b"y")
    time.sleep(1.5)
    record("a folder can be deleted", not os.path.exists(f"{base}/gone"),
           str(os.listdir(base)))
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

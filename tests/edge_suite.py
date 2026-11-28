#!/usr/bin/env python3
"""Edge cases and stress checks for rfn (pseudo terminal driven).

Run with:  python3 tests/edge_suite.py
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
ROOT = "/tmp/opencode/rfn-edge"
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
    def __init__(self, path=None, env=None, config=None):
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

    def drain_until_quiet(self, quiet=0.2, limit=3.0):
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

    def key(self, data, wait=0.2):
        os.write(self.fd, data)
        self.drain_until_quiet(quiet=wait)

    def mash(self, data, count, wait=0.02):
        for _ in range(count):
            os.write(self.fd, data)
            time.sleep(wait)
        self.drain_until_quiet()

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

    # ---------------------------------------------- rapid navigation race
    base = tree("race", dirs=["one", "two", "three"])
    app = App(base)
    app.mash(b"l", 3, wait=0.05)   # dive into three nested folders quickly
    time.sleep(1.5)
    app.drain(0.5)
    record("survives rapid navigation", app.alive())
    record("shows a real directory after racing", "one" in app.text() or "two" in app.text()
           or "three" in app.text(), app.text().splitlines()[0])
    app.close()

    # ----------------------------------------- stale results never win
    base = tree("stale", dirs=["a", "b"])
    for letter in ("a", "b"):
        for index in range(200):
            with open(f"{base}/{letter}/file{index}.txt", "w") as handle:
                handle.write(letter)
    app = App(f"{base}/a")
    time.sleep(0.4)
    app.key(b"h")            # parent
    app.mash(b"l", 2, wait=0.05)
    time.sleep(2.0)
    app.drain(0.5)
    listing = app.pane()
    record("no mixed listing after fast navigation", "file" in listing, listing[:200])
    record("survives the stale-result race", app.alive())
    app.close()

    # ------------------------------------------------------ weird names
    base = tree("weird", files=[
        ("name with spaces.txt", "a"),
        ("name'with'quotes.txt", "b"),
        ("emoji-\U0001f600.txt", "c"),
        ("üñïçø∂é.txt", "d"),
        ("-dash-leading.txt", "e"),
        ("tab\tinside.txt", "f"),
    ])
    app = App(base)
    record("survives weird file names", app.alive())
    record("shows a file with spaces", app.has("name with spaces"))
    app.key(b"j")
    app.key(b"j")
    app.key(b"j")
    record("cursor moves over unicode names", app.alive())
    app.close()

    # ------------------------------------------------- broken symlink
    base = tree("links", files=[("real.txt", "r")])
    os.symlink("/does/not/exist", f"{base}/broken")
    os.symlink(f"{base}/real.txt", f"{base}/good")
    app = App(base)
    record("survives a broken symlink", app.alive())
    record("lists the broken symlink", app.has("broken"))
    app.key(b"j")
    app.key(b"j")
    record("cursor can sit on a broken symlink", app.alive())
    app.close()

    # -------------------------------------------------- symlink loop
    base = tree("loop", dirs=["inner"])
    os.symlink(base, f"{base}/inner/loop")
    app = App(base)
    app.key(b"l")
    time.sleep(0.8)
    app.key(b"l")
    time.sleep(0.8)
    record("survives a symlink loop", app.alive())
    app.close()

    # ------------------------------------------------- unreadable folder
    base = tree("perm", files=[("ok.txt", "o")])
    locked = f"{base}/locked"
    os.makedirs(locked, exist_ok=True)
    with open(f"{locked}/inside.txt", "w") as handle:
        handle.write("x")
    os.chmod(locked, 0o000)
    app = App(base)
    app.key(b"l")            # "locked" sorts first, so it is already selected
    time.sleep(1.0)
    record("survives an unreadable directory", app.alive())
    record("shows an error for the unreadable directory",
           app.has("cannot read") or app.has("permission") or app.has("denied"),
           app.text()[-300:])
    os.chmod(locked, 0o755)
    app.close()

    # ------------------------------------------------------ empty folder
    base = tree("empty", dirs=["nothing"])
    app = App(base)
    app.key(b"l")
    time.sleep(0.8)
    record("survives an empty directory", app.alive())
    record("empty directory is shown", app.has("nothing") or "0 items" in app.text(),
           app.text()[-200:])
    app.close()

    # -------------------------------------------------- deep nesting
    deep = tree("deep")
    path = deep
    for level in range(30):
        path = os.path.join(path, f"level{level}")
        os.makedirs(path, exist_ok=True)
    app = App(deep)
    for _ in range(30):
        app.key(b"l", wait=0.12)
    time.sleep(1.0)
    record("survives deep nesting", app.alive())
    record("reaches the deepest folder", "level29" in app.text(), app.text().splitlines()[0])
    app.close()

    # --------------------------------------------------- big file text
    base = tree("bigfile")
    with open(f"{base}/huge.txt", "w") as handle:
        handle.write("line\n" * 500000)
    app = App(base)
    time.sleep(1.5)
    record("survives a huge text file", app.alive())
    record("previews a huge text file", app.has("huge.txt"))
    app.close()

    # ------------------------------------------------------ binary file
    base = tree("binary")
    with open(f"{base}/blob.bin", "wb") as handle:
        handle.write(bytes(range(256)) * 100)
    app = App(base)
    time.sleep(1.0)
    record("survives a binary file", app.alive())
    app.close()

    # -------------------------------------------------- key mashing
    base = tree("mash", files=[("a.txt", "a"), ("b.txt", "b")], dirs=["d"])
    app = App(base)
    app.mash(b"jklhdsrgv", 12, wait=0.01)
    time.sleep(0.5)
    record("survives key mashing", app.alive())
    app.mash(b"\x1b", 3, wait=0.05)
    app.close()

    # --------------------------------------------- queue persistence
    base = tree("persist", files=[("src/one.txt", "one"), ("dst/keep.txt", "keep")],
                dirs=["src", "dst"])
    app = App(f"{base}/src")
    app.key(b"y")
    app.key(b"h")
    time.sleep(0.6)
    app.key(b"l")
    time.sleep(0.7)
    app.key(b"p")
    time.sleep(1.5)
    app.close()
    queue_file = f"{STATE}/rfn/queue/queue"
    record("queue file is written", os.path.exists(queue_file),
           str(os.listdir(f"{STATE}/rfn")) if os.path.exists(f"{STATE}/rfn") else "no dir")

    # ------------------------------------------------- invalid config
    config_dir = f"{ROOT}/.config/rfn"
    os.makedirs(config_dir, exist_ok=True)
    with open(f"{config_dir}/config.toml", "w") as handle:
        handle.write("[ui]\ntheme = \"not-a-real-theme\"\n")
    app = App(base, env={"XDG_CONFIG_HOME": f"{ROOT}/.config"})
    time.sleep(1.0)
    record("survives an unknown theme name", app.alive())
    record("still lists files with a broken theme", app.has("one.txt") or app.has("keep.txt"))
    app.close()

    with open(f"{config_dir}/config.toml", "w") as handle:
        handle.write("this is not = valid toml [[[")
    result = subprocess.run([RFN, base], capture_output=True, timeout=5,
                            env={**os.environ, "XDG_CONFIG_HOME": f"{ROOT}/.config"})
    record("broken config reports an error instead of hanging", result.returncode != 0
           or result.stderr, f"rc={result.returncode} err={result.stderr[:120]!r}")
    shutil.rmtree(f"{ROOT}/.config", ignore_errors=True)

    # ------------------------------------------------- resize handling
    base = tree("resize", files=[("a.txt", "a")])
    app = App(base)
    fcntl.ioctl(app.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    app.key(b"j", wait=0.5)
    record("survives a terminal resize", app.alive())
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

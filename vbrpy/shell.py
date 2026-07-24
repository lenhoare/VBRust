"""`Shell` — VB6's `Shell`, on Python's `subprocess`. `Run` executes through the
system shell, waits, and captures output (`Ok(stdout)` / `Err(stderr)`); `Start`
launches without waiting and hands back a `Process` handle. Mirrors
`vbr_stdlib::Shell`: stdout/stderr are trimmed of trailing whitespace, and a
`wait()` on a signal-killed process reports `-1` (the code is unknowable)."""

import subprocess

from .prelude import Ok, Err


class Shell:
    @staticmethod
    def run(cmd):
        try:
            r = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        except Exception as e:  # noqa: BLE001 — any spawn failure is an Err
            return Err(str(e))
        if r.returncode == 0:
            return Ok(r.stdout.rstrip())
        err = r.stderr.rstrip()
        return Err(err if err else "'%s' failed with code %d" % (cmd, r.returncode))

    @staticmethod
    def start(cmd):
        try:
            return Ok(Process(subprocess.Popen(cmd, shell=True)))
        except Exception as e:  # noqa: BLE001
            return Err(str(e))


class Process:
    def __init__(self, popen):
        self._p = popen

    def kill(self):
        # Killing an already-dead process is a harmless no-op; wait() reaps it.
        try:
            self._p.kill()
            self._p.wait()
        except Exception:  # noqa: BLE001
            pass

    def isrunning(self):
        return self._p.poll() is None

    def wait(self):
        code = self._p.wait()
        # A negative code means killed by a signal — unknowable, report -1.
        return code if code is not None and code >= 0 else -1

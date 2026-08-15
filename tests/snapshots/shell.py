# Shell — VB6's `Shell`, grown up. Two verbs:
# Shell.Run(cmd)   — run through the system shell, WAIT, capture the output:
# stdout on success; failure propagates (or Handle it).
# Shell.Start(cmd) — launch and DON'T wait (VB6's actual Shell semantics):
# you get a Process handle to check on or stop.
# Pipes and PATH work — the command line goes through sh -c / cmd /C.
# A background child: start it, peek at it, stop it. `Kill` on an already-dead
# process is a harmless no-op; `Wait` returns the exit code (-1 after a kill).

import time
from vbrpy import Ok, Err, _vb, Process, Shell

def main():
    output: str = ""
    _t0 = Shell.run('echo hello from Bust')
    if isinstance(_t0, Err):
        why = _t0.error
        print(f"echo failed: {_vb(why)}")
        return
    else:
        output = _t0.value
    print(f"said: {_vb(output)}")
    _t1 = Shell.run('ls /vbr/definitely/missing')
    if isinstance(_t1, Err):
        why = _t1.error
        print('as expected, that failed')
    code: int = 0
    _t2 = runchild()
    if isinstance(_t2, Err):
        why = _t2.error
        print(f"child failed: {_vb(why)}")
        return
    else:
        code = _t2.value
    print(f"child finished with exit code {_vb(code)}")

def runchild() -> int:
    _t3 = Shell.start('sleep 2')
    if isinstance(_t3, Err):
        return _t3
    child: Process = _t3.value
    time.sleep(100 / 1000)
    # VB6's kernel32 Sleep, no Declare needed (milliseconds)
    print(f"running: {_vb(child.is_running())}")
    child.kill()
    print(f"after kill: {_vb(child.is_running())}")
    return Ok(child.wait())


if __name__ == "__main__":
    main()

# Shell — VB6's `Shell`, grown up. Two verbs:
# Shell.Run(cmd)   — run through the system shell, WAIT, capture the output:
# Ok(stdout) on success, Err(stderr) on a nonzero exit.
# Shell.Start(cmd) — launch and DON'T wait (VB6's actual Shell semantics):
# you get a Process handle to check on or stop.
# Pipes and PATH work — the command line goes through sh -c / cmd /C.
# A background child: start it, peek at it, stop it. `Kill` on an already-dead
# process is a harmless no-op; `Wait` returns the exit code (-1 after a kill).

import time
from vbrpy import Ok, Err, _vb, Process, Shell

def main():
    _m0 = Shell.run('echo hello from VBR')
    match _m0:
        case Ok(output):
            print(f"said: {_vb(output)}")
        case Err(why):
            print(f"echo failed: {_vb(why)}")
    _m1 = Shell.run('ls /vbr/definitely/missing')
    match _m1:
        case Ok(output):
            print(_vb(output))
        case Err(_):
            print('as expected, that failed')
    _m2 = runchild()
    match _m2:
        case Ok(code):
            print(f"child finished with exit code {_vb(code)}")
        case Err(why):
            print(f"child failed: {_vb(why)}")

def runchild() -> object:
    _t0 = Shell.start('sleep 2')
    if isinstance(_t0, Err):
        return _t0
    child: Process = _t0.value
    time.sleep(100 / 1000)
    # VB6's kernel32 Sleep, no Declare needed (milliseconds)
    print(f"running: {_vb(child.isrunning())}")
    child.kill()
    print(f"after kill: {_vb(child.isrunning())}")
    return Ok(child.wait())


if __name__ == "__main__":
    main()

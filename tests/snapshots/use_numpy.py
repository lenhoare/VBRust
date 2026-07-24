# `Use` an external pip library — the Python-target parallel of `Use <crate>`
# for Cargo. `Use numpy 2.5.0` emits `import numpy` at the top AND a pinned line
# in requirements.txt, so the generated project is pip-installable. numpy is
# then in scope for direct calls and inline `Python` blocks alike.
# 
# (This is a Python-target example: numpy is a pip package, so — like a Rust-only
# crate on the Rust target — it only builds under `vbr py`, not `vbr run`.)

import numpy

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    # Direct call: a `Use`d module is a namespace whose methods pass straight
    # through, keeping their exact Python casing (nothing is lowercased).
    mean: float = numpy.array([1, 2, 3, 4]).mean()
    print(f"mean is {_vb(mean)}")
    # Because `Use` already imported numpy at module scope, an inline `Python`
    # block reaches it WITHOUT re-importing — they share the module globals.
    total = float(numpy.array([10, 20, 30]).sum())
    print(f"total is {_vb(total)}")


if __name__ == "__main__":
    main()

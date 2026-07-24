# Inline Python handles — hold a Python object VBR has no type for, and pass it
# back into later blocks. Each block is its own GIL scope. (Slice 2.)

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    # Load once: a numpy array. No `As` type — it's held as an opaque PyObject.
    import numpy as np
    data = np.array([3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0])
    # Query it repeatedly — the handle goes back in via `Python(data)`.
    mean = float(data.mean())
    print(f"mean = {_vb(mean)}")
    biggest = float(data.max())
    print(f"max  = {_vb(biggest)}")
    # Pass a scalar in alongside the handle: how many exceed a threshold?
    threshold: float = 4.0
    above = int((data > threshold).sum())
    print(f"{_vb(above)} values exceed {_vb(threshold)}")


if __name__ == "__main__":
    main()

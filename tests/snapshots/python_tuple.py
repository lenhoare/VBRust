# Tuple return — pull SEVERAL values out of one Python block, in a single GIL
# scope. The natural shape for "a name AND its data" from a model, a dataframe,
# a query result… write the results as a comma-separated tuple on the last line,
# and destructure them into typed VBR bindings. (Slice 3.)

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    # One block, three results: a label (String), its data (Vec<Double>), and a
    # summary stat (Double) — extracted together without touching the object twice.
    import numpy as np
    w = np.array([0.5, 1.5, 2.0, 3.0])
    name, weights, total = "layer.weight", w.tolist(), float(w.sum())
    print(f"tensor: {_vb(name)}")
    print(f"sum:    {_vb(total)}")
    print(f"first:  {_vb(weights[0])}")
    # Works with a handle passed in too: destructure stats out of a held object.
    import numpy as np
    data = np.array([3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0])
    lo, hi, mean = float(data.min()), float(data.max()), float(data.mean())
    print(f"range:  {_vb(lo)} .. {_vb(hi)} (mean {_vb(mean)})")


if __name__ == "__main__":
    main()

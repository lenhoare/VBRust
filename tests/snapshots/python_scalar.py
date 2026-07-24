# Inline Python — drop into real CPython for a bit, get a plain value back.
# The block runs via pyo3; its last line is the value, extracted into the
# type you annotate with `As`. (Slice 1: scalars in, scalars out.)

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    # A one-liner reaching a library VBR doesn't have.
    import numpy as np
    mean = np.array([1, 2, 3, 4]).mean()
    print(f"mean is {_vb(mean)}")
    # A multi-line block — real Python, sealed inside; last line is the value.
    name = "world"
    greeting = f"hello, {name}"
    print(_vb(greeting))
    answer = 6 * 7
    print(f"the answer is {_vb(answer)}")


if __name__ == "__main__":
    main()

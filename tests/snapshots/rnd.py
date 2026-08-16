# Rnd() is a Double in [0, 1). A die is Int(Rnd() * 6) + 1.
# The printed line is a range check so the snapshot stays deterministic.

import random

def main():
    r: float = random.random()
    if (r >= 0.0) and (r < 1.0):
        print('ok')


if __name__ == "__main__":
    main()

# Constants — module level, SCREAMING_SNAKE_CASE

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

MAX_RETRIES: int = 3
GREETING: str = 'Hello'
VERSION: float = 1.5

def main():
    i: int = 0
    while i < MAX_RETRIES:
        print(f"{_vb(GREETING)}, attempt {_vb(i + 1)}")
        i = i + 1
    print(f"version {_vb(VERSION)}")


if __name__ == "__main__":
    main()

# Vec<T> — a growable list

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    nums: list[int] = []
    nums.append(10)
    nums.append(20)
    nums.append(30)
    print(f"count = {_vb(len(nums))}")
    total: int = 0
    for n in nums:
        total = total + n
    print(f"total = {_vb(total)}")


if __name__ == "__main__":
    main()

# Iterators — filter, map, sum, any, count, collect

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    nums: list[int] = []
    nums.append(1)
    nums.append(2)
    nums.append(3)
    nums.append(4)
    nums.append(5)
    big: list[int] = [x for x in nums if x > 2]
    doubled: list[int] = [x * 2 for x in nums]
    total: int = sum(nums)
    hasbig: bool = any(x > 4 for x in nums)
    print(f"count:   {_vb(len(nums))}")
    print(f"total:   {_vb(total)}")
    print(f"has big: {_vb(hasbig)}")
    for n in big:
        print(f"big:     {_vb(n)}")
    for n in doubled:
        print(f"doubled: {_vb(n)}")


if __name__ == "__main__":
    main()

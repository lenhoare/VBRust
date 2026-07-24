# More iterator links — take, skip, rev — and the Option-returning consumers
# (max, position) that pair with Match.

from dataclasses import dataclass

@dataclass
class Some:
    value: object

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    hi: int = 8
    nums: list[int] = []
    for i in range(1, hi + 1):
        nums.append(i)
    firstthree: list[int] = nums[:3]
    lasttwo: list[int] = nums[::-1][:2]
    tail: list[int] = nums[5:]
    for n in firstthree:
        print(f"take: {_vb(n)}")
    for n in lasttwo:
        print(f"rev:  {_vb(n)}")
    for n in tail:
        print(f"skip: {_vb(n)}")
    _m0 = (Some(max(nums)) if nums else None)
    match _m0:
        case Some(top):
            print(f"max:  {_vb(top)}")
        case None:
            print('empty')
    _m1 = next((Some(_i) for _i, x in enumerate(nums) if x > 6), None)
    match _m1:
        case Some(idx):
            print(f"pos:  {_vb(idx)}")
        case None:
            print('none over 6')


if __name__ == "__main__":
    main()

# A5 — Shape Areas

Exercises VBR's **sum types** — an `Enum` whose variants **carry data**.
Each shape is built by calling a variant constructor (`Shape.Circle(2.0)`);
the only way to read the payload back is to `Match` and unpack it, and the
compiler guarantees every `Match` handles every variant.

## VBR language features tested

- `Public Enum` with payload variants: scalar (`Circle(Double)`), multiple
  (`Rectangle(Double, Double)`), struct (`Point`), and `Vec<Point>` payloads
- `Match` unpacking in all three functions (`Area`, `Perimeter`,
  `Describe`) — the payload names bind inside the arm
- `Public Type Point` used as a variant payload, built with the literal
  constructor
- `Sqr` for the hypotenuse; the shoelace formula for polygon area
- Cross-module qualified calls (`Shapes.Area(...)`) from main and tests
- `Assert` on `Double` values that are exactly representable (π·r² for
  r=1.0, 2.0 are exact in these test values)

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/A5_shape_areas    # build + run
vbr test        projects/A5_shape_areas   # run the 9 tests
```

## Expected output

```
circle r=2  area=12.56636  perimeter=12.56636
rectangle 3x4  area=12  perimeter=14
triangle b=4 h=3  area=6  perimeter=12
polygon of 4 points  area=12  perimeter=14
empty  area=0  perimeter=0
```

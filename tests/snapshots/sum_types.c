// Data-carrying enums (sum types): each variant carries its own data. Build one
// with `Shape.Circle(r)`; pull the data back out by matching. This is the same
// shape as Option/Result — now you can define your own.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

typedef enum { Shape_Circle, Shape_Rectangle, Shape_Empty } ShapeTag;
typedef struct {
    ShapeTag tag;
    union {
        struct { double f0; } Circle;
        struct { double f0; double f1; } Rectangle;
    } data;
} Shape;

typedef struct { bool is_ok; double ok; char* err; } Result_double_str;
static double Result_double_str_unwrap(Result_double_str r) { if (!r.is_ok) { fprintf(stderr, "unwrapped an Err\n"); exit(1); } return r.ok; }

static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}
static char* vbr_from_double(double d) {
    char buf[64];
    for (int p = 1; p <= 17; p++) {
        snprintf(buf, sizeof buf, "%.*g", p, d);
        if (strtod(buf, NULL) == d) break;
    }
    return vbr_dup(buf);
}
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}

Result_double_str area(Shape s);

Result_double_str area(Shape s) {
    Shape _m0 = s;
    if (_m0.tag == Shape_Circle) {
        double r = _m0.data.Circle.f0;
        return (Result_double_str){ .is_ok = true, .ok = ((3.14159 * r) * r) };
    } else if (_m0.tag == Shape_Rectangle) {
        double w = _m0.data.Rectangle.f0;
        double h = _m0.data.Rectangle.f1;
        return (Result_double_str){ .is_ok = true, .ok = (w * h) };
    } else {
        return (Result_double_str){ .is_ok = true, .ok = 0.0 };
    }
}

int main(void) {
    Shape c = (Shape){ .tag = Shape_Circle, .data.Circle = { 2.0 } };
    Shape r = (Shape){ .tag = Shape_Rectangle, .data.Rectangle = { 3.0, 4.0 } };
    Result_double_str _t0 = area(c);
    if (!_t0.is_ok) { fprintf(stderr, "Error: %s\n", _t0.err); return 1; }
    printf("%s\n", vbr_concat("circle area = ", vbr_from_double(_t0.ok)));
    Result_double_str _t1 = area(r);
    if (!_t1.is_ok) { fprintf(stderr, "Error: %s\n", _t1.err); return 1; }
    printf("%s\n", vbr_concat("rect area   = ", vbr_from_double(_t1.ok)));
    Result_double_str _t2 = area((Shape){ .tag = Shape_Empty });
    if (!_t2.is_ok) { fprintf(stderr, "Error: %s\n", _t2.err); return 1; }
    printf("%s\n", vbr_concat("empty area  = ", vbr_from_double(_t2.ok)));
    return 0;
}

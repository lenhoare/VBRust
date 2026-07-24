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

double area(Shape s);

double area(Shape s) {
    Shape _m0 = s;
    if (_m0.tag == Shape_Circle) {
        double r = _m0.data.Circle.f0;
        return ((3.14159 * r) * r);
    } else if (_m0.tag == Shape_Rectangle) {
        double w = _m0.data.Rectangle.f0;
        double h = _m0.data.Rectangle.f1;
        return (w * h);
    } else {
        return 0.0;
    }
}

int main(void) {
    Shape c = (Shape){ .tag = Shape_Circle, .data.Circle = { 2.0 } };
    Shape r = (Shape){ .tag = Shape_Rectangle, .data.Rectangle = { 3.0, 4.0 } };
    printf("%s\n", vbr_concat("circle area = ", vbr_from_double(area(c))));
    printf("%s\n", vbr_concat("rect area   = ", vbr_from_double(area(r))));
    printf("%s\n", vbr_concat("empty area  = ", vbr_from_double(area((Shape){ .tag = Shape_Empty }))));
    return 0;
}

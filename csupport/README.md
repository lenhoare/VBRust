# csupport — vendored C libraries

Third-party C sources the **C backend** bundles into a project folder when a
program uses a standard-library namespace that has no libc equivalent. The
parallel of Python's `vbrpy/`: `vbr c` copies the needed `.c`/`.h` pair beside
the generated `main.c` and writes a `Makefile` that builds them together.

Each vendored library keeps its own upstream licence header in-file.

| library  | version | upstream                                   | licence | namespace |
|----------|---------|--------------------------------------------|---------|-----------|
| `cJSON`  | 1.7.19  | https://github.com/DaveGamble/cJSON        | MIT     | `Json`    |

These are checked in verbatim (not modified) so a C project builds with nothing
but a C compiler — no system packages, no network. Refresh a library by copying
its released `.c`/`.h` over the pair here and bumping the version above.

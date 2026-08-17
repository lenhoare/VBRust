#ifndef VBR_TCC_RUN_H
#define VBR_TCC_RUN_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    char *stdout_text;
    char *stderr_text;
    int exit_code;
    int ok;
} VbrTccResult;

void vbr_tcc_result_free(VbrTccResult *r);

/* Compile `c_source` with libtcc and run `main`. `tccdir` is TinyCC's runtime
 * dir (libtcc1.a); may be NULL if the install is already configured.
 * `use_prelude` strips #includes and prepends a libc-prototype prelude
 * (needed on Android, where there is no sysroot). */
int vbr_tcc_run(const char *c_source, const char *tccdir, int use_prelude,
                VbrTccResult *out);

#ifdef __cplusplus
}
#endif

#endif

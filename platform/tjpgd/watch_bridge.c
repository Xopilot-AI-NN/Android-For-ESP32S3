/* Streaming adapter: image pixels and compressed files never occupy a full-size framebuffer. */
#include "tjpgd.h"
typedef size_t (*watch_read)(void *, uint8_t *, size_t);
typedef struct { watch_read read; void *ctx; uint16_t *pixels; unsigned w,h,sw,sh; } View;
static size_t input(JDEC *jd, uint8_t *buffer, size_t n) { View *v = (View *)jd->device; return v->read(v->ctx, buffer, n); }
static int output(JDEC *jd, void *bitmap, JRECT *rect) {
    View *v = (View *)jd->device; uint16_t *src = bitmap; unsigned stride = rect->right - rect->left + 1;
    for (unsigned y = rect->top; y <= rect->bottom; ++y) for (unsigned x = rect->left; x <= rect->right; ++x) {
        unsigned dx = x * v->w / v->sw, dy = y * v->h / v->sh;
        if (dx < v->w && dy < v->h) v->pixels[(dy + (200-v->h)/2)*240 + dx + (240-v->w)/2] = src[(y-rect->top)*stride + x-rect->left];
    }
    return 1;
}
/* Rust owns the work/output buffers and serializes filesystem access around this call. */
int watch_jpeg(watch_read read, void *ctx, uint16_t *pixels, void *work, size_t size) {
    JDEC jd = {0}; View view = { .read=read, .ctx=ctx, .pixels=pixels };
    JRESULT result = jd_prepare(&jd, input, work, size, &view); if (result != JDR_OK) return result;
    if (!jd.width || !jd.height || (unsigned)jd.width * jd.height > 64000000u) return JDR_PAR;
    unsigned scale=0; while (scale<3 && ((jd.width>>scale)>480 || (jd.height>>scale)>400)) ++scale;
    view.sw=jd.width>>scale; view.sh=jd.height>>scale; if (!view.sw || !view.sh) return JDR_PAR;
    view.w=view.sw; view.h=view.sh;
    if (view.w>240) { view.h=view.h*240/view.w; view.w=240; }
    if (view.h>200) { view.w=view.w*200/view.h; view.h=200; }
    if (!view.w || !view.h) return JDR_PAR;
    return jd_decomp(&jd, output, scale);
}

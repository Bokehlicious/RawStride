/* Thin C shim around LibRaw's C API for raw-decode-bench.
 * Keeping LibRaw's large structs on the C side avoids mirroring their layout in Rust. */
#define LIBRAW_NODLL
#include <libraw/libraw.h>
#include <string.h>
#include <windows.h>

typedef struct {
    double open_ms, unpack_ms, process_ms, mkimg_ms, total_ms;
    int raw_width, raw_height;   /* sensor data */
    int width, height;           /* visible image (before half-size/crop) */
    int out_width, out_height;   /* produced RGB image */
    int ok;
    char error[160];
} pw_stats;

enum { PW_UNPACK_ONLY = 0, PW_FULL = 1, PW_HALF = 2 };

static double now_ms(void) {
    LARGE_INTEGER f, c;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart * 1000.0 / (double)f.QuadPart;
}

const char *pw_libraw_version(void) { return libraw_version(); }
unsigned pw_libraw_capabilities(void) { return libraw_capabilities(); }

/* mode: PW_UNPACK_ONLY / PW_FULL / PW_HALF; quality: LibRaw user_qual (0 linear, 3 AHD, ...);
 * crop: if > 0, a centred crop x crop box is processed (cropbox), simulating one tile. */
int pw_decode(const wchar_t *path, int mode, int quality, int crop, pw_stats *st) {
    memset(st, 0, sizeof *st);
    double t0 = now_ms(), t;
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { strcpy(st->error, "libraw_init failed"); return 0; }
    lr->params.use_camera_wb = 1;
    lr->params.output_bps = 8;
    lr->params.user_qual = quality;
    lr->params.half_size = (mode == PW_HALF) ? 1 : 0;

    int rc = libraw_open_wfile(lr, path);
    t = now_ms(); st->open_ms = t - t0; t0 = t;
    if (rc != LIBRAW_SUCCESS) goto fail;
    st->raw_width = lr->sizes.raw_width;
    st->raw_height = lr->sizes.raw_height;
    st->width = lr->sizes.width;
    st->height = lr->sizes.height;
    if (crop > 0 && crop < st->width && crop < st->height) {
        lr->params.cropbox[0] = (unsigned)((st->width - crop) / 2);
        lr->params.cropbox[1] = (unsigned)((st->height - crop) / 2);
        lr->params.cropbox[2] = (unsigned)crop;
        lr->params.cropbox[3] = (unsigned)crop;
    }

    rc = libraw_unpack(lr);
    t = now_ms(); st->unpack_ms = t - t0; t0 = t;
    if (rc != LIBRAW_SUCCESS) goto fail;
    if (mode == PW_UNPACK_ONLY) goto ok;

    rc = libraw_dcraw_process(lr);
    t = now_ms(); st->process_ms = t - t0; t0 = t;
    if (rc != LIBRAW_SUCCESS) goto fail;

    {
        int err = 0;
        libraw_processed_image_t *img = libraw_dcraw_make_mem_image(lr, &err);
        t = now_ms(); st->mkimg_ms = t - t0;
        if (!img) { rc = err; goto fail; }
        st->out_width = img->width;
        st->out_height = img->height;
        libraw_dcraw_clear_mem(img);
    }
ok:
    st->ok = 1;
    st->total_ms = st->open_ms + st->unpack_ms + st->process_ms + st->mkimg_ms;
    libraw_close(lr);
    return 1;
fail:
    strncpy(st->error, libraw_strerror(rc), sizeof st->error - 1);
    libraw_close(lr);
    return 0;
}

/* ---------- tile sessions: one open + unpack, then any number of processed regions ---------- */

typedef struct {
    double open_ms, unpack_ms;
    int width, height; /* visible image size */
    char error[160];
} pw_session_info;

typedef struct {
    double process_ms, mkimg_ms;
    int out_width, out_height;
    unsigned long long hash; /* FNV-1a of the RGB output, to check repeatability */
    char error[160];
} pw_tile_stats;

/* Opens and unpacks once. Returns NULL on failure (error in info->error). */
void *pw_session_open(const wchar_t *path, int quality, pw_session_info *info) {
    memset(info, 0, sizeof *info);
    double t0 = now_ms(), t;
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { strcpy(info->error, "libraw_init failed"); return 0; }
    lr->params.use_camera_wb = 1;
    lr->params.output_bps = 8;
    lr->params.user_qual = quality;
    int rc = libraw_open_wfile(lr, path);
    t = now_ms(); info->open_ms = t - t0; t0 = t;
    if (rc == LIBRAW_SUCCESS) {
        info->width = lr->sizes.width;
        info->height = lr->sizes.height;
        rc = libraw_unpack(lr);
        t = now_ms(); info->unpack_ms = t - t0;
    }
    if (rc != LIBRAW_SUCCESS) {
        strncpy(info->error, libraw_strerror(rc), sizeof info->error - 1);
        libraw_close(lr);
        return 0;
    }
    return lr;
}

/* Processes the region (x, y, w, h) of the visible image with the session's quality.
 * dcraw_process re-reads the unpacked RAW data, so it can be called repeatedly. */
int pw_session_tile(void *session, int x, int y, int w, int h, pw_tile_stats *st) {
    memset(st, 0, sizeof *st);
    libraw_data_t *lr = (libraw_data_t *)session;
    lr->params.cropbox[0] = (unsigned)x;
    lr->params.cropbox[1] = (unsigned)y;
    lr->params.cropbox[2] = (unsigned)w;
    lr->params.cropbox[3] = (unsigned)h;
    double t0 = now_ms(), t;
    int rc = libraw_dcraw_process(lr);
    t = now_ms(); st->process_ms = t - t0; t0 = t;
    if (rc != LIBRAW_SUCCESS) {
        strncpy(st->error, libraw_strerror(rc), sizeof st->error - 1);
        return 0;
    }
    int err = 0;
    libraw_processed_image_t *img = libraw_dcraw_make_mem_image(lr, &err);
    t = now_ms(); st->mkimg_ms = t - t0;
    if (!img) {
        strncpy(st->error, libraw_strerror(err), sizeof st->error - 1);
        return 0;
    }
    st->out_width = img->width;
    st->out_height = img->height;
    unsigned long long hsh = 1469598103934665603ULL;
    for (unsigned i = 0; i < img->data_size; i++) {
        hsh = (hsh ^ img->data[i]) * 1099511628211ULL;
    }
    st->hash = hsh;
    libraw_dcraw_clear_mem(img);
    return 1;
}

void pw_session_close(void *session) {
    if (session) libraw_close((libraw_data_t *)session);
}

/* ---------- verification: identical pixels across builds, and tile seams ---------- */

static unsigned long long fnv(const unsigned char *d, size_t n) {
    unsigned long long h = 1469598103934665603ULL;
    for (size_t i = 0; i < n; i++) h = (h ^ d[i]) * 1099511628211ULL;
    return h;
}

/* LibRaw raises the white level to the brightest value found in the processed area when that
 * exceeds 75 % of the camera maximum (adjust_maximum_thr). That makes the result depend on the
 * region; 0 disables it. Default as LibRaw: 0.75. */
static float g_adjust_maximum_thr = 0.75f;
void pw_set_adjust_maximum_thr(float thr) { g_adjust_maximum_thr = thr; }

static void verify_params(libraw_data_t *lr, int quality, int half, int auto_bright) {
    lr->params.adjust_maximum_thr = g_adjust_maximum_thr;
    lr->params.use_camera_wb = 1;
    lr->params.output_bps = 8;
    lr->params.user_qual = quality;
    lr->params.half_size = half;
    lr->params.user_flip = 0;            /* sensor orientation: the viewer rotates */
    lr->params.no_auto_bright = auto_bright ? 0 : 1;
}

/* Decodes the whole image and returns an FNV-1a hash of the 8-bit RGB output. */
int pw_decode_hash(const wchar_t *path, int quality, int half, int auto_bright,
                   unsigned long long *hash, int *w, int *h, char *error) {
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { strcpy(error, "libraw_init failed"); return 0; }
    verify_params(lr, quality, half, auto_bright);
    int rc = libraw_open_wfile(lr, path);
    if (rc == LIBRAW_SUCCESS) rc = libraw_unpack(lr);
    if (rc == LIBRAW_SUCCESS) rc = libraw_dcraw_process(lr);
    libraw_processed_image_t *img = 0;
    if (rc == LIBRAW_SUCCESS) img = libraw_dcraw_make_mem_image(lr, &rc);
    if (!img) {
        strncpy(error, libraw_strerror(rc), 159);
        libraw_close(lr);
        return 0;
    }
    *hash = fnv(img->data, img->data_size);
    *w = img->width;
    *h = img->height;
    libraw_dcraw_clear_mem(img);
    libraw_close(lr);
    return 1;
}

typedef struct {
    int margin;
    int out_w, out_h;      /* processed region incl. margin */
    int dx, dy;            /* best alignment against the full image (expected 0, 0) */
    double diff_pct;       /* % of tile pixels (inside the margin) that differ */
    int max_diff;          /* largest channel difference */
    double mean_abs;       /* mean absolute channel difference */
    int band_max;          /* largest difference within 16 px of the tile edge */
    int interior_max;      /* largest difference further inside */
    double process_ms;
    unsigned long long hash; /* of the processed tile, for comparing builds */
    char error[160];
} pw_seam_result;

/* Decodes the full image once, then the region (x, y, w, h) as a tile with each margin, and
 * compares the tile's inner w x h pixels with the same pixels of the full image. */
int pw_seam_check(const wchar_t *path, int auto_bright, int x, int y, int w, int h,
                  const int *margins, int n, pw_seam_result *out,
                  unsigned long long *full_hash, char *error) {
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { strcpy(error, "libraw_init failed"); return 0; }
    verify_params(lr, 3, 0, auto_bright);
    int rc = libraw_open_wfile(lr, path);
    if (rc == LIBRAW_SUCCESS) rc = libraw_unpack(lr);
    if (rc == LIBRAW_SUCCESS) rc = libraw_dcraw_process(lr);
    libraw_processed_image_t *full = 0;
    if (rc == LIBRAW_SUCCESS) full = libraw_dcraw_make_mem_image(lr, &rc);
    if (!full) { strncpy(error, libraw_strerror(rc), 159); libraw_close(lr); return 0; }
    *full_hash = fnv(full->data, full->data_size);
    const int FW = full->width, FH = full->height;

    for (int k = 0; k < n; k++) {
        pw_seam_result *r = &out[k];
        memset(r, 0, sizeof *r);
        int m = margins[k];
        r->margin = m;
        int x0 = x - m, y0 = y - m, tw = w + 2 * m, th = h + 2 * m;
        if (x0 < 0 || y0 < 0 || x0 + tw > FW || y0 + th > FH) {
            strcpy(r->error, "region outside image");
            continue;
        }
        lr->params.cropbox[0] = (unsigned)x0;
        lr->params.cropbox[1] = (unsigned)y0;
        lr->params.cropbox[2] = (unsigned)tw;
        lr->params.cropbox[3] = (unsigned)th;
        double t0 = now_ms();
        rc = libraw_dcraw_process(lr);
        libraw_processed_image_t *t = 0;
        if (rc == LIBRAW_SUCCESS) t = libraw_dcraw_make_mem_image(lr, &rc);
        r->process_ms = now_ms() - t0;
        if (!t) { strncpy(r->error, libraw_strerror(rc), 159); continue; }
        r->out_w = t->width;
        r->out_h = t->height;
        r->hash = fnv(t->data, t->data_size);
        /* Find the alignment (-2..2) with the fewest differing pixels, then measure it. */
        long best = -1;
        for (int dy = -2; dy <= 2; dy++)
            for (int dx = -2; dx <= 2; dx++) {
                long cnt = 0;
                for (int j = 0; j < h; j += 7) {
                    int ty = m + j + dy, fy = y + j;
                    if (ty < 0 || ty >= t->height) continue;
                    for (int i = 0; i < w; i += 7) {
                        int tx = m + i + dx, fx = x + i;
                        if (tx < 0 || tx >= t->width) continue;
                        const unsigned char *a = t->data + ((size_t)ty * t->width + tx) * 3;
                        const unsigned char *b = full->data + ((size_t)fy * FW + fx) * 3;
                        if (a[0] != b[0] || a[1] != b[1] || a[2] != b[2]) cnt++;
                    }
                }
                if (best < 0 || cnt < best) { best = cnt; r->dx = dx; r->dy = dy; }
            }
        long diff = 0, total = 0;
        double sum = 0;
        for (int j = 0; j < h; j++) {
            int ty = m + j + r->dy;
            if (ty < 0 || ty >= t->height) continue;
            for (int i = 0; i < w; i++) {
                int tx = m + i + r->dx;
                if (tx < 0 || tx >= t->width) continue;
                const unsigned char *a = t->data + ((size_t)ty * t->width + tx) * 3;
                const unsigned char *b = full->data + ((size_t)(y + j) * FW + (x + i)) * 3;
                int px = 0;
                for (int c = 0; c < 3; c++) {
                    int d = a[c] > b[c] ? a[c] - b[c] : b[c] - a[c];
                    sum += d;
                    if (d > px) px = d;
                }
                total++;
                if (px) {
                    diff++;
                    if (px > r->max_diff) r->max_diff = px;
                    int edge = i < 16 || j < 16 || i >= w - 16 || j >= h - 16;
                    if (edge) { if (px > r->band_max) r->band_max = px; }
                    else if (px > r->interior_max) r->interior_max = px;
                }
            }
        }
        r->diff_pct = total ? 100.0 * diff / total : 0;
        r->mean_abs = total ? sum / (3.0 * total) : 0;
        libraw_dcraw_clear_mem(t);
    }
    libraw_dcraw_clear_mem(full);
    libraw_close(lr);
    return 1;
}

/* Geometry and CFA pattern after unpack (to explain crop behaviour). */
int pw_probe_info(const wchar_t *path, unsigned *filters, int *top, int *left, int *w, int *h,
                  int *raw_w, int *raw_h, char *error) {
    libraw_data_t *lr = libraw_init(0);
    if (!lr) { strcpy(error, "libraw_init failed"); return 0; }
    int rc = libraw_open_wfile(lr, path);
    if (rc == LIBRAW_SUCCESS) rc = libraw_unpack(lr);
    if (rc != LIBRAW_SUCCESS) { strncpy(error, libraw_strerror(rc), 159); libraw_close(lr); return 0; }
    *filters = lr->idata.filters;
    *top = lr->sizes.top_margin;
    *left = lr->sizes.left_margin;
    *w = lr->sizes.width;
    *h = lr->sizes.height;
    *raw_w = lr->sizes.raw_width;
    *raw_h = lr->sizes.raw_height;
    libraw_close(lr);
    return 1;
}

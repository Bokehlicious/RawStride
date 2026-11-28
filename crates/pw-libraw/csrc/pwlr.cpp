/* Thin C shim around LibRaw for pw-libraw (ADR 0006).
 *
 * All decodes use the same fixed settings, chosen so that a region is pixel-identical to the
 * same area of a whole-image decode (docs/analyses/2026-11-04-m2-libraw-decode.md, follow-ups
 * 3 and 4): no auto white balance, no auto brightness, no white-level
 * adjustment from the image data, sensor orientation, 8-bit sRGB. The demosaic method is chosen per
 * render: AHD (the default, ADR 0006) or PPG (ADR 0008 section 11; the egui loupe, ADR 0009).
 * Each render sets the exposure and white balance (ADR 0027 sections 3.4 and 3.5).
 * use_camera_wb and use_camera_matrix follow the balance. Both adjustments are independent of
 * earlier renders and of the statistics and clipping map.
 *
 * Damaged files (M2 error handling): LibRaw does not return an error when the RAW data is
 * shorter than the file says (a truncated copy); many decoders read on and fill the rest. The
 * shim therefore opens the file through its own data stream that counts the bytes LibRaw asked
 * for but did not get while unpacking the RAW data, and it collects LibRaw's data-error reports.
 * A file with missing or corrupt RAW data is refused (pwlr_open returns NULL with an error that
 * starts with "damaged:"), so the viewer keeps the embedded preview. LibRaw's processing warnings
 * (white balance, colour profiles, ...) are only reported, never treated as damage. */
#define LIBRAW_NODLL
#include <libraw/libraw.h>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <math.h>
#include <new>
#include <stdio.h>
#include <string.h>

/* Bytes of RAW data a valid file may lack. Measured on all 24 complete test files (A7R IVA, A1,
 * GM1 RW2, two kinds of DNG): none comes up even one byte short while unpacking, so any shortfall
 * means data is missing. (A first version allowed 64 KiB for decoders that read in blocks; it let
 * a DNG cut to half its size through, because that decoder reads byte by byte.) */
static const long long PWLR_SHORT_TOLERANCE = 0;

/* The stream LibRaw uses for wide paths on Windows, plus a count of short reads. */
class PwStream : public LibRaw_bigfile_buffered_datastream {
public:
  explicit PwStream(const wchar_t *path) : LibRaw_bigfile_buffered_datastream(path) {}
  std::atomic<int> watching{0};
  std::atomic<long long> short_reads{0};
  std::atomic<long long> missing{0};
  virtual int read(void *ptr, size_t size, size_t nmemb) {
    int r = LibRaw_bigfile_buffered_datastream::read(ptr, size, nmemb);
    if (watching.load() && size > 0 && nmemb > 0 && (r < 0 || (size_t)r < nmemb)) {
      size_t got = r < 0 ? 0 : (size_t)r;
      short_reads++;
      missing += (long long)((nmemb - got) * size);
    }
    return r;
  }
  /* get_char() needs no override: past its buffer it calls read() above. */
};

struct pwlr_preset {
  char name[24];
  float mul[4];
};

static constexpr int PWLR_MAX_PRESETS = 96;

struct PwSession {
  LibRaw *lr;
  PwStream *stream;
  int data_errors;
  /* The check of the render under way (T327), set right before dcraw_process and cleared right
   * after it; NULL otherwise. */
  int (*stop)(void *ctx);
  void *ctx;
  /* 1 when a failed render (a stop, or any other failure) made LibRaw free the unpacked RAW data:
   * nothing but pwlr_close and pwlr_spent may be used on the session any more. */
  int spent;
  pwlr_preset presets[PWLR_MAX_PRESETS];
  int preset_count;
};

static const char PWLR_SPENT_TEXT[] =
    "spent: a render failed or was stopped and LibRaw dropped the RAW data; open the file again";

extern "C" {

const char *pwlr_version(void) { return libraw_version(); }
unsigned pwlr_capabilities(void) { return libraw_capabilities(); }

typedef struct {
  int width, height; /* visible image, sensor orientation */
  int flip;          /* LibRaw flip code of the file (0, 3, 5, 6) */
  /* Diagnostics of the unpack (also on failure): */
  unsigned warnings;     /* LibRaw process_warnings (LIBRAW_WARN_*): informational only */
  int data_errors;       /* LibRaw data-error reports (corrupt data or unexpected end) */
  long long short_reads; /* reads that returned less than asked while unpacking */
  long long missing;     /* bytes asked for but not in the file while unpacking */
  long long file_size;
  int damaged;           /* 1 = refused because RAW data is missing or corrupt */
  /* The camera's brightness (ADR 0027 W5-Q5, T331), read once after a successful unpack, in EV as
   * the file has it: */
  float camera_ev;
  int camera_ev_from; /* 0 none, 1 DNG BaselineExposure, 2 Fujifilm (ExifTool's RawExposureBias) */
  float cam_mul[4], pre_mul[4], matrix_mul[4];
  int daylight_from; /* 0 none, 1 pre_mul, 2 matrix */
  int measurable;
  int preset_count;
  unsigned char other_ids[16];
  int other_count;
  long long presets_ns;
  char error[200];
} pwlr_info;

/* Where pwlr_info.camera_ev came from. */
static const int PWLR_CAMERA_EV_NONE = 0;
static const int PWLR_CAMERA_EV_DNG = 1;
static const int PWLR_CAMERA_EV_FUJIFILM = 2;

/* LibRaw's value for "not in the file" of dng_levels.baseline_exposure and
 * makernotes.fuji.ExpoMidPointShift (set by LibRaw's init, utils/init_close_utils.cpp, 0.22.3). */
static const float PWLR_LIBRAW_NOT_SET = -999.0f;

/* The camera's brightness of an unpacked file: a DNG's BaselineExposure when the file has the
 * tag; for a Fujifilm file that is not a DNG, the maker note's ExpoMidPointShift; else none. A
 * value that is not finite, or LibRaw's "not set" value, counts as none.
 *
 * - DNG: LibRaw 0.22.3 copies the tag's value from the raw IFD (or IFD 0) into
 *   color.dng_levels.baseline_exposure but never sets LIBRAW_DNGFM_BASELINEEXPOSURE in
 *   color.dng_levels.parsedfields (metadata/identify.cpp), so the flag cannot tell whether the file
 *   has the tag; the value differs from PWLR_LIBRAW_NOT_SET exactly when it does (the owner's
 *   decision, T331, 2026-11-25).
 * - Fujifilm: LibRaw reads the maker note's tag 0x9650 as a / b, which is ExifTool's
 *   RawExposureBias with the same sign (-0.72 on the owner's X-T30 files, ExifTool's unrounded
 *   value; found in T331), so it is kept as LibRaw has it. */
static void pwlr_camera_ev(LibRaw *lr, pwlr_info *info) {
  info->camera_ev = 0.0f;
  info->camera_ev_from = PWLR_CAMERA_EV_NONE;
  float ev = 0.0f;
  int from = PWLR_CAMERA_EV_NONE;
  if (lr->imgdata.idata.dng_version != 0) {
    ev = lr->imgdata.color.dng_levels.baseline_exposure;
    from = PWLR_CAMERA_EV_DNG;
  } else if (lr->imgdata.idata.maker_index == LIBRAW_CAMERAMAKER_Fujifilm) {
    ev = lr->imgdata.makernotes.fuji.ExpoMidPointShift;
    from = PWLR_CAMERA_EV_FUJIFILM;
  }
  if (from != PWLR_CAMERA_EV_NONE && isfinite(ev) && ev != PWLR_LIBRAW_NOT_SET) {
    info->camera_ev = ev;
    info->camera_ev_from = from;
  }
}

static void set_error(char *dst, const char *what, int rc) {
  snprintf(dst, 200, "%s: %s", what, libraw_strerror(rc));
}

/* LibRaw calls this for corrupt data (offset = position) or an unexpected end (offset = -1).
 * Counting it also keeps LibRaw from printing to stderr. */
static void on_data_error(void *data, const char *, const INT64) {
  PwSession *s = (PwSession *)data;
  if (s) s->data_errors++;
}

/* LibRaw's progress handler (T327): a non-zero return cancels the render under way. LibRaw calls
 * it between the stages of dcraw_process; it throws nothing. */
static int on_progress(void *data, enum LibRaw_progress, int, int) {
  PwSession *s = (PwSession *)data;
  if (s && s->stop) return s->stop(s->ctx) ? 1 : 0;
  return 0;
}

void pwlr_close(void *session);
static void pwlr_read_white_balance(PwSession *s, pwlr_info *info);

/* Opens and unpacks; NULL on failure (info->error says why; "damaged: ..." for missing or
 * corrupt RAW data). */
void *pwlr_open(const wchar_t *path, pwlr_info *info) {
  memset(info, 0, sizeof *info);
  PwSession *s = new PwSession{};
  s->stream = new PwStream(path);
  if (!s->stream->valid()) {
    strcpy(info->error, "open: cannot read the file");
    pwlr_close(s);
    return 0;
  }
  info->file_size = s->stream->size();
  s->lr = new LibRaw(0);
  LibRaw *lr = s->lr;
  lr->set_dataerror_handler(on_data_error, s);
  lr->imgdata.params.use_camera_wb = 1;
  lr->imgdata.params.use_auto_wb = 0;
  lr->imgdata.params.no_auto_bright = 1;
  lr->imgdata.params.bright = 1.0f;
  lr->imgdata.params.adjust_maximum_thr = 0.0f;
  lr->imgdata.params.user_flip = 0;
  lr->imgdata.params.output_bps = 8;
  lr->imgdata.params.output_color = 1; /* sRGB */
  lr->imgdata.params.user_qual = 3;    /* AHD */
  lr->imgdata.params.highlight = 0;    /* clip */
  int rc = lr->open_datastream(s->stream);
  if (rc != LIBRAW_SUCCESS) {
    set_error(info->error, "open", rc);
    pwlr_close(s);
    return 0;
  }
  info->flip = lr->imgdata.sizes.flip;
  info->width = lr->imgdata.sizes.width;
  info->height = lr->imgdata.sizes.height;
  s->stream->watching = 1;
  rc = lr->unpack();
  s->stream->watching = 0;
  info->warnings = lr->imgdata.process_warnings;
  /* The handler and LibRaw's own counter see the same reports; take the larger. */
  info->data_errors = s->data_errors > lr->error_count() ? s->data_errors : lr->error_count();
  info->short_reads = s->stream->short_reads.load();
  info->missing = s->stream->missing.load();
  if (rc != LIBRAW_SUCCESS) {
    /* An unexpected end of file inside the RAW data surfaces here as an I/O error. */
    info->damaged = info->data_errors > 0 || info->missing > 0;
    if (info->damaged)
      snprintf(info->error, 200, "damaged: RAW data incomplete (%s)", libraw_strerror(rc));
    else
      set_error(info->error, "unpack", rc);
    pwlr_close(s);
    return 0;
  }
  if (info->missing > PWLR_SHORT_TOLERANCE) {
    info->damaged = 1;
    snprintf(info->error, 200,
             "damaged: RAW data incomplete, %lld bytes missing (file %lld bytes, truncated?)",
             info->missing, info->file_size);
    pwlr_close(s);
    return 0;
  }
  if (info->data_errors > 0) {
    info->damaged = 1;
    snprintf(info->error, 200, "damaged: LibRaw reported %d data error(s) in the RAW data",
             info->data_errors);
    pwlr_close(s);
    return 0;
  }
  pwlr_camera_ev(lr, info);
  pwlr_read_white_balance(s, info);
  lr->set_progress_handler(on_progress, s);
  return s;
}

/* LibRaw user_qual values the shim accepts. */
static const int PWLR_QUAL_PPG = 2;
static const int PWLR_QUAL_AHD = 3;

/* The exposure per render (ADR 0027 section 3.4, T331). LibRaw's exp_shift is usable from 0.25 to
 * 8 (-2 to +3 EV). exp_preser (0 to 1) keeps the highlights from clipping when the image is made
 * brighter (LibRaw ignores it when darkening); 1.0 is the starting value of ADR 0027 W5-Q2, to be
 * tuned in the owner's hand check. */
static const float PWLR_EXP_SHIFT_MIN = 0.25f;
static const float PWLR_EXP_SHIFT_MAX = 8.0f;
static const float PWLR_EXP_PRESER = 1.0f;

/* ---- The box an X-Trans region is processed in (T197) ----
 *
 * LibRaw demosaics X-Trans (filters == LIBRAW_XTRANS) with its own interpolation (xtrans_interpolate,
 * 1 pass for PPG, 3 for AHD), which needs a box that differs from the one asked for:
 * - it refuses an image under LIBRAW_AHD_TILE (512) px wide or high with IO_CORRUPT, and LibRaw
 *   then recycles the whole session (the unpacked RAW data is gone; every later call fails);
 * - LibRaw rounds an X-Trans crop's origin down to a multiple of 6 (the pattern) without an error;
 * - it works in tiles of 512 px that start 3 px into the box and follow each other every 496 px
 *   (LIBRAW_AHD_TILE - 16), and a pixel near a tile's seam depends on where the seam is; a region
 *   is identical to the same area of a whole-image render only when the box's tiles lie where the
 *   whole image's do, so the origin must be a multiple of 496 as well;
 * - the pixels within about 24 px of the box's own edge differ (measured on _DSF4243.RAF and
 *   _DSF4271.RAF, X-T30: PPG needed 16 px, AHD 24 px).
 * The box is therefore the asked one plus PWLR_XTRANS_EDGE on every side, its origin rounded down
 * to a multiple of PWLR_XTRANS_ALIGN, at least PWLR_XTRANS_MIN px wide and high, and clamped to
 * the image. A Bayer file is processed in exactly the asked box. */
static const int PWLR_XTRANS_ALIGN = 1488; /* the least common multiple of 6 and 496 */
static const int PWLR_XTRANS_MIN = LIBRAW_AHD_TILE;
static const int PWLR_XTRANS_EDGE = 32;
static_assert(PWLR_XTRANS_ALIGN % 6 == 0 && PWLR_XTRANS_ALIGN % (LIBRAW_AHD_TILE - 16) == 0,
              "the X-Trans box origin must keep the pattern and the tiles of xtrans_interpolate");

/* One axis of the X-Trans box: the asked span [a, a + len) of an axis of `size` px becomes
 * [*lo_out, *lo_out + *len_out), which holds it. Returns 0 when the image is too small for X-Trans interpolation. */
static int pwlr_xtrans_axis(int a, int len, int size, int *lo_out, int *len_out) {
  if (size < PWLR_XTRANS_MIN) return 0;
  int lo = a - PWLR_XTRANS_EDGE;
  if (lo < 0) lo = 0;
  lo -= lo % PWLR_XTRANS_ALIGN;
  int hi = a + len + PWLR_XTRANS_EDGE;
  if (hi > size) hi = size;
  if (hi - lo < PWLR_XTRANS_MIN) {
    hi = lo + PWLR_XTRANS_MIN;
    if (hi > size) {
      hi = size;
      lo = size - PWLR_XTRANS_MIN;
      lo -= lo % PWLR_XTRANS_ALIGN;
    }
  }
  *lo_out = lo;
  *len_out = hi - lo;
  return 1;
}

/* Processes the region (x, y, w, h) of the visible image, or the whole image if w == 0, with the
 * demosaic method `quality` (LibRaw user_qual: 2 PPG or 3 AHD; anything else is refused).
 * half != 0 gives half size (2x2 binning, whole image only; no demosaicing). Returns a LibRaw
 * image that the caller frees with pwlr_free; NULL on failure. For a region, *ox, *oy, *ow and
 * *oh are set to the box of the visible image it was processed in, which the returned image is:
 * (x, y, w, h) on a Bayer file, and on an X-Trans file the wider box above, which holds the asked
 * one. For the whole image they are 0.
 *
 * `stop` (may be NULL) is asked with `ctx` before LibRaw starts and while it works; a non-zero
 * answer ends the render with an error that starts with "cancelled" (T327). Before LibRaw starts
 * that costs nothing. If LibRaw frees the unpacked RAW data when the render fails (a stop while it
 * works always does, in LibRaw 0.22), the session is spent (pwlr_spent): the error of every later
 * render, statistics and clipping map starts with "spent:" and LibRaw is not called.
 *
 * `exp_shift` is the exposure of this render (ADR 0027 section 3.4, T331): LibRaw's linear factor
 * (2^EV) applied before the demosaic, 1.0 for none. Every render
 * sets exp_correc, exp_shift and exp_preser, so nothing of an earlier render's exposure is left. A
 * factor that is not finite or lies outside 0.25 to 8 (LibRaw's usable range, -2 to +3 EV) is
 * refused before LibRaw is called, and the session is untouched.
 * `user_mul` supplies this render's white balance (R, G1, B, G2), or NULL for as shot. Each
 * render sets the four values, use_camera_wb and use_camera_matrix. A custom balance disables
 * camera WB so it cannot overwrite user_mul, and retains the embedded colour matrix when present.
 * Invalid multipliers are refused before LibRaw is called, leaving the session untouched. */
libraw_processed_image_t *pwlr_render(void *session, int quality, int half, float exp_shift,
                                      const float *user_mul, int x,
                                      int y, int w, int h, int *ox, int *oy, int *ow, int *oh,
                                      char *error, int (*stop)(void *ctx), void *ctx) {
  *ox = 0;
  *oy = 0;
  *ow = 0;
  *oh = 0;
  PwSession *s = (PwSession *)session;
  if (s->spent) {
    strcpy(error, PWLR_SPENT_TEXT);
    return 0;
  }
  if (quality != PWLR_QUAL_PPG && quality != PWLR_QUAL_AHD) {
    snprintf(error, 200, "unsupported demosaic method %d", quality);
    return 0;
  }
  if (!isfinite(exp_shift) || exp_shift < PWLR_EXP_SHIFT_MIN || exp_shift > PWLR_EXP_SHIFT_MAX) {
    snprintf(error, 200, "exposure shift %g is outside 0.25 to 8", (double)exp_shift);
    return 0;
  }
  if (user_mul) {
    for (int c = 0; c < 4; c++) {
      if (!isfinite(user_mul[c]) || user_mul[c] <= 0.0f) {
        strcpy(error, "white balance multipliers are not usable");
        return 0;
      }
    }
  }
  if (stop && stop(ctx)) {
    strcpy(error, "cancelled: the render was stopped before it started");
    return 0;
  }
  LibRaw *lr = s->lr;
  lr->imgdata.params.user_qual = quality;
  lr->imgdata.params.half_size = half ? 1 : 0;
  lr->imgdata.params.exp_correc = (exp_shift != 1.0f) ? 1 : 0;
  lr->imgdata.params.exp_shift = exp_shift;
  lr->imgdata.params.exp_preser = PWLR_EXP_PRESER;
  for (int c = 0; c < 4; c++) lr->imgdata.params.user_mul[c] = user_mul ? user_mul[c] : 0.0f;
  lr->imgdata.params.use_camera_wb = user_mul ? 0 : 1;
  lr->imgdata.params.use_camera_matrix = user_mul ? 3 : 1;
  if (w > 0 && h > 0) {
    /* The layout saved at the unpack; a render changes imgdata.idata and imgdata.sizes. */
    const libraw_rawdata_t &R = lr->imgdata.rawdata;
    const int width = (int)R.sizes.width, height = (int)R.sizes.height;
    if (x < 0 || y < 0 || x > width - w || y > height - h) {
      snprintf(error, 200, "region %dx%d at %d,%d is outside the image (%dx%d)", w, h, x, y,
               width, height);
      return 0;
    }
    if (R.iparams.filters == LIBRAW_XTRANS && !half) {
      if (!pwlr_xtrans_axis(x, w, width, &x, &w) || !pwlr_xtrans_axis(y, h, height, &y, &h)) {
        snprintf(error, 200, "the image (%dx%d) is smaller than X-Trans processing needs (%d px)",
                 width, height, PWLR_XTRANS_MIN);
        return 0;
      }
    }
    *ox = x;
    *oy = y;
    *ow = w;
    *oh = h;
    lr->imgdata.params.cropbox[0] = (unsigned)x;
    lr->imgdata.params.cropbox[1] = (unsigned)y;
    lr->imgdata.params.cropbox[2] = (unsigned)w;
    lr->imgdata.params.cropbox[3] = (unsigned)h;
  } else {
    lr->imgdata.params.cropbox[0] = 0;
    lr->imgdata.params.cropbox[1] = 0;
    lr->imgdata.params.cropbox[2] = 0xFFFFFFFFu;
    lr->imgdata.params.cropbox[3] = 0xFFFFFFFFu;
  }
  s->stop = stop;
  s->ctx = ctx;
  int rc = lr->dcraw_process();
  s->stop = nullptr;
  s->ctx = nullptr;
  if (rc != LIBRAW_SUCCESS) {
    /* LibRaw's exception handler in dcraw_process calls recycle() on every exception (a stop, a
     * corrupt-data error, out of memory): the unpacked RAW data is then gone. In LibRaw 0.22 a stop
     * while LibRaw works always does it (T327 measured so). */
    if (!lr->imgdata.rawdata.raw_alloc) s->spent = 1;
    if (rc == LIBRAW_CANCELLED_BY_CALLBACK) {
      snprintf(error, 200, "cancelled: %s%s", libraw_strerror(rc),
               s->spent ? " (the session is spent)" : "");
    } else {
      set_error(error, "process", rc);
    }
    /* A kept session holds only the unpacked RAW data, as after a success. */
    if (!s->spent) lr->free_image();
    return 0;
  }
  libraw_processed_image_t *img = lr->dcraw_make_mem_image(&rc);
  /* The processed buffer (image[]) is not needed once copied out; a kept session then holds
   * only the unpacked RAW data. The next render rebuilds it from the RAW data. */
  lr->free_image();
  if (!img) { set_error(error, "make_mem_image", rc); return 0; }
  if (img->type != LIBRAW_IMAGE_BITMAP || img->colors != 3 || img->bits != 8) {
    LibRaw::dcraw_clear_mem(img);
    strcpy(error, "unexpected output format");
    return 0;
  }
  return img;
}

void pwlr_image(const libraw_processed_image_t *img, int *w, int *h, const unsigned char **data,
                unsigned *size) {
  *w = img->width;
  *h = img->height;
  *data = img->data;
  *size = img->data_size;
}

void pwlr_free(libraw_processed_image_t *img) { LibRaw::dcraw_clear_mem(img); }

/* ---- Statistics of the unpacked RAW data (ADR 0017 section 2.6, U7; T142) ----
 *
 * Read from the data pwlr_open unpacked (imgdata.rawdata), with the sizes, colours and levels
 * LibRaw saved there at the end of the unpack: a render changes imgdata.color, imgdata.idata and
 * the image buffer, never these. Nothing is decoded and no LibRaw function is called.
 *
 * Per colour channel (R, G, B; both greens of a mosaic count as G):
 * - histogram: 256 bins of the value after the black level is subtracted and the camera's white
 *   balance applied (smallest multiplier 1, as the render), scaled so the white level is the top
 *   bin, on an sRGB-like curve;
 * - clipped: pixels whose raw value is at or above the white level (linear_max of the channel
 *   when the file gives it, else maximum), before white balance.
 * Deep shadows: pixels where every channel is below 1/256 of the white level above black. A
 * mosaic has one channel per photosite, so it is counted in cells that hold every colour (2x2
 * for a Bayer pattern, 3x3 for X-Trans): a cell is dark when all its photosites are. */
typedef struct {
  unsigned long long histogram[3][256];
  unsigned long long pixels[3];      /* pixels counted per channel (the sum of its bins) */
  unsigned long long clipped[3];     /* of them, at or above the white level */
  unsigned long long shadow_pixels;  /* pixels judged for the deep shadows */
  unsigned long long deep_shadows;   /* of them, with every channel in the deep shadows */
} pwlr_stats;

static double pwlr_curve(double x) {
  if (x <= 0.0) return 0.0;
  if (x >= 1.0) return 1.0;
  return x <= 0.0031308 ? 12.92 * x : 1.055 * pow(x, 1.0 / 2.4) - 0.055;
}

/* The levels of one CFA colour (0..3): black (LibRaw's common level, the colour's own and
 * `common`, the part every value of a black level pattern shares), white and the deep-shadow
 * limit (a value after black subtraction). */
struct PwLimits {
  unsigned black, white, shadow;
};

/* The levels of one CFA colour and the histogram bin of every value after black subtraction. */
struct PwLevels {
  unsigned black, white, shadow;
  unsigned char bin[65536];
};

/* What the statistics and the clipping map read from the unpacked RAW data, once its layout has
 * passed the checks of pwlr_layout_of. */
struct PwLayout {
  unsigned width, height; /* the visible image */
  PwLimits lim[4];        /* per CFA colour */
  /* A black level pattern on top of the per-colour level (prows x pcols values): `rest` is what
   * is left per position after `common` (in lim[].black), `pattern` whether any is. */
  unsigned prows, pcols;
  bool pattern;
  unsigned rest[LIBRAW_CBLACK_SIZE];
  /* A mosaic (raw_image): one colour per photosite, cfa[row % frows][col % fcols]. Otherwise a
   * three- or four-colour image (color3_image or color4_image) with n values per pixel. */
  bool mosaic;
  unsigned char cfa[8][6];
  unsigned frows, fcols;
  unsigned n;
  /* Photosites per side of a cell that holds every colour: 2 for a Bayer pattern and for a
   * three- or four-colour image, 3 for X-Trans. */
  unsigned cell;
};

static void pwlr_limits(const libraw_colordata_t &C, unsigned common, PwLimits *lim) {
  for (int c = 0; c < 4; c++) {
    PwLimits &l = lim[c];
    l.black = C.black + C.cblack[c] + common;
    l.white = C.linear_max[c] ? C.linear_max[c] : C.maximum;
    if (l.white > 65535u) l.white = 65535u;
    double span = l.white > l.black ? (double)(l.white - l.black) : 1.0;
    /* "below 1/256 of the white level above black" */
    l.shadow = (unsigned)ceil(span / 256.0);
  }
}

static void pwlr_levels(const PwLimits *lim, const float mul[4], PwLevels *lv) {
  for (int c = 0; c < 4; c++) {
    PwLevels &l = lv[c];
    l.black = lim[c].black;
    l.white = lim[c].white;
    l.shadow = lim[c].shadow;
    double span = l.white > l.black ? (double)(l.white - l.black) : 1.0;
    for (unsigned v = 0; v < 65536u; v++) {
      int b = (int)(pwlr_curve((double)v / span * (double)mul[c]) * 256.0);
      l.bin[v] = (unsigned char)(b > 255 ? 255 : b);
    }
  }
}

/* The layout checks shared by the statistics and the clipping map: 1 with *L filled, 0 when the
 * layout is not one they are made for (error says which). Allocates nothing and throws nothing. */
static int pwlr_layout_of(const libraw_rawdata_t &R, PwLayout *L, char *error) {
  const libraw_image_sizes_t &S = R.sizes;
  const libraw_iparams_t &P = R.iparams;
  const libraw_colordata_t &C = R.color;
  const unsigned width = S.width, height = S.height;
  L->width = width;
  L->height = height;
  if (width == 0 || height == 0) { strcpy(error, "no image"); return 0; }
  if (P.colors != 3 || strncmp(P.cdesc, "RGB", 3) != 0) {
    strcpy(error, "not an RGB sensor");
    return 0;
  }
  if (R.ph1_cblack || R.ph1_rblack) {
    strcpy(error, "black level per row and column");
    return 0;
  }
  if ((unsigned)S.top_margin + height > S.raw_height ||
      (unsigned)S.left_margin + width > S.raw_width) {
    strcpy(error, "the visible image is not inside the RAW data");
    return 0;
  }
  /* A black level pattern on top of the per-channel level (cblack[4] x cblack[5] values). The
   * part all its values share belongs to the black level itself (so the white level above black
   * is right); `rest` is what is left per position, and `pattern` whether any is. */
  const unsigned prows = C.cblack[4], pcols = C.cblack[5];
  if ((prows || pcols) && (!prows || !pcols || prows > LIBRAW_CBLACK_SIZE ||
                           pcols > LIBRAW_CBLACK_SIZE || prows * pcols > LIBRAW_CBLACK_SIZE - 6)) {
    strcpy(error, "unknown black level pattern");
    return 0;
  }
  L->prows = prows;
  L->pcols = pcols;
  L->pattern = false;
  unsigned common = 0;
  if (prows && pcols) {
    common = C.cblack[6];
    for (unsigned i = 1; i < prows * pcols; i++)
      if (C.cblack[6 + i] < common) common = C.cblack[6 + i];
    for (unsigned i = 0; i < prows * pcols; i++) {
      L->rest[i] = C.cblack[6 + i] - common;
      if (L->rest[i]) L->pattern = true;
    }
  }
  pwlr_limits(C, common, L->lim);

  if (R.raw_image) {
    L->mosaic = true;
    L->n = 1;
    if (S.raw_pitch < (unsigned)S.raw_width * 2u) { strcpy(error, "short RAW rows"); return 0; }
    if (P.filters >= 1000) {
      L->frows = 8; L->fcols = 2; L->cell = 2;
      for (unsigned r = 0; r < 8; r++)
        for (unsigned c = 0; c < 2; c++)
          L->cfa[r][c] = (unsigned char)((P.filters >> ((((r << 1) & 14) | (c & 1)) << 1)) & 3);
    } else if (P.filters == 9) {
      L->frows = 6; L->fcols = 6; L->cell = 3;
      for (unsigned r = 0; r < 6; r++)
        for (unsigned c = 0; c < 6; c++) {
          int x = P.xtrans[r][c];
          if (x < 0 || x > 2) { strcpy(error, "unknown X-Trans pattern"); return 0; }
          L->cfa[r][c] = (unsigned char)x;
        }
    } else {
      strcpy(error, "not a Bayer or X-Trans pattern");
      return 0;
    }
  } else if (R.color3_image || R.color4_image) {
    /* Three or four values per pixel (the fourth is not a colour of an RGB file). */
    L->mosaic = false;
    L->n = R.color4_image ? 4 : 3;
    L->frows = L->fcols = 0;
    L->cell = 2;
    if ((size_t)S.raw_pitch < (size_t)((unsigned)S.left_margin + width) * L->n * sizeof(ushort)) {
      strcpy(error, "short RAW rows");
      return 0;
    }
  } else {
    strcpy(error, "no integer RAW data");
    return 0;
  }
  return 1;
}

static bool pwlr_usable_rgb(const float *mul) {
  for (int c = 0; c < 3; c++)
    if (!isfinite(mul[c]) || mul[c] <= 0.0f) return false;
  return true;
}

static bool pwlr_same_balance(const float *a, const float *b) {
  if (!pwlr_usable_rgb(a) || !pwlr_usable_rgb(b)) return false;
  for (int c : {0, 2}) {
    const double ar = (double)a[c] / a[1], br = (double)b[c] / b[1];
    if (fabs(ar - br) > 0.005 * br) return false;
  }
  return true;
}

/* dcraw's sRGB/D65 matrix, used to derive the daylight balance from cam_xyz. */
static constexpr double PWLR_XYZ_RGB[3][3] = {
    {0.412453, 0.357580, 0.180423},
    {0.212671, 0.715160, 0.072169},
    {0.019334, 0.119193, 0.950227},
};

static void pwlr_matrix_balance(const libraw_colordata_t &color, float *out) {
  float mul[3];
  for (int i = 0; i < 3; i++) {
    double num = 0.0;
    for (int j = 0; j < 3; j++) {
      double cam_rgb = 0.0;
      for (int k = 0; k < 3; k++) cam_rgb += color.cam_xyz[i][k] * PWLR_XYZ_RGB[k][j];
      num += cam_rgb;
    }
    if (!isfinite(num) || num <= 0.0) return;
    mul[i] = (float)(1.0 / num);
  }
  if (!pwlr_usable_rgb(mul)) return;
  for (int i = 0; i < 3; i++) out[i] = mul[i];
  out[3] = mul[1];
}

static void pwlr_add_preset(PwSession *s, const char *name, const float *mul) {
  if (s->preset_count == PWLR_MAX_PRESETS || !pwlr_usable_rgb(mul)) return;
  for (int i = 0; i < s->preset_count; i++)
    if (strcmp(s->presets[i].name, name) == 0) return;
  pwlr_preset &preset = s->presets[s->preset_count++];
  snprintf(preset.name, sizeof preset.name, "%s", name);
  memcpy(preset.mul, mul, sizeof preset.mul);
}

static void pwlr_read_white_balance(PwSession *s, pwlr_info *info) {
  const auto start = std::chrono::steady_clock::now();
  const libraw_rawdata_t &R = s->lr->imgdata.rawdata;
  const libraw_colordata_t &color = R.color;
  memcpy(info->cam_mul, color.cam_mul, sizeof info->cam_mul);
  memcpy(info->pre_mul, color.pre_mul, sizeof info->pre_mul);
  pwlr_matrix_balance(color, info->matrix_mul);
  /* In the test set, pre_mul agrees with the D65 matrix for ARW, RAF and RW2. The Ricoh
   * DNGs use the matrix fallback, and the Xiaomi DNGs have no usable cam_xyz matrix. */
  if (pwlr_same_balance(info->pre_mul, info->matrix_mul)) {
    info->daylight_from = 1;
  } else if (pwlr_usable_rgb(info->matrix_mul)) {
    info->daylight_from = 2;
  }

  static constexpr const char *names[] = {
      "Daylight", "Fluorescent", "Tungsten", "Flash", nullptr, nullptr, nullptr, nullptr,
      "Fine Weather", "Cloudy", "Shade", "Fluorescent D", "Fluorescent N", "Fluorescent W",
      "Fluorescent WW", "Fluorescent L", "Illuminant A", "Illuminant B", "Illuminant C",
      "D55", "D65", "D75", "D50", "Studio Tungsten",
  };
  static constexpr const char *extra_names[] = {"Sunset", "Underwater", "Fluorescent High",
                                                "Mercury Vapor"};
  for (int id = 0; id < 256; id++) {
    const auto &row = color.WB_Coeffs[id];
    const char *name = id >= 1 && id <= 24 ? names[id - 1] : nullptr;
    if (id >= 64 && id <= 67) name = extra_names[id - 64];
    if (name) {
      const float mul[4] = {(float)row[0], (float)row[1], (float)row[2], (float)row[3]};
      pwlr_add_preset(s, name, mul);
    } else if ((row[0] || row[1] || row[2] || row[3]) && info->other_count < 16) {
      info->other_ids[info->other_count++] = (unsigned char)id;
    }
  }

  const float *temperatures[64];
  int count = 0;
  for (const auto &row : color.WBCT_Coeffs)
    if (isfinite(row[0]) && row[0] > 0.0f) temperatures[count++] = row;
  std::sort(temperatures, temperatures + count,
            [](const float *a, const float *b) { return a[0] < b[0]; });
  for (int i = 0; i < count; i++) {
    char name[24];
    const int length = snprintf(name, sizeof name, "%.0f K", round((double)temperatures[i][0]));
    if (length > 0 && length < (int)sizeof name) pwlr_add_preset(s, name, temperatures[i] + 1);
  }
  info->preset_count = s->preset_count;
  PwLayout layout;
  char error[200];
  info->measurable = pwlr_layout_of(R, &layout, error);
  info->presets_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(
      std::chrono::steady_clock::now() - start).count();
}

int pwlr_presets(void *session, pwlr_preset *out, int max) {
  const PwSession *s = (const PwSession *)session;
  if (!s || !out || max <= 0) return 0;
  const int count = std::min(s->preset_count, max);
  memcpy(out, s->presets, (size_t)count * sizeof *out);
  return count;
}

struct pwlr_means {
  double mean[3];
  unsigned long long used[3], total[3];
};

} /* extern "C" */

/* Walk complete cells once. Keep each cell's sums separate until its clipping and shadow
 * tests have seen every photosite, including both greens on a Bayer sensor. */
template <bool Mosaic>
static void pwlr_channel_means_of(const libraw_rawdata_t &R, const PwLayout &L,
                                  unsigned x, unsigned y, unsigned right, unsigned bottom,
                                  pwlr_means *out) {
  const auto &S = R.sizes;
  const unsigned cell = L.cell;
  const unsigned first_x = ((x + cell - 1) / cell) * cell;
  const unsigned first_y = ((y + cell - 1) / cell) * cell;
  const unsigned stride = Mosaic ? 1 : L.n;
  const void *base = Mosaic ? (const void *)R.raw_image :
      (R.color4_image ? (const void *)R.color4_image : (const void *)R.color3_image);
  unsigned long long sum[3] = {};
  for (unsigned cy = first_y; cy + cell <= bottom; cy += cell) {
    const ushort *rows[3];
    const unsigned char *colours[3];
    const unsigned *pattern[3];
    for (unsigned dy = 0; dy < cell; dy++) {
      const unsigned row = cy + dy;
      rows[dy] = (const ushort *)((const unsigned char *)base +
          (size_t)(row + S.top_margin) * S.raw_pitch) + (size_t)S.left_margin * stride;
      if (Mosaic) colours[dy] = L.cfa[row % L.frows];
      pattern[dy] = L.pattern ? &L.rest[(row % L.prows) * L.pcols] : nullptr;
    }
    for (unsigned cx = first_x; cx + cell <= right; cx += cell) {
      unsigned counts[3] = {}, sums[3] = {};
      bool clipped = false, bright = false;
      for (unsigned dy = 0; dy < cell; dy++) {
        for (unsigned col = cx; col < cx + cell; col++) {
          const unsigned extra = pattern[dy] ? pattern[dy][col % L.pcols] : 0;
          const ushort *pixel = rows[dy] + (size_t)col * stride;
          for (unsigned c = 0; c < (Mosaic ? 1u : 3u); c++) {
            const unsigned c4 = Mosaic ? colours[dy][col % L.fcols] : c;
            const unsigned ch = c4 == 3 ? 1 : c4;
            const PwLimits &limit = L.lim[c4];
            const unsigned black = limit.black + extra;
            const unsigned value = pixel[c];
            const unsigned above = value > black ? value - black : 0;
            clipped |= value >= limit.white;
            bright |= above >= limit.shadow;
            counts[ch]++;
            sums[ch] += above;
          }
        }
      }
      for (unsigned c = 0; c < 3; c++) out->total[c] += counts[c];
      if (!clipped && bright) {
        for (unsigned c = 0; c < 3; c++) {
          out->used[c] += counts[c];
          sum[c] += sums[c];
        }
      }
    }
  }
  for (unsigned c = 0; c < 3; c++)
    if (out->used[c]) out->mean[c] = (double)sum[c] / (double)out->used[c];
}

extern "C" {

int pwlr_channel_means(void *session, unsigned x, unsigned y, unsigned w, unsigned h,
                       pwlr_means *out, char *error) {
  *out = {};
  error[0] = 0;
  const PwSession *s = (const PwSession *)session;
  if (!s || !s->lr) { strcpy(error, "channel means: no session"); return -1; }
  if (s->spent) { strcpy(error, PWLR_SPENT_TEXT); return -1; }
  const auto &R = s->lr->imgdata.rawdata;
  const unsigned width = R.sizes.width, height = R.sizes.height;
  if (!w || !h || x > width || y > height || w > width - x || h > height - y) {
    snprintf(error, 200, "region %ux%u at %u,%u is outside the image (%ux%u)", w, h, x, y,
             width, height);
    return -1;
  }
  PwLayout layout;
  if (!pwlr_layout_of(R, &layout, error)) return 0;
  if (layout.mosaic) pwlr_channel_means_of<true>(R, layout, x, y, x + w, y + h, out);
  else pwlr_channel_means_of<false>(R, layout, x, y, x + w, y + h, out);
  return 1;
}

/* The statistics of unpacked RAW data: 1 with *out filled, 0 when the layout is not one they are
 * made for (error says which). `L` and `lv` are work space (one PwLayout, four PwLevels) and
 * `bright` has room for one byte per column of the visible image. Allocates nothing and throws
 * nothing. */
static int pwlr_statistics_of(const libraw_rawdata_t &R, PwLayout *L, PwLevels *lv,
                              unsigned char *bright, pwlr_stats *out, char *error) {
  if (!pwlr_layout_of(R, L, error)) return 0;
  const libraw_image_sizes_t &S = R.sizes;
  const libraw_colordata_t &C = R.color;
  const unsigned width = L->width, height = L->height;
  /* The camera's white balance with the smallest multiplier 1, as the render uses it; a file
   * without one gets LibRaw's daylight multipliers, else none. */
  float mul[4] = {1.0f, 1.0f, 1.0f, 1.0f};
  const float tiny = 0.00001f;
  const float *wb = 0;
  if (C.cam_mul[0] > tiny && C.cam_mul[1] > tiny && C.cam_mul[2] > tiny) wb = C.cam_mul;
  else if (C.pre_mul[0] > tiny && C.pre_mul[1] > tiny && C.pre_mul[2] > tiny) wb = C.pre_mul;
  if (wb) {
    float low = wb[0] < wb[1] ? wb[0] : wb[1];
    if (wb[2] < low) low = wb[2];
    for (int c = 0; c < 3; c++) mul[c] = wb[c] / low;
    mul[3] = (wb[3] > tiny ? wb[3] : wb[1]) / low;
  }
  pwlr_levels(L->lim, mul, lv);
  const bool pattern = L->pattern;
  const unsigned prows = L->prows, pcols = L->pcols;
  const unsigned *rest = L->rest;

  if (L->mosaic) {
    /* A mosaic: one colour per photosite. */
    const unsigned frows = L->frows, fcols = L->fcols, cell = L->cell;
    const unsigned cells_x = width / cell, cells_y = height / cell;
    const size_t pitch = S.raw_pitch / 2;
    for (unsigned row = 0; row < height; row++) {
      const ushort *src = R.raw_image + (size_t)(row + S.top_margin) * pitch + S.left_margin;
      const unsigned char *colours = L->cfa[row % frows];
      const unsigned *prow = pattern ? &rest[(row % prows) * pcols] : 0;
      const bool in_cells = row / cell < cells_y;
      if (in_cells && row % cell == 0) memset(bright, 0, cells_x);
      unsigned k = 0; /* col % fcols */
      for (unsigned col = 0; col < width; col++) {
        const unsigned c4 = colours[k];
        if (++k == fcols) k = 0;
        const PwLevels &l = lv[c4];
        const unsigned ch = c4 == 3 ? 1 : c4;
        const unsigned v = src[col];
        const unsigned black = l.black + (prow ? prow[col % pcols] : 0);
        const unsigned above = v > black ? v - black : 0;
        out->histogram[ch][l.bin[above]]++;
        if (v >= l.white) out->clipped[ch]++;
        if (above >= l.shadow && in_cells) {
          const unsigned cx = col / cell;
          if (cx < cells_x) bright[cx] = 1;
        }
      }
      if (in_cells && row % cell == cell - 1)
        for (unsigned cx = 0; cx < cells_x; cx++)
          if (!bright[cx]) out->deep_shadows += (unsigned long long)cell * cell;
    }
    out->shadow_pixels = (unsigned long long)cells_x * cells_y * cell * cell;
  } else {
    /* Three or four values per pixel (the fourth is not a colour of an RGB file). */
    const unsigned n = L->n;
    const size_t pitch = S.raw_pitch; /* bytes */
    const unsigned char *base = R.color4_image ? (const unsigned char *)R.color4_image
                                               : (const unsigned char *)R.color3_image;
    for (unsigned row = 0; row < height; row++) {
      const ushort *src = (const ushort *)(base + (size_t)(row + S.top_margin) * pitch) +
                          (size_t)S.left_margin * n;
      const unsigned *prow = pattern ? &rest[(row % prows) * pcols] : 0;
      for (unsigned col = 0; col < width; col++, src += n) {
        const unsigned extra = prow ? prow[col % pcols] : 0;
        bool dark = true;
        for (unsigned c = 0; c < 3; c++) {
          const PwLevels &l = lv[c];
          const unsigned v = src[c];
          const unsigned black = l.black + extra;
          const unsigned above = v > black ? v - black : 0;
          out->histogram[c][l.bin[above]]++;
          if (v >= l.white) out->clipped[c]++;
          if (above >= l.shadow) dark = false;
        }
        if (dark) out->deep_shadows++;
      }
    }
    out->shadow_pixels = (unsigned long long)width * height;
  }
  for (int c = 0; c < 3; c++)
    for (int b = 0; b < 256; b++) out->pixels[c] += out->histogram[c][b];
  return 1;
}

/* Returns 1 with *out filled; 0 when the file's RAW data has a layout the statistics are not made
 * for (error says which); -1 on an internal failure. It calls no LibRaw function and uses no
 * exceptions (the work buffers are allocated with nothrow). */
int pwlr_statistics(void *session, pwlr_stats *out, char *error) {
  memset(out, 0, sizeof *out);
  error[0] = 0;
  PwSession *s = (PwSession *)session;
  if (!s || !s->lr) { strcpy(error, "statistics: no session"); return -1; }
  if (s->spent) { strcpy(error, PWLR_SPENT_TEXT); return -1; }
  const libraw_rawdata_t &R = s->lr->imgdata.rawdata;
  PwLayout *layout = new (std::nothrow) PwLayout;
  PwLevels *lv = new (std::nothrow) PwLevels[4];
  unsigned char *bright = new (std::nothrow) unsigned char[(size_t)R.sizes.width + 1];
  int result = -1;
  if (layout && lv && bright) result = pwlr_statistics_of(R, layout, lv, bright, out, error);
  else strcpy(error, "statistics: out of memory");
  delete layout;
  delete[] lv;
  delete[] bright;
  return result;
}

/* ---- The clipping map of the unpacked RAW data (ADR 0017 section 2.6, U7b; T146) ----
 *
 * Where T142's clipped and deep-shadow pixels are: the visible image (sensor orientation, no flip
 * applied) in cells of cell x cell photosites (2 for a Bayer pattern and a three- or four-colour
 * image, 3 for X-Trans), counted from its top left corner. The map has ceil(width / cell) x
 * ceil(height / cell) cells, one byte each, row by row; a cell at the right or bottom edge is
 * judged on the photosites it holds. Same data, levels and layout checks as the statistics:
 * - bit 0, 1, 2 (PWLR_MAP_CLIPPED << channel): at least one photosite of R, G, B (both greens of
 *   a mosaic are G) in the cell has a raw value at or above the white level, before white
 *   balance;
 * - bit 3 (PWLR_MAP_DEEP): every photosite of the cell, in every channel, is below the
 *   deep-shadow limit above black;
 * - bits 4-7: 0. */
static const unsigned char PWLR_MAP_CLIPPED = 1;
static const unsigned char PWLR_MAP_DEEP = 8;

/* Fills `map` (cells_x x cells_y bytes, already checked) from the RAW data. The map's own row is
 * the work space of a row of cells: it starts as "deep shadows, nothing clipped" and every
 * photosite of the cell can only add a clipped bit or clear the deep-shadow bit. */
static void pwlr_clipping_map_of(const libraw_rawdata_t &R, const PwLayout &L, unsigned cells_x,
                                 unsigned char *map) {
  const libraw_image_sizes_t &S = R.sizes;
  const unsigned width = L.width, height = L.height, cell = L.cell;
  const unsigned *prow = 0;
  if (L.mosaic) {
    const size_t pitch = S.raw_pitch / 2;
    for (unsigned row = 0; row < height; row++) {
      const ushort *src = R.raw_image + (size_t)(row + S.top_margin) * pitch + S.left_margin;
      const unsigned char *colours = L.cfa[row % L.frows];
      prow = L.pattern ? &L.rest[(row % L.prows) * L.pcols] : 0;
      unsigned char *cells = map + (size_t)(row / cell) * cells_x;
      if (row % cell == 0) memset(cells, PWLR_MAP_DEEP, cells_x);
      unsigned k = 0;    /* col % fcols */
      unsigned cx = 0;   /* col / cell */
      unsigned in = 0;   /* col % cell */
      for (unsigned col = 0; col < width; col++) {
        const unsigned c4 = colours[k];
        if (++k == L.fcols) k = 0;
        const PwLimits &l = L.lim[c4];
        const unsigned ch = c4 == 3 ? 1 : c4;
        const unsigned v = src[col];
        const unsigned black = l.black + (prow ? prow[col % L.pcols] : 0);
        const unsigned above = v > black ? v - black : 0;
        unsigned char m = cells[cx];
        if (v >= l.white) m = (unsigned char)(m | (PWLR_MAP_CLIPPED << ch));
        if (above >= l.shadow) m = (unsigned char)(m & ~PWLR_MAP_DEEP);
        cells[cx] = m;
        if (++in == cell) { in = 0; cx++; }
      }
    }
  } else {
    const unsigned n = L.n;
    const size_t pitch = S.raw_pitch; /* bytes */
    const unsigned char *base = R.color4_image ? (const unsigned char *)R.color4_image
                                               : (const unsigned char *)R.color3_image;
    for (unsigned row = 0; row < height; row++) {
      const ushort *src = (const ushort *)(base + (size_t)(row + S.top_margin) * pitch) +
                          (size_t)S.left_margin * n;
      prow = L.pattern ? &L.rest[(row % L.prows) * L.pcols] : 0;
      unsigned char *cells = map + (size_t)(row / cell) * cells_x;
      if (row % cell == 0) memset(cells, PWLR_MAP_DEEP, cells_x);
      unsigned cx = 0, in = 0;
      for (unsigned col = 0; col < width; col++, src += n) {
        const unsigned extra = prow ? prow[col % L.pcols] : 0;
        unsigned char m = cells[cx];
        for (unsigned c = 0; c < 3; c++) {
          const PwLimits &l = L.lim[c];
          const unsigned v = src[c];
          const unsigned black = l.black + extra;
          const unsigned above = v > black ? v - black : 0;
          if (v >= l.white) m = (unsigned char)(m | (PWLR_MAP_CLIPPED << c));
          if (above >= l.shadow) m = (unsigned char)(m & ~PWLR_MAP_DEEP);
        }
        cells[cx] = m;
        if (++in == cell) { in = 0; cx++; }
      }
    }
  }
}

/* The clipping map, in two calls. With map == NULL: the layout checks only, and *cell, *width and
 * *height (the map's size in cells) are set. With a buffer: the same, then `length` must be
 * exactly *width x *height (else -1 and nothing is written to the buffer) and the buffer is
 * filled. Returns 1 on success; 0 when the file's RAW data has a layout the map is not made for
 * (error says which, as pwlr_statistics); -1 on an internal failure. It calls no LibRaw function,
 * uses no exceptions (the work space is allocated with nothrow) and keeps no pointer to the
 * buffer. */
int pwlr_clipping_map(void *session, unsigned char *map, size_t length, unsigned *cell,
                      unsigned *width, unsigned *height, char *error) {
  *cell = 0;
  *width = 0;
  *height = 0;
  error[0] = 0;
  PwSession *s = (PwSession *)session;
  if (!s || !s->lr) { strcpy(error, "clipping map: no session"); return -1; }
  if (s->spent) { strcpy(error, PWLR_SPENT_TEXT); return -1; }
  const libraw_rawdata_t &R = s->lr->imgdata.rawdata;
  PwLayout *layout = new (std::nothrow) PwLayout;
  if (!layout) { strcpy(error, "clipping map: out of memory"); return -1; }
  int result = pwlr_layout_of(R, layout, error);
  if (result == 1) {
    const unsigned c = layout->cell;
    const unsigned cells_x = (layout->width + c - 1) / c;
    const unsigned cells_y = (layout->height + c - 1) / c;
    *cell = c;
    *width = cells_x;
    *height = cells_y;
    if (map) {
      if (length != (size_t)cells_x * cells_y) {
        strcpy(error, "clipping map: the buffer's length is not the map's size");
        result = -1;
      } else {
        pwlr_clipping_map_of(R, *layout, cells_x, map);
      }
    }
  }
  delete layout;
  return result;
}

/* 1 when a failed or stopped render made LibRaw free the unpacked RAW data (see pwlr_render). */
int pwlr_spent(void *session) {
  PwSession *s = (PwSession *)session;
  return s && s->spent ? 1 : 0;
}

void pwlr_close(void *session) {
  PwSession *s = (PwSession *)session;
  if (!s) return;
  delete s->lr; /* recycles; does not delete the stream (it is ours) */
  delete s->stream;
  delete s;
}

} /* extern "C" */

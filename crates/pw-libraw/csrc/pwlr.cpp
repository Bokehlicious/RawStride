/* Thin C shim around LibRaw for pw-libraw (ADR 0006).
 *
 * All decodes use the same fixed settings, chosen so that a region is pixel-identical to the
 * same area of a whole-image decode (docs/analyses/2026-11-04-m2-libraw-decode.md, follow-ups
 * 3 and 4): camera white balance, no auto white balance, no auto brightness, no white-level
 * adjustment from the image data, sensor orientation, 8-bit sRGB. The demosaic method is chosen per
 * render: AHD (the default, ADR 0006) or PPG (ADR 0008 section 11; the egui loupe, ADR 0009).
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
#include <atomic>
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

struct PwSession {
  LibRaw *lr;
  PwStream *stream;
  int data_errors;
};

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
  char error[200];
} pwlr_info;

static void set_error(char *dst, const char *what, int rc) {
  snprintf(dst, 200, "%s: %s", what, libraw_strerror(rc));
}

/* LibRaw calls this for corrupt data (offset = position) or an unexpected end (offset = -1).
 * Counting it also keeps LibRaw from printing to stderr. */
static void on_data_error(void *data, const char *, const INT64) {
  PwSession *s = (PwSession *)data;
  if (s) s->data_errors++;
}

void pwlr_close(void *session);

/* Opens and unpacks; NULL on failure (info->error says why; "damaged: ..." for missing or
 * corrupt RAW data). */
void *pwlr_open(const wchar_t *path, pwlr_info *info) {
  memset(info, 0, sizeof *info);
  PwSession *s = new PwSession{nullptr, nullptr, 0};
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
  return s;
}

/* LibRaw user_qual values the shim accepts. */
static const int PWLR_QUAL_PPG = 2;
static const int PWLR_QUAL_AHD = 3;

/* Processes the region (x, y, w, h) of the visible image, or the whole image if w == 0, with the
 * demosaic method `quality` (LibRaw user_qual: 2 PPG or 3 AHD; anything else is refused).
 * half != 0 gives half size (2x2 binning, whole image only; no demosaicing). Returns a LibRaw
 * image that the caller frees with pwlr_free; NULL on failure. */
libraw_processed_image_t *pwlr_render(void *session, int quality, int half, int x, int y, int w,
                                      int h, char *error) {
  if (quality != PWLR_QUAL_PPG && quality != PWLR_QUAL_AHD) {
    snprintf(error, 200, "unsupported demosaic method %d", quality);
    return 0;
  }
  LibRaw *lr = ((PwSession *)session)->lr;
  lr->imgdata.params.user_qual = quality;
  lr->imgdata.params.half_size = half ? 1 : 0;
  if (w > 0 && h > 0) {
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
  int rc = lr->dcraw_process();
  if (rc != LIBRAW_SUCCESS) { set_error(error, "process", rc); return 0; }
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
 * `common`, the part every value of a black level pattern shares), white, the deep-shadow limit
 * (a value after black subtraction) and the histogram bin of every value after black
 * subtraction. */
struct PwLevels {
  unsigned black, white, shadow;
  unsigned char bin[65536];
};

static void pwlr_levels(const libraw_colordata_t &C, unsigned common, const float mul[4],
                        PwLevels *lv) {
  for (int c = 0; c < 4; c++) {
    PwLevels &l = lv[c];
    l.black = C.black + C.cblack[c] + common;
    l.white = C.linear_max[c] ? C.linear_max[c] : C.maximum;
    if (l.white > 65535u) l.white = 65535u;
    double span = l.white > l.black ? (double)(l.white - l.black) : 1.0;
    /* "below 1/256 of the white level above black" */
    l.shadow = (unsigned)ceil(span / 256.0);
    for (unsigned v = 0; v < 65536u; v++) {
      int b = (int)(pwlr_curve((double)v / span * (double)mul[c]) * 256.0);
      l.bin[v] = (unsigned char)(b > 255 ? 255 : b);
    }
  }
}

/* The statistics of unpacked RAW data: 1 with *out filled, 0 when the layout is not one they are
 * made for (error says which). `lv` has room for four PwLevels and `bright` for one byte per
 * column of the visible image. Allocates nothing and throws nothing. */
static int pwlr_statistics_of(const libraw_rawdata_t &R, PwLevels *lv, unsigned char *bright,
                              pwlr_stats *out, char *error) {
  const libraw_image_sizes_t &S = R.sizes;
  const libraw_iparams_t &P = R.iparams;
  const libraw_colordata_t &C = R.color;
  const unsigned width = S.width, height = S.height;
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
  /* A black level pattern on top of the per-channel level (cblack[4] x cblack[5] values). The
   * part all its values share belongs to the black level itself (so the white level above black
   * is right); `rest` is what is left per position, and `pattern` whether any is. */
  const unsigned prows = C.cblack[4], pcols = C.cblack[5];
  if ((prows || pcols) && (!prows || !pcols || prows > LIBRAW_CBLACK_SIZE ||
                           pcols > LIBRAW_CBLACK_SIZE || prows * pcols > LIBRAW_CBLACK_SIZE - 6)) {
    strcpy(error, "unknown black level pattern");
    return 0;
  }
  unsigned rest[LIBRAW_CBLACK_SIZE];
  unsigned common = 0;
  bool pattern = false;
  if (prows && pcols) {
    common = C.cblack[6];
    for (unsigned i = 1; i < prows * pcols; i++)
      if (C.cblack[6 + i] < common) common = C.cblack[6 + i];
    for (unsigned i = 0; i < prows * pcols; i++) {
      rest[i] = C.cblack[6 + i] - common;
      if (rest[i]) pattern = true;
    }
  }
  pwlr_levels(C, common, mul, lv);

  if (R.raw_image) {
    /* A mosaic: one colour per photosite. */
    if (S.raw_pitch < (unsigned)S.raw_width * 2u) { strcpy(error, "short RAW rows"); return 0; }
    unsigned char cfa[8][6]; /* the colour (0..3) of row % frows, column % fcols */
    unsigned frows, fcols, cell;
    if (P.filters >= 1000) {
      frows = 8; fcols = 2; cell = 2;
      for (unsigned r = 0; r < 8; r++)
        for (unsigned c = 0; c < 2; c++)
          cfa[r][c] = (unsigned char)((P.filters >> ((((r << 1) & 14) | (c & 1)) << 1)) & 3);
    } else if (P.filters == 9) {
      frows = 6; fcols = 6; cell = 3;
      for (unsigned r = 0; r < 6; r++)
        for (unsigned c = 0; c < 6; c++) {
          int x = P.xtrans[r][c];
          if (x < 0 || x > 2) { strcpy(error, "unknown X-Trans pattern"); return 0; }
          cfa[r][c] = (unsigned char)x;
        }
    } else {
      strcpy(error, "not a Bayer or X-Trans pattern");
      return 0;
    }
    const unsigned cells_x = width / cell, cells_y = height / cell;
    const size_t pitch = S.raw_pitch / 2;
    for (unsigned row = 0; row < height; row++) {
      const ushort *src = R.raw_image + (size_t)(row + S.top_margin) * pitch + S.left_margin;
      const unsigned char *colours = cfa[row % frows];
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
  } else if (R.color3_image || R.color4_image) {
    /* Three or four values per pixel (the fourth is not a colour of an RGB file). */
    const unsigned n = R.color4_image ? 4 : 3;
    const size_t pitch = S.raw_pitch; /* bytes */
    if (pitch < (size_t)((unsigned)S.left_margin + width) * n * sizeof(ushort)) {
      strcpy(error, "short RAW rows");
      return 0;
    }
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
  } else {
    strcpy(error, "no integer RAW data");
    return 0;
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
  const libraw_rawdata_t &R = s->lr->imgdata.rawdata;
  PwLevels *lv = new (std::nothrow) PwLevels[4];
  unsigned char *bright = new (std::nothrow) unsigned char[(size_t)R.sizes.width + 1];
  int result = -1;
  if (lv && bright) result = pwlr_statistics_of(R, lv, bright, out, error);
  else strcpy(error, "statistics: out of memory");
  delete[] lv;
  delete[] bright;
  return result;
}

void pwlr_close(void *session) {
  PwSession *s = (PwSession *)session;
  if (!s) return;
  delete s->lr; /* recycles; does not delete the stream (it is ours) */
  delete s->stream;
  delete s;
}

} /* extern "C" */

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

void pwlr_close(void *session) {
  PwSession *s = (PwSession *)session;
  if (!s) return;
  delete s->lr; /* recycles; does not delete the stream (it is ours) */
  delete s->stream;
  delete s;
}

} /* extern "C" */

/* Prototype shim for P4b (ADR 0008 §8): LibRaw with the decode settings of ADR 0006 §2, but with a
 * selectable demosaic method (user_qual) and half-size, rendering the whole image or a region.
 * Separate from the app's pw-libraw shim; nothing in the app uses this file. */
#define LIBRAW_NODLL
#include <libraw/libraw.h>
#include <string.h>
#include <windows.h>

static double now_ms() {
  LARGE_INTEGER f, c;
  QueryPerformanceFrequency(&f);
  QueryPerformanceCounter(&c);
  return (double)c.QuadPart * 1000.0 / (double)f.QuadPart;
}

extern "C" {

typedef struct {
  double open_ms, unpack_ms;
  int width, height;  /* visible image, sensor orientation */
  int flip;
  char error[200];
} pp_info;

typedef struct {
  double process_ms, mkimg_ms;
  int width, height;
  char error[200];
} pp_render_info;

const char *pp_version(void) { return libraw_version(); }

void *pp_open(const wchar_t *path, pp_info *info) {
  memset(info, 0, sizeof *info);
  LibRaw *lr = new LibRaw(0);
  lr->imgdata.params.use_camera_wb = 1;
  lr->imgdata.params.use_auto_wb = 0;
  lr->imgdata.params.no_auto_bright = 1;
  lr->imgdata.params.bright = 1.0f;
  lr->imgdata.params.adjust_maximum_thr = 0.0f;
  lr->imgdata.params.user_flip = 0;
  lr->imgdata.params.output_bps = 8;
  lr->imgdata.params.output_color = 1; /* sRGB */
  lr->imgdata.params.highlight = 0;
  double t0 = now_ms();
  int rc = lr->open_file(path);
  double t1 = now_ms();
  info->open_ms = t1 - t0;
  if (rc != LIBRAW_SUCCESS) {
    strncpy(info->error, libraw_strerror(rc), sizeof info->error - 1);
    delete lr;
    return 0;
  }
  info->width = lr->imgdata.sizes.width;
  info->height = lr->imgdata.sizes.height;
  info->flip = lr->imgdata.sizes.flip;
  rc = lr->unpack();
  info->unpack_ms = now_ms() - t1;
  if (rc != LIBRAW_SUCCESS) {
    strncpy(info->error, libraw_strerror(rc), sizeof info->error - 1);
    delete lr;
    return 0;
  }
  return lr;
}

/* quality = LibRaw user_qual (0 linear, 1 VNG, 2 PPG, 3 AHD, 4 DCB, 11 DHT, 12 AAHD).
 * half != 0: half size (whole image only). w == 0: whole image, else the region (x, y, w, h). */
libraw_processed_image_t *pp_render(void *s, int quality, int half, int x, int y, int w, int h,
                                    pp_render_info *ri) {
  memset(ri, 0, sizeof *ri);
  LibRaw *lr = (LibRaw *)s;
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
  double t0 = now_ms();
  int rc = lr->dcraw_process();
  double t1 = now_ms();
  ri->process_ms = t1 - t0;
  if (rc != LIBRAW_SUCCESS) {
    strncpy(ri->error, libraw_strerror(rc), sizeof ri->error - 1);
    return 0;
  }
  libraw_processed_image_t *img = lr->dcraw_make_mem_image(&rc);
  ri->mkimg_ms = now_ms() - t1;
  lr->free_image();
  if (!img) {
    strncpy(ri->error, libraw_strerror(rc), sizeof ri->error - 1);
    return 0;
  }
  if (img->type != LIBRAW_IMAGE_BITMAP || img->colors != 3 || img->bits != 8) {
    LibRaw::dcraw_clear_mem(img);
    strcpy(ri->error, "unexpected output format");
    return 0;
  }
  ri->width = img->width;
  ri->height = img->height;
  return img;
}

void pp_image(const libraw_processed_image_t *img, const unsigned char **data, unsigned *size) {
  *data = img->data;
  *size = img->data_size;
}

void pp_free(libraw_processed_image_t *img) { LibRaw::dcraw_clear_mem(img); }

void pp_close(void *s) { delete (LibRaw *)s; }

} /* extern "C" */

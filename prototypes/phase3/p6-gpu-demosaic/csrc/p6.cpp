/* P6 shim (ADR 0008): LibRaw with the decode settings of ADR 0006 section 2, opened and unpacked only.
 * Exposes the visible Bayer data and the parameters dcraw_process would use, so the GPU can do
 * scaling, demosaicing, colour conversion and the output curve itself. Not part of the app. */
#define LIBRAW_NODLL
#include <libraw/libraw.h>
#include <string.h>
#include <windows.h>

static double p6_now_ms() {
  LARGE_INTEGER f, c;
  QueryPerformanceFrequency(&f);
  QueryPerformanceCounter(&c);
  return (double)c.QuadPart * 1000.0 / (double)f.QuadPart;
}

extern "C" {

typedef struct {
  double open_ms, unpack_ms;
  int width, height;           /* visible image, sensor orientation */
  int raw_width, raw_height, top_margin, left_margin;
  int flip;
  unsigned filters;            /* dcraw CFA code (index 3 = second green) */
  int colors;
  float black[4];              /* total black level per CFA index (as after adjust_bl) */
  float scale_mul[4];          /* per CFA index, as in scale_colors with camera WB, highlight 0 */
  float rgb_cam[3][3];         /* camera -> sRGB (output_color 1) */
  float maximum;               /* maximum - common black */
  char error[200];
} p6_info;

void *p6_open(const wchar_t *path, p6_info *info) {
  memset(info, 0, sizeof *info);
  LibRaw *lr = new LibRaw(0);
  lr->imgdata.params.use_camera_wb = 1;
  lr->imgdata.params.use_auto_wb = 0;
  lr->imgdata.params.no_auto_bright = 1;
  lr->imgdata.params.bright = 1.0f;
  lr->imgdata.params.adjust_maximum_thr = 0.0f;
  lr->imgdata.params.user_flip = 0;
  lr->imgdata.params.output_bps = 8;
  lr->imgdata.params.output_color = 1;
  lr->imgdata.params.highlight = 0;
  double t0 = p6_now_ms();
  int rc = lr->open_file(path);
  double t1 = p6_now_ms();
  info->open_ms = t1 - t0;
  if (rc != LIBRAW_SUCCESS) {
    strncpy(info->error, libraw_strerror(rc), sizeof info->error - 1);
    delete lr;
    return 0;
  }
  rc = lr->unpack();
  info->unpack_ms = p6_now_ms() - t1;
  if (rc != LIBRAW_SUCCESS || !lr->imgdata.rawdata.raw_image) {
    strncpy(info->error, rc != LIBRAW_SUCCESS ? libraw_strerror(rc) : "not a Bayer raw", sizeof info->error - 1);
    delete lr;
    return 0;
  }
  libraw_data_t &d = lr->imgdata;
  info->width = d.sizes.width;
  info->height = d.sizes.height;
  info->raw_width = d.sizes.raw_width;
  info->raw_height = d.sizes.raw_height;
  info->top_margin = d.sizes.top_margin;
  info->left_margin = d.sizes.left_margin;
  info->flip = d.sizes.flip;
  info->filters = d.idata.filters;
  info->colors = d.idata.colors;

  /* Black levels as LibRaw's adjust_bl(): a 1x1/2x2 pattern is folded into cblack[0..3], then the
   * common part moves into black. */
  unsigned cb[10]; /* cblack[0..3], pattern size [4],[5], and a pattern of at most 2x2 at [6..9] */
  memcpy(cb, d.color.cblack, sizeof cb);
  unsigned black = d.color.black;
  if (d.idata.filters > 1000 && (cb[4] + 1) / 2 == 1 && (cb[5] + 1) / 2 == 1) {
    for (int c = 0; c < 4; c++) {
      int row = c / 2, col = c % 2;
      int clr = (d.idata.filters >> ((((row << 1) & 14) | (col & 1)) << 1)) & 3;
      if (cb[4] && cb[5]) cb[clr] += cb[6 + (row % cb[4]) * cb[5] + (col % cb[5])];
    }
    cb[4] = cb[5] = 0;
  }
  unsigned m = cb[3];
  for (int c = 0; c < 3; c++) if (m > cb[c]) m = cb[c];
  for (int c = 0; c < 4; c++) cb[c] -= m;
  black += m;
  for (int c = 0; c < 4; c++) info->black[c] = (float)(black + cb[c]);
  float maximum = (float)d.color.maximum - (float)black;
  info->maximum = maximum;

  /* scale_colors with use_camera_wb and highlight 0 */
  double pre[4];
  for (int c = 0; c < 4; c++) pre[c] = d.color.pre_mul[c];
  if (d.color.cam_mul[0] > 0) {
    double sum = 0;
    for (int c = 0; c < 4; c++) sum += d.color.cam_mul[c];
    if (sum > 0) for (int c = 0; c < 4; c++) pre[c] = d.color.cam_mul[c];
  }
  if (pre[1] == 0) pre[1] = 1;
  if (pre[3] == 0) pre[3] = d.idata.colors < 4 ? pre[1] : 1;
  double dmin = 1e30;
  for (int c = 0; c < 4; c++) if (pre[c] > 0 && pre[c] < dmin) dmin = pre[c];
  for (int c = 0; c < 4; c++) info->scale_mul[c] = (float)(pre[c] / dmin * 65535.0 / maximum);
  for (int i = 0; i < 3; i++)
    for (int j = 0; j < 3; j++) info->rgb_cam[i][j] = d.color.rgb_cam[i][j];
  return lr;
}

/* Copies the visible area (width x height, row-major, 16-bit) into dst. */
int p6_copy_raw(void *s, unsigned short *dst) {
  LibRaw *lr = (LibRaw *)s;
  libraw_data_t &d = lr->imgdata;
  const unsigned short *src = d.rawdata.raw_image;
  int pitch = d.sizes.raw_pitch / 2;
  for (int r = 0; r < d.sizes.height; r++)
    memcpy(dst + (size_t)r * d.sizes.width, src + (size_t)(r + d.sizes.top_margin) * pitch + d.sizes.left_margin,
           (size_t)d.sizes.width * 2);
  return 0;
}

void p6_close(void *s) { delete (LibRaw *)s; }

} /* extern "C" */

#ifndef JUSTPDF_H
#define JUSTPDF_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Error codes */
#define JUSTPDF_OK            0
#define JUSTPDF_ERR_NULL_PTR  (-1)
#define JUSTPDF_ERR_INVALID_PATH (-2)
#define JUSTPDF_ERR_PARSE     (-3)
#define JUSTPDF_ERR_RENDER    (-4)
#define JUSTPDF_ERR_OUT_OF_RANGE (-5)
#define JUSTPDF_ERR_ENCRYPTED (-6)
#define JUSTPDF_ERR_IO        (-7)

/* Opaque types */
typedef struct JustPdfDocument JustPdfDocument;
typedef struct JustPdfImage JustPdfImage;

/* Document lifecycle */
int justpdf_open(const char *path, JustPdfDocument **out);
int justpdf_open_memory(const uint8_t *data, size_t len, JustPdfDocument **out);
void justpdf_close(JustPdfDocument *doc);
int justpdf_authenticate(JustPdfDocument *doc, const char *password);

/* Document info */
int justpdf_page_count(const JustPdfDocument *doc, unsigned int *out);
int justpdf_version(const JustPdfDocument *doc, uint8_t *major, uint8_t *minor);
int justpdf_is_encrypted(const JustPdfDocument *doc, int *out);

/* Text extraction */
int justpdf_extract_page_text(const JustPdfDocument *doc, unsigned int page_index, char **out);
int justpdf_extract_all_text(const JustPdfDocument *doc, char **out);
void justpdf_free_string(char *s);

/* Rendering */
int justpdf_render_page_png(const JustPdfDocument *doc, unsigned int page_index, double dpi, JustPdfImage **out);
int justpdf_image_data(const JustPdfImage *img, const uint8_t **data_out, size_t *len_out);
int justpdf_image_save(const JustPdfImage *img, const char *path);
void justpdf_free_image(JustPdfImage *img);

/* Page info */
int justpdf_page_size(const JustPdfDocument *doc, unsigned int page_index, double *width, double *height);

#ifdef __cplusplus
}
#endif

#endif /* JUSTPDF_H */

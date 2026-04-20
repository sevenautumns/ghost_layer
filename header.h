#pragma once
#include <stdint.h>
#include <stddef.h>

typedef struct {
    uint8_t* data;
    size_t len;
    size_t capacity;
} PdfBuffer;

typedef struct {
    const uint8_t* img_ptr;
    size_t         img_len;
    uint32_t       width_px;
    uint32_t       height_px;
    double         dpi;
} GhostLayerImagePage;

// Returns a pointer to the last error message, or NULL if no error occurred.
// The pointer is valid until the next FFI call on this thread.
const char* pdf_get_last_error(void);

// Frees a PdfBuffer returned by any API function.
void free_pdf_buffer(PdfBuffer buf);

// N images + optional OCR JSON per page → new PDF.
// json_array is an array of page_count C-strings; NULL entry = no OCR for that page.
// Returns {NULL, 0, 0} on error. Check pdf_get_last_error() on failure.
PdfBuffer generate_pdf_from_images(const GhostLayerImagePage* pages, const char** json_array, int page_count);

// PDF + OCR JSON array → PDF with invisible text layer (in-place overlay).
// json_array is an array of page_count C-strings (one per page).
// A NULL entry skips OCR for that page and copies it unchanged.
// Returns {NULL, 0, 0} on error. Check pdf_get_last_error() on failure.
PdfBuffer pdf_ocr_document(const uint8_t* pdf_ptr, size_t pdf_len, const char** json_array, int page_count);

typedef struct GhostLayerDoc GhostLayerDoc;

GhostLayerDoc* ghost_layer_doc_new_images(void);
void ghost_layer_doc_add_image_page(GhostLayerDoc* doc, const uint8_t* img_ptr, size_t img_len, uint32_t width_px, uint32_t height_px, double dpi, const char* json_ptr);
PdfBuffer ghost_layer_doc_finish_images(GhostLayerDoc* doc);

GhostLayerDoc* ghost_layer_doc_new_ocr(void);
void ghost_layer_doc_add_ocr_page(GhostLayerDoc* doc, const char* json_ptr);
PdfBuffer ghost_layer_doc_finish_ocr(GhostLayerDoc* doc, const uint8_t* pdf_ptr, size_t pdf_len);

void ghost_layer_doc_free(GhostLayerDoc* doc);

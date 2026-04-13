#pragma once
#include <stdint.h>
#include <stddef.h>

typedef struct {
    uint8_t* data;
    size_t len;
    size_t capacity;
} PdfBuffer;

// Returns a pointer to the last error message, or NULL if no error occurred.
// The pointer is valid until the next FFI call on this thread.
const char* pdf_get_last_error(void);

// Single-page convenience API.
// Returns {NULL, 0} on error. Caller must free with free_pdf_buffer().
PdfBuffer generate_pdf_from_ocr(const uint8_t* img_ptr, size_t img_len, uint32_t width_px, uint32_t height_px, double dpi, const char* json_ptr);

// Frees a PdfBuffer returned by generate_pdf_from_ocr or pdf_builder_finalize.
void free_pdf_buffer(PdfBuffer buf);

// Multi-page builder API.
// pdf_builder_new: caller owns the returned pointer; free with pdf_builder_free OR pdf_builder_finalize.
void*     pdf_builder_new(void);
// pdf_builder_add_page: returns 1 on success, 0 on error.
int       pdf_builder_add_page(void* builder, const uint8_t* img_ptr, size_t img_len, uint32_t width_px, uint32_t height_px, double dpi, const char* json_ptr);
// pdf_builder_finalize: consumes the builder (do not call pdf_builder_free after).
// Returns {NULL, 0} on error. Caller must free result with free_pdf_buffer().
PdfBuffer pdf_builder_finalize(void* builder);
// pdf_builder_free: frees builder without producing a PDF. No-op if builder is NULL.
void      pdf_builder_free(void* builder);

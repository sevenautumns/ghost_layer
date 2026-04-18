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

// Image + OCR JSON → single-page PDF.
// Returns {NULL, 0, 0} on error. Caller must free with free_pdf_buffer().
PdfBuffer generate_pdf_from_ocr(const uint8_t* img_ptr, size_t img_len, uint32_t width_px, uint32_t height_px, double dpi, const char* json_ptr);

// Frees a PdfBuffer returned by any API function.
void free_pdf_buffer(PdfBuffer buf);

// PDF + OCR JSON array → PDF with invisible text layer (in-place overlay).
// json_array is an array of page_count C-strings (one per page).
// A NULL entry skips OCR for that page and copies it unchanged.
// Returns the finished PDF. Caller must free with free_pdf_buffer().
// Returns {NULL, 0, 0} on error. Check pdf_get_last_error() on failure.
PdfBuffer pdf_ocr_document(const uint8_t* pdf_ptr, size_t pdf_len, const char** json_array, int page_count);

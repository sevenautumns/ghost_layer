use ghostlayer::{free_pdf_buffer, generate_pdf_from_ocr, pdf_ocr_document};
use std::ffi::CString;
use std::fs;
use std::path::Path;

#[test]
fn test_ffi_interface() {
    let json_path = Path::new("tests/en_ltr.json");
    let png_path = Path::new("tests/en_ltr.png");
    let output_path = Path::new("tests/output_ffi.pdf");

    let json_content = fs::read_to_string(json_path).expect("Read JSON");
    let img_bytes = fs::read(png_path).expect("Read PNG");

    let c_json = CString::new(json_content).expect("CString conversion");

    let img_reader = image::load_from_memory(&img_bytes).expect("Load image for dimensions");
    let (width, height) = (img_reader.width(), img_reader.height());

    let pdf_buffer = unsafe {
        generate_pdf_from_ocr(
            img_bytes.as_ptr(),
            img_bytes.len(),
            width,
            height,
            300.0,
            c_json.as_ptr(),
        )
    };

    assert!(!pdf_buffer.data.is_null(), "PDF data pointer is null");
    assert!(pdf_buffer.len > 0, "PDF length is 0");

    let result_slice = unsafe { std::slice::from_raw_parts(pdf_buffer.data, pdf_buffer.len) };
    fs::write(output_path, result_slice).expect("Write PDF");

    free_pdf_buffer(pdf_buffer);
}

#[test]
fn test_pdf_ocr_document_convenience() {
    let img_bytes = fs::read("tests/en_ltr.png").expect("Read PNG");
    let json_content = fs::read_to_string("tests/en_ltr.json").expect("Read JSON");

    let img_reader = image::load_from_memory(&img_bytes).expect("Load image");
    let (width, height) = (img_reader.width(), img_reader.height());

    // Build a 2-page source PDF.
    let c_json = CString::new(json_content.clone()).unwrap();
    let page1 = unsafe {
        generate_pdf_from_ocr(
            img_bytes.as_ptr(),
            img_bytes.len(),
            width,
            height,
            300.0,
            c_json.as_ptr(),
        )
    };
    assert!(!page1.data.is_null());

    // pdf_ocr_document needs a multi-page PDF as input, so build one by calling
    // generate_pdf_from_ocr twice and concatenating — instead we just use a
    // single-page PDF and pass a 1-element JSON array.
    let source_pdf = unsafe { std::slice::from_raw_parts(page1.data, page1.len) };

    let json0 = CString::new(json_content).expect("CString");
    let jsons: [*const std::ffi::c_char; 1] = [json0.as_ptr()];

    let out_buf =
        unsafe { pdf_ocr_document(source_pdf.as_ptr(), source_pdf.len(), jsons.as_ptr(), 1) };
    assert!(!out_buf.data.is_null(), "pdf_ocr_document returned null");
    assert!(out_buf.len > 0);

    let result = unsafe { std::slice::from_raw_parts(out_buf.data, out_buf.len) };
    fs::write("tests/output_ocr_document.pdf", result).expect("Write");

    free_pdf_buffer(out_buf);
    free_pdf_buffer(page1);

    // NULL input — should return null without crashing.
    let null_buf = unsafe { pdf_ocr_document(std::ptr::null(), 0, std::ptr::null(), 0) };
    assert!(null_buf.data.is_null());
}

use ghostlayer::{generate_pdf_from_ocr, free_pdf_buffer, pdf_builder_new, pdf_builder_add_page, pdf_builder_finalize, pdf_builder_free};
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

    let pdf_buffer = generate_pdf_from_ocr(
        img_bytes.as_ptr(),
        img_bytes.len(),
        width,
        height,
        300.0,
        c_json.as_ptr()
    );

    assert!(!pdf_buffer.data.is_null(), "PDF data pointer is null");
    assert!(pdf_buffer.len > 0, "PDF length is 0");

    let result_slice = unsafe {
        std::slice::from_raw_parts(pdf_buffer.data, pdf_buffer.len)
    };
    fs::write(output_path, result_slice).expect("Write PDF");

    free_pdf_buffer(pdf_buffer);
}

#[test]
fn test_multipage_builder() {
    let output_path = Path::new("tests/output_multipage.pdf");

    let pages = [
        ("tests/en_ltr.png",  "tests/en_ltr.json"),
        ("tests/jp_ttb.png",  "tests/jp_ttb.json"),
        ("tests/jp_ltr.jpg",  "tests/jp_ltr.json"),
        ("tests/ar_rtl.jpg",  "tests/ar_rtl.json"),
    ];

    let builder = pdf_builder_new();
    assert!(!builder.is_null(), "Builder is null");

    for (png, json) in &pages {
        let img_bytes = fs::read(png).expect("Read PNG");
        let json_content = fs::read_to_string(json).expect("Read JSON");
        let c_json = CString::new(json_content).expect("CString conversion");

        let img_reader = image::load_from_memory(&img_bytes).expect("Load image for dimensions");
        let (width, height) = (img_reader.width(), img_reader.height());

        let ok = pdf_builder_add_page(builder, img_bytes.as_ptr(), img_bytes.len(), width, height, 300.0, c_json.as_ptr());
        assert_eq!(ok, 1, "add_page failed for {png}");
    }

    let pdf_buffer = pdf_builder_finalize(builder);
    assert!(!pdf_buffer.data.is_null(), "Multipage PDF data is null");
    assert!(pdf_buffer.len > 0, "Multipage PDF length is 0");

    let result_slice = unsafe {
        std::slice::from_raw_parts(pdf_buffer.data, pdf_buffer.len)
    };
    fs::write(output_path, result_slice).expect("Write multipage PDF");

    // Four-page PDF must be larger than a single-page fixture
    let first_page_bytes = fs::read(pages[0].0).expect("Read first PNG");
    assert!(pdf_buffer.len > first_page_bytes.len(), "Multipage PDF not larger than first image");

    free_pdf_buffer(pdf_buffer);
}

#[test]
fn test_builder_free_without_finalize() {
    let img_bytes = fs::read("tests/en_ltr.png").expect("Read PNG");
    let json_content = fs::read_to_string("tests/en_ltr.json").expect("Read JSON");
    let c_json = CString::new(json_content).expect("CString conversion");

    let img_reader = image::load_from_memory(&img_bytes).expect("Load image for dimensions");
    let (width, height) = (img_reader.width(), img_reader.height());

    let builder = pdf_builder_new();
    let _ = pdf_builder_add_page(builder, img_bytes.as_ptr(), img_bytes.len(), width, height, 300.0, c_json.as_ptr());
    // Drop without finalizing — must not leak or crash
    pdf_builder_free(builder);
}

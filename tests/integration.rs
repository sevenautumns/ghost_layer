use ghostlayer::{
    free_pdf_buffer, ghost_layer_doc_add_image_page, ghost_layer_doc_add_ocr_page,
    ghost_layer_doc_finish_images, ghost_layer_doc_finish_images_to_path,
    ghost_layer_doc_finish_ocr_to_path, ghost_layer_doc_new_images, ghost_layer_doc_new_ocr,
    pdf_get_last_error,
};
use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;

struct TestPage {
    img_bytes: Vec<u8>,
    width: u32,
    height: u32,
    json: CString,
}

fn load_all_test_pages() -> Vec<TestPage> {
    let pairs = [
        ("tests/en_ltr.png", "tests/en_ltr.json"),
        ("tests/ar_rtl.jpg", "tests/ar_rtl.json"),
        ("tests/jp_ltr.jpg", "tests/jp_ltr.json"),
        ("tests/jp_ttb.png", "tests/jp_ttb.json"),
    ];
    pairs
        .iter()
        .map(|(img_path, json_path)| {
            let img_bytes = fs::read(img_path).expect("Read image");
            let img = image::load_from_memory(&img_bytes).expect("Load image");
            let json_str = fs::read_to_string(json_path).expect("Read JSON");
            TestPage {
                img_bytes,
                width: img.width(),
                height: img.height(),
                json: CString::new(json_str).expect("CString"),
            }
        })
        .collect()
}

fn last_error_string() -> String {
    let p = pdf_get_last_error();
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn build_streaming_image_pdf(pages: &[TestPage]) -> Vec<u8> {
    let doc = ghost_layer_doc_new_images();
    assert!(!doc.is_null());
    for p in pages {
        unsafe {
            ghost_layer_doc_add_image_page(
                doc,
                p.img_bytes.as_ptr(),
                p.img_bytes.len(),
                p.width,
                p.height,
                300.0,
                p.json.as_ptr(),
            );
        }
        assert!(
            last_error_string().is_empty(),
            "add_image_page set error: {}",
            last_error_string()
        );
    }
    let buf = unsafe { ghost_layer_doc_finish_images(doc) };
    assert!(!buf.data.is_null(), "finish_images returned null");
    let bytes = unsafe { std::slice::from_raw_parts(buf.data, buf.len) }.to_vec();
    free_pdf_buffer(buf);
    bytes
}

#[test]
fn doc_finish_images_to_path_writes_valid_pdf() {
    let test_pages = load_all_test_pages();
    let out_path = Path::new("tests/output_streaming_images.pdf");
    if out_path.exists() {
        fs::remove_file(out_path).expect("rm prior");
    }
    let c_path = CString::new(out_path.to_str().unwrap()).expect("CString");

    let doc = ghost_layer_doc_new_images();
    assert!(!doc.is_null());
    for p in &test_pages {
        unsafe {
            ghost_layer_doc_add_image_page(
                doc,
                p.img_bytes.as_ptr(),
                p.img_bytes.len(),
                p.width,
                p.height,
                300.0,
                p.json.as_ptr(),
            );
        }
        assert!(
            last_error_string().is_empty(),
            "add_image_page set error: {}",
            last_error_string()
        );
    }
    let rc = unsafe { ghost_layer_doc_finish_images_to_path(doc, c_path.as_ptr()) };
    assert_eq!(rc, 0, "finish_to_path failed: {}", last_error_string());
    assert!(out_path.exists(), "output file not created");
    let bytes = fs::read(out_path).expect("read output");
    assert!(
        bytes.len() > 1000,
        "PDF suspiciously small: {} bytes",
        bytes.len()
    );

    let loaded = lopdf::Document::load_mem(&bytes).expect("parse output PDF");
    assert_eq!(
        loaded.get_pages().len(),
        test_pages.len(),
        "page count mismatch"
    );
}

#[test]
fn doc_finish_ocr_to_path_overlays_existing_pdf() {
    let test_pages = load_all_test_pages();
    let source_pdf = build_streaming_image_pdf(&test_pages);

    let out_path = Path::new("tests/output_streaming_ocr.pdf");
    if out_path.exists() {
        fs::remove_file(out_path).expect("rm prior");
    }
    let c_path = CString::new(out_path.to_str().unwrap()).expect("CString");

    let doc = ghost_layer_doc_new_ocr();
    assert!(!doc.is_null());
    for p in &test_pages {
        unsafe { ghost_layer_doc_add_ocr_page(doc, p.json.as_ptr()) };
    }
    let rc = unsafe {
        ghost_layer_doc_finish_ocr_to_path(
            doc,
            source_pdf.as_ptr(),
            source_pdf.len(),
            c_path.as_ptr(),
        )
    };
    assert_eq!(rc, 0, "ocr_to_path failed: {}", last_error_string());
    assert!(out_path.exists(), "ocr output file not created");

    let bytes = fs::read(out_path).expect("read output");
    let loaded = lopdf::Document::load_mem(&bytes).expect("parse ocr output");
    assert_eq!(loaded.get_pages().len(), test_pages.len());
}

#[test]
fn doc_finish_images_to_path_null_path_returns_error() {
    let doc = ghost_layer_doc_new_images();
    let rc = unsafe { ghost_layer_doc_finish_images_to_path(doc, std::ptr::null()) };
    assert_eq!(rc, -1);
    unsafe { ghostlayer::ghost_layer_doc_free(doc) };
}

#[test]
fn doc_finish_images_to_path_empty_builder_returns_error() {
    let doc = ghost_layer_doc_new_images();
    let out = Path::new("tests/output_streaming_empty.pdf");
    let c_path = CString::new(out.to_str().unwrap()).expect("CString");
    let rc = unsafe { ghost_layer_doc_finish_images_to_path(doc, c_path.as_ptr()) };
    assert_eq!(rc, -1, "expected error for empty builder");
    assert!(!last_error_string().is_empty());
}

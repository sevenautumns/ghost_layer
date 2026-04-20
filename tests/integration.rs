use ghostlayer::{
    free_pdf_buffer, generate_pdf_from_images, pdf_ocr_document, GhostLayerImagePage,
};
use std::ffi::CString;
use std::fs;
use std::path::Path;

fn load_test_image() -> (Vec<u8>, u32, u32) {
    let img_bytes = fs::read("tests/en_ltr.png").expect("Read PNG");
    let img = image::load_from_memory(&img_bytes).expect("Load image");
    (img_bytes, img.width(), img.height())
}

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

#[test]
fn generate_pdf_from_images_single_page_returns_valid_pdf() {
    let (img_bytes, width, height) = load_test_image();
    let json_content = fs::read_to_string("tests/en_ltr.json").expect("Read JSON");
    let c_json = CString::new(json_content).expect("CString");

    let page = GhostLayerImagePage {
        img_ptr: img_bytes.as_ptr(),
        img_len: img_bytes.len(),
        width_px: width,
        height_px: height,
        dpi: 300.0,
    };
    let jsons: [*const std::ffi::c_char; 1] = [c_json.as_ptr()];

    let pdf_buffer = unsafe { generate_pdf_from_images(&page as *const _, jsons.as_ptr(), 1) };

    assert!(!pdf_buffer.data.is_null(), "PDF data pointer is null");
    assert!(pdf_buffer.len > 0, "PDF length is 0");

    let result = unsafe { std::slice::from_raw_parts(pdf_buffer.data, pdf_buffer.len) };
    fs::write(Path::new("tests/output_ffi.pdf"), result).expect("Write PDF");

    free_pdf_buffer(pdf_buffer);
}

#[test]
fn generate_pdf_from_images_multipage_returns_valid_pdf() {
    let test_pages = load_all_test_pages();
    let pages: Vec<GhostLayerImagePage> = test_pages
        .iter()
        .map(|p| GhostLayerImagePage {
            img_ptr: p.img_bytes.as_ptr(),
            img_len: p.img_bytes.len(),
            width_px: p.width,
            height_px: p.height,
            dpi: 300.0,
        })
        .collect();
    let json_ptrs: Vec<*const std::ffi::c_char> =
        test_pages.iter().map(|p| p.json.as_ptr()).collect();
    let count = pages.len() as i32;

    let pdf_buffer = unsafe { generate_pdf_from_images(pages.as_ptr(), json_ptrs.as_ptr(), count) };

    assert!(!pdf_buffer.data.is_null(), "PDF data pointer is null");
    assert!(pdf_buffer.len > 0, "PDF length is 0");

    let result = unsafe { std::slice::from_raw_parts(pdf_buffer.data, pdf_buffer.len) };
    fs::write("tests/output_ffi_multipage.pdf", result).expect("Write PDF");

    free_pdf_buffer(pdf_buffer);
}

#[test]
fn pdf_ocr_document_overlays_text_on_existing_pdf() {
    let test_pages = load_all_test_pages();
    let pages: Vec<GhostLayerImagePage> = test_pages
        .iter()
        .map(|p| GhostLayerImagePage {
            img_ptr: p.img_bytes.as_ptr(),
            img_len: p.img_bytes.len(),
            width_px: p.width,
            height_px: p.height,
            dpi: 300.0,
        })
        .collect();
    let json_ptrs: Vec<*const std::ffi::c_char> =
        test_pages.iter().map(|p| p.json.as_ptr()).collect();
    let count = pages.len() as i32;

    let source = unsafe { generate_pdf_from_images(pages.as_ptr(), json_ptrs.as_ptr(), count) };
    assert!(!source.data.is_null());

    let source_pdf = unsafe { std::slice::from_raw_parts(source.data, source.len) };

    let overlay_jsons: Vec<*const std::ffi::c_char> =
        test_pages.iter().map(|p| p.json.as_ptr()).collect();

    let out_buf = unsafe {
        pdf_ocr_document(
            source_pdf.as_ptr(),
            source_pdf.len(),
            overlay_jsons.as_ptr(),
            count,
        )
    };
    assert!(!out_buf.data.is_null(), "pdf_ocr_document returned null");
    assert!(out_buf.len > 0);

    let result = unsafe { std::slice::from_raw_parts(out_buf.data, out_buf.len) };
    fs::write("tests/output_ocr_document.pdf", result).expect("Write");

    free_pdf_buffer(out_buf);
    free_pdf_buffer(source);
}

#[test]
fn pdf_ocr_document_null_input_returns_null() {
    let null_buf = unsafe { pdf_ocr_document(std::ptr::null(), 0, std::ptr::null(), 0) };
    assert!(null_buf.data.is_null());
}

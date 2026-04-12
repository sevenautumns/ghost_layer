use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::slice;

use lopdf::{
    content::{Content, Operation},
    dictionary,
    xobject, Document, Object, ObjectId, Stream, StringFormat,
};
use serde::Deserialize;

// --- 0. ERROR REPORTING ---

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(e: &dyn std::error::Error) {
    let msg = CString::new(e.to_string()).unwrap_or_else(|_| CString::new("unknown error").unwrap());
    LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(msg));
}

fn clear_last_error() {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = None);
}

/// Returns a pointer to the last error message, or NULL if no error occurred.
/// The pointer is valid until the next FFI call on this thread.
#[no_mangle]
pub extern "C" fn pdf_get_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow().as_ref().map_or(std::ptr::null(), |s| s.as_ptr())
    })
}

// --- 1. DATA STRUCTURES ---

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct Geometry {
    top_left: Point,
    bottom_left: Point,
    bottom_right: Point,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Direction {
    LeftToRight,
    RightToLeft,
    TopToBottom,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OCRWord {
    text: String,
    geometry: Geometry,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OCRLine {
    geometry: Geometry,
    direction: Direction,
    words: Vec<OCRWord>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OCRParagraph {
    lines: Vec<OCRLine>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OCRInput {
    paragraphs: Vec<OCRParagraph>,
}

// --- 2. FONT EMBEDDING ---

const FONT_DATA: &[u8] = include_bytes!("pdf.ttf");
const FONT_NAME: &str = "f-0-0";

const K_CHAR_WIDTH: f64 = 2.0;
const NOMINAL_GLYPH_WIDTH: f64 = 1000.0 / K_CHAR_WIDTH;

const TO_UNICODE_CMAP: &str = r#"/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo
<<
  /Registry (Adobe)
  /Ordering (UCS)
  /Supplement 0
>> def
/CMapName /Adobe-Identify-UCS def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
1 beginbfrange
<0000> <FFFF> <0000>
endbfrange
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"#;

fn n(name: &str) -> Object {
    Object::Name(name.as_bytes().to_vec())
}

fn s(text: &str) -> Object {
    Object::String(text.as_bytes().to_vec(), StringFormat::Literal)
}

fn add_glyphless_font(doc: &mut Document) -> Object {
    let font_stream = Stream::new(
        dictionary! {
            "Length1" => FONT_DATA.len() as i64,
            "Length" => FONT_DATA.len() as i64
        },
        FONT_DATA.to_vec(),
    );
    let font_stream_id = doc.add_object(font_stream);

    let map_data = [0u8, 1].repeat(65536);
    let mut map_stream = Stream::new(dictionary! { "Length" => 131072 }, map_data);
    let _ = map_stream.compress();
    let map_stream_id = doc.add_object(map_stream);

    let to_unicode_stream = Stream::new(dictionary! {}, TO_UNICODE_CMAP.as_bytes().to_vec());
    let to_unicode_id = doc.add_object(to_unicode_stream);

    let font_descriptor = dictionary! {
        "Type" => n("FontDescriptor"),
        "FontName" => n("GlyphLessFont"),
        "FontFile2" => font_stream_id,
        "Flags" => 5,
        "FontBBox" => vec![0.into(), (-1).into(), NOMINAL_GLYPH_WIDTH.into(), 1000.into()],
        "Ascent" => 1000,
        "Descent" => -1,
        "CapHeight" => 1000,
        "StemV" => 80,
        "ItalicAngle" => 0,
    };
    let descriptor_id = doc.add_object(font_descriptor);

    let cid_font = dictionary! {
        "Type" => n("Font"),
        "Subtype" => n("CIDFontType2"),
        "BaseFont" => n("GlyphLessFont"),
        "CIDSystemInfo" => dictionary! {
            "Registry" => s("Adobe"),
            "Ordering" => s("Identity"),
            "Supplement" => 0,
        },
        "FontDescriptor" => descriptor_id,
        "DW" => 500,
        "W" => vec![0.into(), vec![500.into()].into()],
        "CIDToGIDMap" => map_stream_id,
    };
    let cid_font_id = doc.add_object(cid_font);

    let font_dict = dictionary! {
        "Type" => n("Font"),
        "Subtype" => n("Type0"),
        "BaseFont" => n("GlyphLessFont"),
        "Encoding" => n("Identity-H"),
        "DescendantFonts" => vec![cid_font_id.into()],
        "ToUnicode" => to_unicode_id,
    };

    doc.add_object(font_dict).into()
}

// --- 3. MATH & LOGIC ---

fn prec(n: f64) -> f64 {
    (n * 1000.0).round() / 1000.0
}

fn clip_baseline(p1: Point, p2: Point) -> (Point, Point) {
    let rise = (p2.y - p1.y).abs();
    let run = (p2.x - p1.x).abs();
    // Clip if nearly horizontal: same logic as Tesseract (rise < 2/72 inch threshold),
    // adapted for normalized coords. Avoids wandering baselines in viewers like Preview.
    if run > 0.01 && rise < run * 0.028 {
        let avg_y = (p1.y + p2.y) / 2.0;
        (Point { x: p1.x, y: avg_y }, Point { x: p2.x, y: avg_y })
    } else {
        (p1, p2)
    }
}

// --- 4. PDF BUILDER ---

pub struct PdfBuilder {
    doc: Document,
    font_id: Object,
    pages_id: ObjectId,
    page_ids: Vec<Object>,
    finalized: bool,
}

impl PdfBuilder {
    fn new() -> Self {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = add_glyphless_font(&mut doc);
        PdfBuilder { doc, font_id, pages_id, page_ids: vec![], finalized: false }
    }

    pub fn add_page(&mut self, image_bytes: &[u8], json_input: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.finalized {
            return Err("add_page called after finalize".into());
        }
        let input: OCRInput = serde_json::from_str(json_input)?;
        let img_reader = image::load_from_memory(image_bytes)?;

        let width_pts = img_reader.width() as f64;
        let height_pts = img_reader.height() as f64;

        let image_stream = xobject::image_from(image_bytes.to_vec())?;
        let image_id = self.doc.add_object(Object::Stream(image_stream));

        let image_name = "Im1";
        let font_ref = Object::Name(FONT_NAME.into());

        let to_pdf_pt = |p: &Point| -> (f64, f64) {
            (p.x * width_pts, p.y * height_pts)
        };

        let mut ops = Vec::new();

        // Draw image
        ops.push(Operation::new("q", vec![]));
        ops.push(Operation::new("cm", vec![
            prec(width_pts).into(), 0.into(), 0.into(), prec(height_pts).into(), 0.into(), 0.into()
        ]));
        ops.push(Operation::new("Do", vec![Object::Name(image_name.into())]));
        ops.push(Operation::new("Q", vec![]));

        #[cfg(debug_assertions)]
        {
            ops.push(Operation::new("q", vec![]));
            ops.push(Operation::new("RG", vec![1.into(), 0.into(), 0.into()]));
            ops.push(Operation::new("w", vec![0.5.into()]));

            for paragraph in &input.paragraphs {
                for line in &paragraph.lines {
                    for word in &line.words {
                        let (wx_tl, wy_tl) = to_pdf_pt(&word.geometry.top_left);
                        let (wx_bl, wy_bl) = to_pdf_pt(&word.geometry.bottom_left);
                        let (wx_br, wy_br) = to_pdf_pt(&word.geometry.bottom_right);

                        let wx_tr = wx_tl + (wx_br - wx_bl);
                        let wy_tr = wy_tl + (wy_br - wy_bl);

                        ops.push(Operation::new("m", vec![prec(wx_bl).into(), prec(wy_bl).into()]));
                        ops.push(Operation::new("l", vec![prec(wx_br).into(), prec(wy_br).into()]));
                        ops.push(Operation::new("l", vec![prec(wx_tr).into(), prec(wy_tr).into()]));
                        ops.push(Operation::new("l", vec![prec(wx_tl).into(), prec(wy_tl).into()]));
                        ops.push(Operation::new("h", vec![]));
                        ops.push(Operation::new("S", vec![]));
                    }
                }
            }
            ops.push(Operation::new("Q", vec![]));
        }

        // Draw invisible text layer
        for paragraph in input.paragraphs {
            ops.push(Operation::new("BT", vec![]));
            ops.push(Operation::new("Tr", vec![3.into()]));

            let mut old_x = 0.0;
            let mut old_y = 0.0;
            let mut old_fontsize = 0.0;
            let mut old_dir = Direction::LeftToRight;
            let mut is_new_block = true;

            for line in paragraph.lines {
                let (lp1, lp2) = clip_baseline(line.geometry.bottom_left, line.geometry.bottom_right);
                let (lx1, ly1) = to_pdf_pt(&lp1);
                let (lx2, ly2) = to_pdf_pt(&lp2);

                let l_dx = lx2 - lx1;
                let l_dy = ly2 - ly1;
                let line_len_sq = l_dx * l_dx + l_dy * l_dy;

                let theta = l_dy.atan2(l_dx);
                let a = theta.cos();
                let b = theta.sin();
                let c = -theta.sin();
                let d = theta.cos();

                for word in line.words {
                    if word.text.trim().is_empty() { continue; }

                    let (wx_tl, wy_tl) = to_pdf_pt(&word.geometry.top_left);
                    let (wx_bl, wy_bl) = to_pdf_pt(&word.geometry.bottom_left);
                    let (wx_br, wy_br) = to_pdf_pt(&word.geometry.bottom_right);

                    let font_size = ((wx_tl - wx_bl).powi(2) + (wy_tl - wy_bl).powi(2)).sqrt();
                    let word_length = ((wx_br - wx_bl).powi(2) + (wy_br - wy_bl).powi(2)).sqrt();

                    if font_size < 0.1 || word_length < 0.1 { continue; }

                    let (target_x, target_y) = (wx_bl, wy_bl);

                    let (px, py) = if line_len_sq < 0.001 {
                        (target_x, target_y)
                    } else {
                        let t = ((target_x - lx1) * l_dx + (target_y - ly1) * l_dy) / line_len_sq;
                        (lx1 + t * l_dx, ly1 + t * l_dy)
                    };

                    if is_new_block || line.direction != old_dir {
                        ops.push(Operation::new("Tm", vec![
                            prec(a).into(), prec(b).into(), prec(c).into(), prec(d).into(),
                            prec(px).into(), prec(py).into()
                        ]));
                        is_new_block = false;
                    } else {
                        let dx = px - old_x;
                        let dy = py - old_y;
                        let tx = dx * a + dy * b;
                        let ty = dx * c + dy * d;
                        ops.push(Operation::new("Td", vec![prec(tx).into(), prec(ty).into()]));
                    }

                    old_x = px;
                    old_y = py;
                    old_dir = line.direction;

                    if (font_size - old_fontsize).abs() > 0.01 {
                        ops.push(Operation::new("Tf", vec![font_ref.clone(), prec(font_size).into()]));
                        old_fontsize = font_size;
                    }

                    let char_count = word.text.encode_utf16().count() as f64;
                    let h_scale = if char_count > 0.0 {
                        let raw = K_CHAR_WIDTH * (100.0 * word_length) / (font_size * char_count);
                        raw.clamp(1.0, 2000.0)
                    } else {
                        100.0
                    };
                    ops.push(Operation::new("Tz", vec![prec(h_scale).into()]));

                    let mut utf16_bytes: Vec<u8> = word.text.encode_utf16().flat_map(|x| x.to_be_bytes()).collect();
                    utf16_bytes.push(0x00); utf16_bytes.push(0x20);
                    let hex_str = Object::String(utf16_bytes, StringFormat::Hexadecimal);
                    ops.push(Operation::new("TJ", vec![Object::Array(vec![hex_str])]));
                }
            }
            ops.push(Operation::new("ET", vec![]));
        }

        let content = Content { operations: ops };
        let mut content_stream = Stream::new(dictionary! {}, content.encode()?);
        let _ = content_stream.compress();
        let content_id = self.doc.add_object(content_stream);

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => self.pages_id,
            "MediaBox" => vec![0.into(), 0.into(), prec(width_pts).into(), prec(height_pts).into()],
            "Contents" => content_id,
            "Resources" => dictionary! {
                "Font" => dictionary! { FONT_NAME => self.font_id.clone() },
                "XObject" => dictionary! { image_name => image_id },
                "ProcSet" => vec!["PDF".into(), "Text".into(), "ImageB".into(), "ImageC".into(), "ImageI".into()]
            }
        };
        let page_id = self.doc.add_object(page_dict);
        self.page_ids.push(page_id.into());

        Ok(())
    }

    pub fn finalize(mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.finalized = true;
        let count = self.page_ids.len() as i64;
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => self.page_ids,
            "Count" => count,
        };
        self.doc.objects.insert(self.pages_id, Object::Dictionary(pages_dict));

        let catalog_id = self.doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => self.pages_id,
        });
        self.doc.trailer.set("Root", catalog_id);

        let mut buffer = Vec::new();
        self.doc.save_to(&mut buffer)?;
        Ok(buffer)
    }
}

// --- 5. FFI ---

#[repr(C)]
pub struct PdfBuffer {
    pub data: *mut u8,
    pub len: usize,
    capacity: usize,
}

fn vec_to_pdf_buffer(mut vec: Vec<u8>) -> PdfBuffer {
    let len = vec.len();
    let capacity = vec.capacity();
    let data = vec.as_mut_ptr();
    std::mem::forget(vec);
    PdfBuffer { data, len, capacity }
}

#[no_mangle]
pub extern "C" fn pdf_builder_new() -> *mut PdfBuilder {
    Box::into_raw(Box::new(PdfBuilder::new()))
}

#[no_mangle]
pub extern "C" fn pdf_builder_add_page(
    builder: *mut PdfBuilder,
    img_ptr: *const u8,
    img_len: usize,
    json_ptr: *const c_char,
) -> i32 {
    if builder.is_null() || img_ptr.is_null() || json_ptr.is_null() {
        return 0;
    }
    let builder = unsafe { &mut *builder };
    let image_bytes = unsafe { slice::from_raw_parts(img_ptr, img_len) };
    let json_str = unsafe { CStr::from_ptr(json_ptr).to_string_lossy() };

    match builder.add_page(image_bytes, &json_str) {
        Ok(_) => { clear_last_error(); 1 }
        Err(e) => { set_last_error(e.as_ref()); 0 }
    }
}

#[no_mangle]
pub extern "C" fn pdf_builder_finalize(builder: *mut PdfBuilder) -> PdfBuffer {
    if builder.is_null() {
        return PdfBuffer { data: std::ptr::null_mut(), len: 0, capacity: 0 };
    }
    let builder = unsafe { Box::from_raw(builder) };
    match builder.finalize() {
        Ok(vec) => { clear_last_error(); vec_to_pdf_buffer(vec) }
        Err(e) => { set_last_error(e.as_ref()); PdfBuffer { data: std::ptr::null_mut(), len: 0, capacity: 0 } }
    }
}

#[no_mangle]
pub extern "C" fn pdf_builder_free(builder: *mut PdfBuilder) {
    if !builder.is_null() {
        unsafe { drop(Box::from_raw(builder)); }
    }
}

/// Single-page convenience wrapper — backward compatible.
#[no_mangle]
pub extern "C" fn generate_pdf_from_ocr(
    img_ptr: *const u8,
    img_len: usize,
    json_ptr: *const c_char,
) -> PdfBuffer {
    if img_ptr.is_null() || json_ptr.is_null() {
        return PdfBuffer { data: std::ptr::null_mut(), len: 0, capacity: 0 };
    }
    let image_bytes = unsafe { slice::from_raw_parts(img_ptr, img_len) };
    let json_str = unsafe { CStr::from_ptr(json_ptr).to_string_lossy() };

    let mut builder = PdfBuilder::new();
    let result = builder.add_page(image_bytes, &json_str)
        .and_then(|_| builder.finalize());

    match result {
        Ok(vec) => { clear_last_error(); vec_to_pdf_buffer(vec) }
        Err(e) => { set_last_error(e.as_ref()); PdfBuffer { data: std::ptr::null_mut(), len: 0, capacity: 0 } }
    }
}

#[no_mangle]
pub extern "C" fn free_pdf_buffer(buf: PdfBuffer) {
    if !buf.data.is_null() {
        unsafe { let _ = Vec::from_raw_parts(buf.data, buf.len, buf.capacity); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- prec ---

    #[test]
    fn prec_rounds_down() {
        assert_eq!(prec(1.2344), 1.234);
    }

    #[test]
    fn prec_rounds_up() {
        assert_eq!(prec(1.2345), 1.235);
    }

    #[test]
    fn prec_negative_zero_stays_negative() {
        // Tesseract has a special case to convert -0 to 0; we don't.
        // This test documents that our prec(-0.0) == -0.0 (IEEE 754).
        assert!(prec(-0.0).is_sign_negative());
    }

    #[test]
    fn prec_large_value_unchanged_in_integer_part() {
        assert_eq!(prec(1234.5678), 1234.568);
    }

    // --- clip_baseline ---

    fn pt(x: f64, y: f64) -> Point { Point { x, y } }

    #[test]
    fn clip_baseline_nearly_horizontal_clips() {
        // rise/run = 0.001/0.5 = 0.002, well below 0.028 threshold
        let (p1, p2) = clip_baseline(pt(0.1, 0.500), pt(0.6, 0.501));
        let avg_y = (0.500 + 0.501) / 2.0;
        assert_eq!(p1.y, avg_y);
        assert_eq!(p2.y, avg_y);
        assert_eq!(p1.x, 0.1);
        assert_eq!(p2.x, 0.6);
    }

    #[test]
    fn clip_baseline_exact_horizontal_clips() {
        let (p1, p2) = clip_baseline(pt(0.1, 0.5), pt(0.9, 0.5));
        assert_eq!(p1.y, 0.5);
        assert_eq!(p2.y, 0.5);
    }

    #[test]
    fn clip_baseline_vertical_unchanged() {
        // run = 0, rise = 0.5 → must not clip
        let (p1, p2) = clip_baseline(pt(0.5, 0.1), pt(0.5, 0.6));
        assert_eq!(p1, pt(0.5, 0.1));
        assert_eq!(p2, pt(0.5, 0.6));
    }

    #[test]
    fn clip_baseline_diagonal_unchanged() {
        // 45°: rise == run → ratio 1.0, far above 0.028
        let (p1, p2) = clip_baseline(pt(0.1, 0.1), pt(0.6, 0.6));
        assert_eq!(p1, pt(0.1, 0.1));
        assert_eq!(p2, pt(0.6, 0.6));
    }

    #[test]
    fn clip_baseline_too_short_unchanged() {
        // run < 0.01 → skip clipping even if nearly horizontal
        let (p1, p2) = clip_baseline(pt(0.1, 0.500), pt(0.105, 0.5001));
        assert_eq!(p1, pt(0.1, 0.500));
        assert_eq!(p2, pt(0.105, 0.5001));
    }

    #[test]
    fn clip_baseline_just_above_threshold_unchanged() {
        // rise/run = 0.028 → equal to threshold, should NOT clip (strict <)
        let (p1, p2) = clip_baseline(pt(0.0, 0.0), pt(0.5, 0.014));
        assert_eq!(p1, pt(0.0, 0.0));
        assert_eq!(p2, pt(0.5, 0.014));
    }
}

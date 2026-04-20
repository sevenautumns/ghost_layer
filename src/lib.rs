use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::slice;

use lopdf::{
    content::{Content, Operation},
    dictionary, xobject, Dictionary, Document, Object, ObjectId, Stream, StringFormat,
};
use serde::Deserialize;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(e: &dyn std::error::Error) {
    let msg =
        CString::new(e.to_string()).unwrap_or_else(|_| CString::new("unknown error").unwrap());
    LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(msg));
}

fn clear_last_error() {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = None);
}

#[no_mangle]
pub extern "C" fn pdf_get_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ptr())
    })
}

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

const FONT_DATA: &[u8] = include_bytes!("pdf.ttf");
const FONT_NAME: &str = "f-0-0";

const CHAR_WIDTH: f64 = 2.0;

struct ImagePage<'a> {
    image_bytes: &'a [u8],
    width_px: u32,
    height_px: u32,
    dpi: f64,
    json_input: Option<&'a str>,
}
const GLYPH_WIDTH_FONT_UNITS: f64 = 1000.0 / CHAR_WIDTH;

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

fn add_glyphless_font(doc: &mut Document) -> Object {
    let n = |name: &str| Object::Name(name.as_bytes().to_vec());
    let s = |text: &str| Object::String(text.as_bytes().to_vec(), StringFormat::Literal);
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
        "FontBBox" => vec![0.into(), (-1).into(), GLYPH_WIDTH_FONT_UNITS.into(), 1000.into()],
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

fn round3(n: f64) -> f64 {
    (n * 1000.0).round() / 1000.0
}

fn level_baseline(p1: Point, p2: Point) -> (Point, Point) {
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

fn add_compressed_content(
    doc: &mut Document,
    ops: Vec<Operation>,
) -> Result<ObjectId, Box<dyn std::error::Error>> {
    let mut stream = Stream::new(dictionary! {}, Content { operations: ops }.encode()?);
    let _ = stream.compress();
    Ok(doc.add_object(stream))
}

fn get_media_box(
    doc: &Document,
    page_id: ObjectId,
) -> Result<[f64; 4], Box<dyn std::error::Error>> {
    let mut current = page_id;
    loop {
        let dict = doc.get_dictionary(current)?;
        if let Ok(mb) = dict.get(b"MediaBox") {
            let arr = mb.as_array()?;
            if arr.len() >= 4 {
                return Ok([
                    arr[0].as_float()? as f64,
                    arr[1].as_float()? as f64,
                    arr[2].as_float()? as f64,
                    arr[3].as_float()? as f64,
                ]);
            }
        }
        match dict.get(b"Parent").and_then(Object::as_reference) {
            Ok(parent_id) => current = parent_id,
            Err(_) => return Err("MediaBox not found in page tree".into()),
        }
    }
}

fn upsert_font(res: &mut Dictionary, font_obj: Object) {
    let already_present = res
        .get(b"Font")
        .ok()
        .and_then(|o| {
            if let Object::Dictionary(d) = o {
                Some(d)
            } else {
                None
            }
        })
        .map(|d| d.get(FONT_NAME.as_bytes()).is_ok())
        .unwrap_or(false);

    if already_present {
        return;
    }

    let font_dict_exists = matches!(res.get(b"Font"), Ok(Object::Dictionary(_)));

    if font_dict_exists {
        if let Ok(Object::Dictionary(fonts)) = res.get_mut(b"Font") {
            fonts.set(FONT_NAME, font_obj);
        }
    } else {
        let mut fonts = Dictionary::new();
        fonts.set(FONT_NAME, font_obj);
        res.set("Font", Object::Dictionary(fonts));
    }
}

fn add_font_to_page_resources(doc: &mut Document, page_id: ObjectId, font_obj: Object) {
    let res_ref = doc
        .objects
        .get(&page_id)
        .and_then(|o| {
            if let Object::Dictionary(d) = o {
                Some(d)
            } else {
                None
            }
        })
        .and_then(|d| d.get(b"Resources").ok())
        .and_then(|o| {
            if let Object::Reference(id) = o {
                Some(*id)
            } else {
                None
            }
        });

    if let Some(res_id) = res_ref {
        if let Some(Object::Dictionary(res)) = doc.objects.get_mut(&res_id) {
            upsert_font(res, font_obj);
        }
        return;
    }

    let res_clone = doc
        .objects
        .get(&page_id)
        .and_then(|o| {
            if let Object::Dictionary(d) = o {
                Some(d)
            } else {
                None
            }
        })
        .and_then(|d| d.get(b"Resources").ok())
        .and_then(|o| {
            if let Object::Dictionary(d) = o {
                Some(d.clone())
            } else {
                None
            }
        });

    let mut res = res_clone.unwrap_or_default();
    upsert_font(&mut res, font_obj);

    if let Some(Object::Dictionary(page_dict)) = doc.objects.get_mut(&page_id) {
        page_dict.set("Resources", Object::Dictionary(res));
    }
}

fn ocr_operations(
    width_pts: f64,
    height_pts: f64,
    x_off: f64,
    y_off: f64,
    font_ref: Object,
    input: OCRInput,
) -> Vec<Operation> {
    let to_pdf_pt =
        |p: &Point| -> (f64, f64) { (x_off + p.x * width_pts, y_off + p.y * height_pts) };

    let mut ops = Vec::new();

    for paragraph in input.paragraphs {
        for line in paragraph.lines {
            let (lp1, lp2) = level_baseline(line.geometry.bottom_left, line.geometry.bottom_right);
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

            ops.push(Operation::new("BT", vec![]));
            ops.push(Operation::new("Tr", vec![3.into()]));

            let mut old_x = 0.0;
            let mut old_y = 0.0;
            let mut old_fontsize = 0.0;
            let mut is_first_word = true;

            for word in line.words {
                if word.text.trim().is_empty() {
                    continue;
                }

                let (wx_tl, wy_tl) = to_pdf_pt(&word.geometry.top_left);
                let (wx_bl, wy_bl) = to_pdf_pt(&word.geometry.bottom_left);
                let (wx_br, wy_br) = to_pdf_pt(&word.geometry.bottom_right);

                let font_size = ((wx_tl - wx_bl).powi(2) + (wy_tl - wy_bl).powi(2)).sqrt();
                let word_length = ((wx_br - wx_bl).powi(2) + (wy_br - wy_bl).powi(2)).sqrt();

                if font_size < 0.1 || word_length < 0.1 {
                    continue;
                }

                let (target_x, target_y) = (wx_bl, wy_bl);

                let (px, py) = if line_len_sq < 0.001 {
                    (target_x, target_y)
                } else {
                    let t = ((target_x - lx1) * l_dx + (target_y - ly1) * l_dy) / line_len_sq;
                    (lx1 + t * l_dx, ly1 + t * l_dy)
                };

                if is_first_word {
                    ops.push(Operation::new(
                        "Tm",
                        vec![
                            round3(a).into(),
                            round3(b).into(),
                            round3(c).into(),
                            round3(d).into(),
                            round3(px).into(),
                            round3(py).into(),
                        ],
                    ));
                    is_first_word = false;
                } else {
                    let dx = px - old_x;
                    let dy = py - old_y;
                    let tx = dx * a + dy * b;
                    let ty = dx * c + dy * d;
                    ops.push(Operation::new(
                        "Td",
                        vec![round3(tx).into(), round3(ty).into()],
                    ));
                }

                old_x = px;
                old_y = py;

                if (font_size - old_fontsize).abs() > 0.01 {
                    ops.push(Operation::new(
                        "Tf",
                        vec![font_ref.clone(), round3(font_size).into()],
                    ));
                    old_fontsize = font_size;
                }

                let char_count = word.text.encode_utf16().count() as f64;
                let h_scale = if char_count > 0.0 {
                    let raw = CHAR_WIDTH * (100.0 * word_length) / (font_size * char_count);
                    raw.clamp(1.0, 2000.0)
                } else {
                    100.0
                };
                ops.push(Operation::new("Tz", vec![round3(h_scale).into()]));

                let mut utf16_bytes: Vec<u8> = word
                    .text
                    .encode_utf16()
                    .flat_map(|x| x.to_be_bytes())
                    .collect();
                utf16_bytes.push(0x00);
                utf16_bytes.push(0x20);
                let hex_str = Object::String(utf16_bytes, StringFormat::Hexadecimal);
                ops.push(Operation::new("TJ", vec![Object::Array(vec![hex_str])]));
            }

            ops.push(Operation::new("ET", vec![]));
        }
    }

    ops
}

fn build_pdf_from_images(pages: &[ImagePage]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if pages.is_empty() {
        return Err("at least one page is required".into());
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = add_glyphless_font(&mut doc);

    let mut page_ids: Vec<ObjectId> = Vec::new();

    for (i, page) in pages.iter().enumerate() {
        let width_pts = (page.width_px as f64) * (72.0 / page.dpi);
        let height_pts = (page.height_px as f64) * (72.0 / page.dpi);
        let image_name = format!("Im{}", i + 1);
        let font_ref = Object::Name(FONT_NAME.into());

        let image_stream = xobject::image_from(page.image_bytes.to_vec())?;
        let image_id = doc.add_object(Object::Stream(image_stream));

        let mut ops = vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    round3(width_pts).into(),
                    0.into(),
                    0.into(),
                    round3(height_pts).into(),
                    0.into(),
                    0.into(),
                ],
            ),
            Operation::new("Do", vec![Object::Name(image_name.clone().into())]),
            Operation::new("Q", vec![]),
        ];

        if let Some(json) = page.json_input {
            let input: OCRInput = serde_json::from_str(json)?;

            #[cfg(debug_assertions)]
            {
                let to_pdf_pt = |p: &Point| -> (f64, f64) { (p.x * width_pts, p.y * height_pts) };

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

                            ops.push(Operation::new(
                                "m",
                                vec![round3(wx_bl).into(), round3(wy_bl).into()],
                            ));
                            ops.push(Operation::new(
                                "l",
                                vec![round3(wx_br).into(), round3(wy_br).into()],
                            ));
                            ops.push(Operation::new(
                                "l",
                                vec![round3(wx_tr).into(), round3(wy_tr).into()],
                            ));
                            ops.push(Operation::new(
                                "l",
                                vec![round3(wx_tl).into(), round3(wy_tl).into()],
                            ));
                            ops.push(Operation::new("h", vec![]));
                            ops.push(Operation::new("S", vec![]));
                        }
                    }
                }
                ops.push(Operation::new("Q", vec![]));
            }

            ops.extend(ocr_operations(
                width_pts, height_pts, 0.0, 0.0, font_ref, input,
            ));
        }

        let content_id = add_compressed_content(&mut doc, ops)?;

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), round3(width_pts).into(), round3(height_pts).into()],
            "Contents" => content_id,
            "Resources" => dictionary! {
                "Font" => dictionary! { FONT_NAME => font_id.clone() },
                "XObject" => dictionary! { image_name => image_id },
                "ProcSet" => vec!["PDF".into(), "Text".into(), "ImageB".into(), "ImageC".into(), "ImageI".into()]
            }
        };
        page_ids.push(doc.add_object(page_dict));
    }

    let kids: Vec<Object> = page_ids.iter().map(|&id| id.into()).collect();
    let count = page_ids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buffer = Vec::new();
    doc.save_to(&mut buffer)?;
    Ok(buffer)
}

fn ocr_document_inplace(
    pdf_bytes: &[u8],
    json_opts: &[Option<&str>],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut doc = Document::load_mem(pdf_bytes)?;
    let font_obj = add_glyphless_font(&mut doc);

    // Pass 1 — read-only: collect (page_id, mediabox) in page order
    let page_info: Vec<(ObjectId, [f64; 4])> = doc
        .get_pages()
        .into_values()
        .map(|id| {
            let mb = get_media_box(&doc, id).unwrap_or([0.0, 0.0, 612.0, 792.0]);
            (id, mb)
        })
        .collect();

    for (i, (page_id, mb)) in page_info.iter().enumerate() {
        let json_str = match json_opts.get(i).and_then(|o| *o) {
            Some(s) => s,
            None => continue,
        };

        let input: OCRInput = serde_json::from_str(json_str)?;
        let width_pts = mb[2] - mb[0];
        let height_pts = mb[3] - mb[1];
        let x_off = mb[0];
        let y_off = mb[1];

        let font_ref = Object::Name(FONT_NAME.into());
        let ops = ocr_operations(width_pts, height_pts, x_off, y_off, font_ref, input);

        let content_id = add_compressed_content(&mut doc, ops)?;

        let existing_contents = doc
            .objects
            .get(page_id)
            .and_then(|o| {
                if let Object::Dictionary(d) = o {
                    Some(d)
                } else {
                    None
                }
            })
            .and_then(|d| d.get(b"Contents").ok().cloned());

        let new_contents = match existing_contents {
            Some(Object::Reference(id)) => {
                Object::Array(vec![Object::Reference(id), Object::Reference(content_id)])
            }
            Some(Object::Array(mut arr)) => {
                arr.push(Object::Reference(content_id));
                Object::Array(arr)
            }
            _ => Object::Reference(content_id),
        };

        if let Some(Object::Dictionary(page_dict)) = doc.objects.get_mut(page_id) {
            page_dict.set("Contents", new_contents);
        }

        add_font_to_page_resources(&mut doc, *page_id, font_obj.clone());
    }

    let mut buffer = Vec::new();
    doc.save_to(&mut buffer)?;
    Ok(buffer)
}

#[repr(C)]
pub struct GhostLayerImagePage {
    pub img_ptr: *const u8,
    pub img_len: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: f64,
}

#[repr(C)]
pub struct PdfBuffer {
    pub data: *mut u8,
    pub len: usize,
    capacity: usize,
}

enum DocVariant {
    Images(Vec<(Vec<u8>, u32, u32, f64, Option<String>)>),
    Ocr(Vec<Option<String>>),
}

pub struct GhostLayerDoc {
    variant: DocVariant,
}

fn vec_to_pdf_buffer(mut vec: Vec<u8>) -> PdfBuffer {
    let len = vec.len();
    let capacity = vec.capacity();
    let data = vec.as_mut_ptr();
    std::mem::forget(vec);
    PdfBuffer {
        data,
        len,
        capacity,
    }
}

#[no_mangle]
pub unsafe extern "C" fn generate_pdf_from_images(
    pages: *const GhostLayerImagePage,
    json_array: *const *const c_char,
    page_count: i32,
) -> PdfBuffer {
    if pages.is_null() || json_array.is_null() || page_count < 1 {
        return PdfBuffer {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
    }

    let raw_pages = unsafe { slice::from_raw_parts(pages, page_count as usize) };
    let json_ptrs = unsafe { slice::from_raw_parts(json_array, page_count as usize) };

    let json_strings: Vec<Option<std::borrow::Cow<str>>> = json_ptrs
        .iter()
        .map(|&ptr| {
            if ptr.is_null() {
                None
            } else {
                Some(unsafe { CStr::from_ptr(ptr).to_string_lossy() })
            }
        })
        .collect();

    let image_pages: Vec<ImagePage> = raw_pages
        .iter()
        .zip(json_strings.iter())
        .map(|(p, json)| ImagePage {
            image_bytes: unsafe { slice::from_raw_parts(p.img_ptr, p.img_len) },
            width_px: p.width_px,
            height_px: p.height_px,
            dpi: p.dpi,
            json_input: json.as_deref(),
        })
        .collect();

    match build_pdf_from_images(&image_pages) {
        Ok(vec) => {
            clear_last_error();
            vec_to_pdf_buffer(vec)
        }
        Err(e) => {
            set_last_error(e.as_ref());
            PdfBuffer {
                data: std::ptr::null_mut(),
                len: 0,
                capacity: 0,
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn free_pdf_buffer(buf: PdfBuffer) {
    if !buf.data.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(buf.data, buf.len, buf.capacity);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn pdf_ocr_document(
    pdf_ptr: *const u8,
    pdf_len: usize,
    json_array: *const *const c_char,
    page_count: i32,
) -> PdfBuffer {
    if pdf_ptr.is_null() || json_array.is_null() || page_count < 0 {
        return PdfBuffer {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
    }
    let pdf_bytes = unsafe { slice::from_raw_parts(pdf_ptr, pdf_len) };
    let json_ptrs = unsafe { slice::from_raw_parts(json_array, page_count as usize) };

    let json_strings: Vec<Option<std::borrow::Cow<str>>> = json_ptrs
        .iter()
        .map(|&ptr| {
            if ptr.is_null() {
                None
            } else {
                Some(unsafe { CStr::from_ptr(ptr).to_string_lossy() })
            }
        })
        .collect();
    let json_opts: Vec<Option<&str>> = json_strings.iter().map(|o| o.as_deref()).collect();

    match ocr_document_inplace(pdf_bytes, &json_opts) {
        Ok(vec) => {
            clear_last_error();
            vec_to_pdf_buffer(vec)
        }
        Err(e) => {
            set_last_error(e.as_ref());
            PdfBuffer {
                data: std::ptr::null_mut(),
                len: 0,
                capacity: 0,
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn ghost_layer_doc_new_images() -> *mut GhostLayerDoc {
    Box::into_raw(Box::new(GhostLayerDoc {
        variant: DocVariant::Images(Vec::new()),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn ghost_layer_doc_add_image_page(
    doc: *mut GhostLayerDoc,
    img_ptr: *const u8,
    img_len: usize,
    width_px: u32,
    height_px: u32,
    dpi: f64,
    json_ptr: *const c_char,
) {
    if doc.is_null() || img_ptr.is_null() {
        return;
    }
    let doc = unsafe { &mut *doc };
    let DocVariant::Images(ref mut pages) = doc.variant else {
        return;
    };
    let img = unsafe { slice::from_raw_parts(img_ptr, img_len) }.to_vec();
    let json = if json_ptr.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(json_ptr) }
                .to_string_lossy()
                .into_owned(),
        )
    };
    pages.push((img, width_px, height_px, dpi, json));
}

#[no_mangle]
pub unsafe extern "C" fn ghost_layer_doc_finish_images(doc: *mut GhostLayerDoc) -> PdfBuffer {
    if doc.is_null() {
        return PdfBuffer {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
    }
    let doc = unsafe { Box::from_raw(doc) };
    let DocVariant::Images(raw_pages) = doc.variant else {
        return PdfBuffer {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
    };
    let pages: Vec<ImagePage> = raw_pages
        .iter()
        .map(|(img, w, h, dpi, json)| ImagePage {
            image_bytes: img,
            width_px: *w,
            height_px: *h,
            dpi: *dpi,
            json_input: json.as_deref(),
        })
        .collect();
    match build_pdf_from_images(&pages) {
        Ok(vec) => {
            clear_last_error();
            vec_to_pdf_buffer(vec)
        }
        Err(e) => {
            set_last_error(e.as_ref());
            PdfBuffer {
                data: std::ptr::null_mut(),
                len: 0,
                capacity: 0,
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn ghost_layer_doc_new_ocr() -> *mut GhostLayerDoc {
    Box::into_raw(Box::new(GhostLayerDoc {
        variant: DocVariant::Ocr(Vec::new()),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn ghost_layer_doc_add_ocr_page(
    doc: *mut GhostLayerDoc,
    json_ptr: *const c_char,
) {
    if doc.is_null() {
        return;
    }
    let doc = unsafe { &mut *doc };
    let DocVariant::Ocr(ref mut entries) = doc.variant else {
        return;
    };
    let json = if json_ptr.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(json_ptr) }
                .to_string_lossy()
                .into_owned(),
        )
    };
    entries.push(json);
}

#[no_mangle]
pub unsafe extern "C" fn ghost_layer_doc_finish_ocr(
    doc: *mut GhostLayerDoc,
    pdf_ptr: *const u8,
    pdf_len: usize,
) -> PdfBuffer {
    if doc.is_null() || pdf_ptr.is_null() {
        return PdfBuffer {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
    }
    let doc = unsafe { Box::from_raw(doc) };
    let DocVariant::Ocr(entries) = doc.variant else {
        return PdfBuffer {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
    };
    let pdf_bytes = unsafe { slice::from_raw_parts(pdf_ptr, pdf_len) };
    let json_opts: Vec<Option<&str>> = entries.iter().map(|o| o.as_deref()).collect();
    match ocr_document_inplace(pdf_bytes, &json_opts) {
        Ok(vec) => {
            clear_last_error();
            vec_to_pdf_buffer(vec)
        }
        Err(e) => {
            set_last_error(e.as_ref());
            PdfBuffer {
                data: std::ptr::null_mut(),
                len: 0,
                capacity: 0,
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ghost_layer_doc_free(doc: *mut GhostLayerDoc) {
    if !doc.is_null() {
        drop(unsafe { Box::from_raw(doc) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round3_rounds_down() {
        assert_eq!(round3(1.2344), 1.234);
    }

    #[test]
    fn round3_rounds_up() {
        assert_eq!(round3(1.2345), 1.235);
    }

    #[test]
    fn round3_negative_zero_stays_negative() {
        // Tesseract has a special case to convert -0 to 0; we don't.
        // This test documents that our round3(-0.0) == -0.0 (IEEE 754).
        assert!(round3(-0.0).is_sign_negative());
    }

    #[test]
    fn round3_large_value_unchanged_in_integer_part() {
        assert_eq!(round3(1234.5678), 1234.568);
    }

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn level_baseline_nearly_horizontal_clips() {
        // rise/run = 0.001/0.5 = 0.002, well below 0.028 threshold
        let (p1, p2) = level_baseline(pt(0.1, 0.500), pt(0.6, 0.501));
        let avg_y = (0.500 + 0.501) / 2.0;
        assert_eq!(p1.y, avg_y);
        assert_eq!(p2.y, avg_y);
        assert_eq!(p1.x, 0.1);
        assert_eq!(p2.x, 0.6);
    }

    #[test]
    fn level_baseline_exact_horizontal_clips() {
        let (p1, p2) = level_baseline(pt(0.1, 0.5), pt(0.9, 0.5));
        assert_eq!(p1.y, 0.5);
        assert_eq!(p2.y, 0.5);
    }

    #[test]
    fn level_baseline_vertical_unchanged() {
        // run = 0, rise = 0.5 → must not clip
        let (p1, p2) = level_baseline(pt(0.5, 0.1), pt(0.5, 0.6));
        assert_eq!(p1, pt(0.5, 0.1));
        assert_eq!(p2, pt(0.5, 0.6));
    }

    #[test]
    fn level_baseline_diagonal_unchanged() {
        // 45°: rise == run → ratio 1.0, far above 0.028
        let (p1, p2) = level_baseline(pt(0.1, 0.1), pt(0.6, 0.6));
        assert_eq!(p1, pt(0.1, 0.1));
        assert_eq!(p2, pt(0.6, 0.6));
    }

    #[test]
    fn level_baseline_too_short_unchanged() {
        // run < 0.01 → skip clipping even if nearly horizontal
        let (p1, p2) = level_baseline(pt(0.1, 0.500), pt(0.105, 0.5001));
        assert_eq!(p1, pt(0.1, 0.500));
        assert_eq!(p2, pt(0.105, 0.5001));
    }

    #[test]
    fn level_baseline_just_above_threshold_unchanged() {
        // rise/run = 0.028 → equal to threshold, should NOT clip (strict <)
        let (p1, p2) = level_baseline(pt(0.0, 0.0), pt(0.5, 0.014));
        assert_eq!(p1, pt(0.0, 0.0));
        assert_eq!(p2, pt(0.5, 0.014));
    }
}

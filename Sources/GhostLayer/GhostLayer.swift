import Foundation
import GhostLayerFFI

// MARK: - Error

public struct GhostLayerError: Error, CustomStringConvertible {
    public let description: String

    init(_ message: String) {
        description = message
    }

    static func last() -> GhostLayerError {
        if let ptr = pdf_get_last_error() {
            return GhostLayerError(String(cString: ptr))
        }
        return GhostLayerError("unknown error")
    }
}

// MARK: - ImageDocBuilder

public final class ImageDocBuilder {
    private var ptr: OpaquePointer?

    public init() {
        ptr = ghost_layer_doc_new_images()
    }

    deinit {
        if let p = ptr { ghost_layer_doc_free(p) }
    }

    public func addPage(
        image: Data,
        widthPx: UInt32,
        heightPx: UInt32,
        dpi: Double,
        json: String? = nil
    ) throws {
        guard let p = ptr else { throw GhostLayerError("builder already finished") }
        image.withUnsafeBytes { buf in
            let base = buf.bindMemory(to: UInt8.self).baseAddress
            if let json {
                json.withCString {
                    ghost_layer_doc_add_image_page(p, base, UInt(buf.count), widthPx, heightPx, dpi, $0)
                }
            } else {
                ghost_layer_doc_add_image_page(p, base, UInt(buf.count), widthPx, heightPx, dpi, nil)
            }
        }
        if let errPtr = pdf_get_last_error() {
            throw GhostLayerError(String(cString: errPtr))
        }
    }

    public func finish() throws -> Data {
        guard let p = ptr else { throw GhostLayerError("builder already finished") }
        ptr = nil
        let buf = ghost_layer_doc_finish_images(p)
        defer { free_pdf_buffer(buf) }
        guard let dataPtr = buf.data else { throw GhostLayerError.last() }
        return Data(bytes: dataPtr, count: Int(buf.len))
    }

    public func finish(to url: URL) throws {
        guard let p = ptr else { throw GhostLayerError("builder already finished") }
        ptr = nil
        let rc = url.path.withCString { ghost_layer_doc_finish_images_to_path(p, $0) }
        if rc != 0 { throw GhostLayerError.last() }
    }
}

// MARK: - OcrDocBuilder

public final class OcrDocBuilder {
    private var ptr: OpaquePointer?

    public init() {
        ptr = ghost_layer_doc_new_ocr()
    }

    deinit {
        if let p = ptr { ghost_layer_doc_free(p) }
    }

    public func addPage(json: String? = nil) {
        guard let p = ptr else { return }
        if let json {
            json.withCString { ghost_layer_doc_add_ocr_page(p, $0) }
        } else {
            ghost_layer_doc_add_ocr_page(p, nil)
        }
    }

    public func finish(overlaying pdf: Data) throws -> Data {
        guard let p = ptr else { throw GhostLayerError("builder already finished") }
        ptr = nil
        let buf = pdf.withUnsafeBytes { pdfBuf -> PdfBuffer in
            ghost_layer_doc_finish_ocr(
                p,
                pdfBuf.bindMemory(to: UInt8.self).baseAddress,
                UInt(pdfBuf.count)
            )
        }
        defer { free_pdf_buffer(buf) }
        guard let dataPtr = buf.data else { throw GhostLayerError.last() }
        return Data(bytes: dataPtr, count: Int(buf.len))
    }

    public func finish(overlaying pdf: Data, to url: URL) throws {
        guard let p = ptr else { throw GhostLayerError("builder already finished") }
        ptr = nil
        let rc = pdf.withUnsafeBytes { pdfBuf -> Int32 in
            url.path.withCString {
                ghost_layer_doc_finish_ocr_to_path(
                    p,
                    pdfBuf.bindMemory(to: UInt8.self).baseAddress,
                    UInt(pdfBuf.count),
                    $0
                )
            }
        }
        if rc != 0 { throw GhostLayerError.last() }
    }
}

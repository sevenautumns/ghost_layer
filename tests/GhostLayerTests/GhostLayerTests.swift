@testable import GhostLayer
import ImageIO
import XCTest

final class GhostLayerTests: XCTestCase {
    private let fixtures: [(img: String, json: String)] = [
        ("en_ltr.png", "en_ltr.json"),
        ("ar_rtl.jpg", "ar_rtl.json"),
        ("jp_ltr.jpg", "jp_ltr.json"),
        ("jp_ttb.png", "jp_ttb.json"),
    ]

    private var testsDir: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    // MARK: - Error path tests

    func testImageBuilderThrowsOnEmpty() throws {
        let builder = ImageDocBuilder()
        XCTAssertThrowsError(try builder.finish()) { error in
            XCTAssert(error is GhostLayerError)
        }
    }

    func testImageBuilderThrowsAfterFinish() throws {
        let builder = ImageDocBuilder()
        _ = try? builder.finish()
        XCTAssertThrowsError(try builder.finish())
    }

    func testOcrBuilderThrowsOnInvalidPdf() throws {
        let builder = OcrDocBuilder()
        builder.addPage(json: nil)
        XCTAssertThrowsError(try builder.finish(overlaying: Data("not a pdf".utf8))) { error in
            XCTAssert(error is GhostLayerError)
        }
    }

    func testOcrBuilderThrowsAfterFinish() throws {
        let builder = OcrDocBuilder()
        _ = try? builder.finish(overlaying: Data("not a pdf".utf8))
        XCTAssertThrowsError(try builder.finish(overlaying: Data("not a pdf".utf8)))
    }

    // MARK: - Fixture-based integration tests

    func testImageBuilderFromFixtures() throws {
        let builder = ImageDocBuilder()
        for (imgName, jsonName) in fixtures {
            let imgData = try Data(contentsOf: testsDir.appendingPathComponent(imgName))
            let json = try String(contentsOf: testsDir.appendingPathComponent(jsonName), encoding: .utf8)
            let (w, h) = try imageDimensions(imgData)
            try builder.addPage(image: imgData, widthPx: w, heightPx: h, dpi: 300, json: json)
        }
        let pdf = try builder.finish()
        XCTAssertGreaterThan(pdf.count, 1000)
    }

    func testOcrBuilderFromFixtures() throws {
        let imagePdf = try buildImagePdf()
        let builder = OcrDocBuilder()
        for (_, jsonName) in fixtures {
            let json = try String(contentsOf: testsDir.appendingPathComponent(jsonName), encoding: .utf8)
            builder.addPage(json: json)
        }
        let ocrPdf = try builder.finish(overlaying: imagePdf)
        XCTAssertGreaterThan(ocrPdf.count, 1000)
    }

    // MARK: - Helpers

    private func buildImagePdf() throws -> Data {
        let builder = ImageDocBuilder()
        for (imgName, jsonName) in fixtures {
            let imgData = try Data(contentsOf: testsDir.appendingPathComponent(imgName))
            let json = try String(contentsOf: testsDir.appendingPathComponent(jsonName), encoding: .utf8)
            let (w, h) = try imageDimensions(imgData)
            try builder.addPage(image: imgData, widthPx: w, heightPx: h, dpi: 300, json: json)
        }
        return try builder.finish()
    }

    private func imageDimensions(_ data: Data) throws -> (UInt32, UInt32) {
        let src = try XCTUnwrap(CGImageSourceCreateWithData(data as CFData, nil))
        let props = try XCTUnwrap(CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any])
        let w = try XCTUnwrap(props[kCGImagePropertyPixelWidth] as? UInt32)
        let h = try XCTUnwrap(props[kCGImagePropertyPixelHeight] as? UInt32)
        return (w, h)
    }
}

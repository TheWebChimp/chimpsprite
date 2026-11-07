// ESQUELETO sin compilar: la extensión de miniaturas de Quick Look para los .chimp (ver
// docs/thumbnails.md). Necesita un target "Thumbnail Extension" de Xcode dentro del .app, con
// chimp_thumbnail.h como bridging header y libchimpsprite_thumbnails.a enlazada.

import CoreGraphics
import Foundation
import ImageIO
import QuickLookThumbnailing

class ThumbnailProvider: QLThumbnailProvider {
    override func provideThumbnail(
        for request: QLFileThumbnailRequest,
        _ handler: @escaping (QLThumbnailReply?, Error?) -> Void
    ) {
        guard let image = Self.embeddedThumbnail(of: request.fileURL) else {
            // Sin miniatura (un archivo viejo): Finder muestra el ícono normal.
            handler(nil, nil)
            return
        }
        // Escala que cabe en lo que pide Finder, sin suavizar (pixel art).
        let maximum = request.maximumSize
        let scale = min(maximum.width / CGFloat(image.width), maximum.height / CGFloat(image.height))
        let size = CGSize(width: CGFloat(image.width) * scale, height: CGFloat(image.height) * scale)
        let reply = QLThumbnailReply(contextSize: size) { (context: CGContext) -> Bool in
            context.interpolationQuality = .none
            context.draw(image, in: CGRect(origin: .zero, size: size))
            return true
        }
        handler(reply, nil)
    }

    /// El thumbnail.png del zip, por la función C de chimpsprite-thumbnails.
    static func embeddedThumbnail(of url: URL) -> CGImage? {
        guard let data = try? Data(contentsOf: url, options: .mappedIfSafe) else { return nil }
        var out: UnsafeMutablePointer<UInt8>? = nil
        var outLen = 0
        let code = data.withUnsafeBytes { raw in
            chimp_thumbnail_png(raw.bindMemory(to: UInt8.self).baseAddress, raw.count, &out, &outLen)
        }
        guard code == CHIMP_THUMBNAIL_OK, let png = out else { return nil }
        let bytes = Data(bytes: png, count: outLen)
        chimp_thumbnail_free(png, outLen)
        guard let source = CGImageSourceCreateWithData(bytes as CFData, nil) else { return nil }
        return CGImageSourceCreateImageAtIndex(source, 0, nil)
    }
}

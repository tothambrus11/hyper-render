//! Integration tests for network-backed resource loading (the `net` feature).
//!
//! These deliberately use `data:` URIs so the tests run fully offline and
//! deterministically while still exercising the real `blitz-net` provider and
//! the resource-loading loop in `render`.

#![cfg(all(feature = "net", feature = "png"))]

use hyper_render::{render, Config};

/// PNG header magic bytes.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Decode a PNG into `(width, height, rgba_bytes)`.
fn decode_png(data: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(data);
    let mut reader = decoder.read_info().expect("valid PNG");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("decode PNG frame");
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

/// Sample the RGBA value at the given pixel coordinate.
fn pixel_at(data: &[u8], x: u32, y: u32) -> [u8; 4] {
    let (width, _height, buf) = decode_png(data);
    let idx = ((y * width + x) * 4) as usize;
    [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]]
}

#[test]
fn test_external_stylesheet_via_data_uri_is_applied() {
    // The linked stylesheet (a `data:` URI fetched through the net provider)
    // gives `.box` its dimensions and a green background. If the resource is
    // not loaded, the element has no size/background and the pixel stays blank.
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <link rel="stylesheet"
                  href="data:text/css;base64,LmJveHt3aWR0aDo2NHB4O2hlaWdodDo2NHB4O2JhY2tncm91bmQ6cmdiKDAsMTI4LDApfQ==" />
        </head>
        <body style="margin: 0; padding: 0;">
            <div class="box"></div>
        </body>
        </html>
    "#;

    let config = Config::new().width(64).height(64).scale(1.0);
    let bytes = render(html, config).expect("render should succeed");
    assert!(bytes.starts_with(&PNG_SIGNATURE), "output should be a PNG");

    let [r, g, b, a] = pixel_at(&bytes, 8, 8);
    assert!(
        r < 40 && (100..=160).contains(&g) && b < 40 && a > 200,
        "external stylesheet should paint a green box, got rgba({r},{g},{b},{a})"
    );
}

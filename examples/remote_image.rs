//! Example demonstrating remote resource loading (the `net` feature).
//!
//! With the `net` feature enabled (on by default), hyper-render fetches
//! external resources referenced by the HTML — here an image loaded over
//! HTTPS — before rendering.
//!
//! Run with: `cargo run --example remote_image`
//!
//! Note: this example requires network access and the `net` feature. Build
//! without networking using `--no-default-features --features png`.

use hyper_render::{render, Config};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body { font-family: system-ui, sans-serif; margin: 0; padding: 32px; background: #0f172a; }
                .card { background: #1e293b; border-radius: 16px; padding: 24px; max-width: 360px; }
                h1 { color: #e2e8f0; font-size: 22px; margin: 0 0 16px 0; }
                img { display: block; border-radius: 12px; width: 100%; height: auto; }
            </style>
        </head>
        <body>
            <div class="card">
                <h1>Fetched over HTTPS</h1>
                <img src="https://picsum.photos/id/237/360/240" alt="Remote image" />
            </div>
        </body>
        </html>
    "#;

    println!("Rendering (fetching remote image)...");
    let config = Config::new().size(440, 360).scale(2.0);
    let png_bytes = render(html, config)?;

    std::fs::write("remote_image.png", &png_bytes)?;
    println!("Saved remote_image.png ({} bytes)", png_bytes.len());

    Ok(())
}

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Remote resource loading via the optional `net` feature (enabled by default),
  powered by [`blitz-net`]. Images, linked stylesheets, `@import`s, and web
  fonts referenced by the HTML are now fetched and applied before rendering.
  - Supported URL schemes: `http(s)://`, `file://`, and `data:`.
  - `render` remains synchronous: it transparently uses a shared, process-wide
    Tokio runtime (created once and reused across calls) and blocks until all
    resources are loaded (subject to an internal timeout).
- `remote_image` example demonstrating remote image loading.
- Offline integration test (`tests/render_net.rs`) covering resource loading
  via `data:` URIs.

### Changed

- Disabling the `net` feature builds with no async or TLS dependencies.

### Notes

- The `net` feature uses `reqwest` with the native TLS backend, which requires
  OpenSSL development headers at build time.

[`blitz-net`]: https://crates.io/crates/blitz-net

## [0.1.0] - 2024-01-29

### Added

- Initial release of hyper-render
- PNG rendering via Vello CPU rasterizer
- PDF rendering via Krilla with vector graphics
- HTML/CSS parsing using Blitz (html5ever + Stylo)
- Flexbox and Grid layout support via Taffy
- Configuration options:
  - Viewport dimensions (width, height)
  - Scale factor for HiDPI displays
  - Output format selection (PNG/PDF)
  - Color scheme preference (light/dark)
  - Auto-height detection for content
  - Custom background colors with transparency support
- Feature flags for optional PNG and PDF support
- Comprehensive error handling with descriptive messages
- Configuration validation (dimensions, scale)
- Font embedding in PDF output
- Background color rendering for all elements

### Known Limitations

- No JavaScript support (by design)
- System fonts only (`@font-face` not yet supported)
- External image loading not yet implemented
- HTML parser may emit warnings for non-standard CSS properties

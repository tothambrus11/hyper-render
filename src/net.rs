//! Network resource loading via [`blitz_net`].
//!
//! Blitz fetches external resources (images, linked stylesheets, web fonts,
//! CSS `@import`s, etc.) asynchronously through a [`NetProvider`]. This module
//! wraps `blitz-net`'s [`Provider`] together with a process-wide Tokio runtime
//! and provides a synchronous "fetch everything, then return" helper so the rest
//! of the crate can stay fully synchronous.
//!
//! The runtime is created lazily **once** and shared across every `render` call
//! (see [`shared_runtime`]); building a runtime is comparatively expensive, so
//! reusing it keeps per-render overhead to just a channel and a provider. A
//! multi-threaded runtime is used so concurrent renders can fetch in parallel.

use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use blitz_dom::net::Resource;
use blitz_html::HtmlDocument;
use blitz_net::{MpscCallback, Provider};
use blitz_traits::net::{NetProvider, SharedCallback};
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::UnboundedReceiver;

/// How long to wait for a single resource before re-checking whether any
/// requests are still in flight. Kept short so termination is responsive.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Overall safety budget for the resource-loading phase. Guards against a hung
/// connection keeping a request in flight forever (reqwest has no default timeout).
const OVERALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Returns the process-wide Tokio runtime, creating it on first use.
///
/// The runtime lives for the remainder of the process; reusing it avoids the
/// cost of spinning up a new runtime (and worker threads) on every render.
fn shared_runtime() -> std::io::Result<&'static Runtime> {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();

    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }

    // Build outside `get_or_init` so initialization errors can be propagated.
    // On the rare init race the loser's runtime is simply dropped.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    Ok(RUNTIME.get_or_init(|| runtime))
}

/// Holds a handle to the shared runtime, the `blitz-net` provider, and the
/// channel that receives parsed resources back from in-flight requests.
pub(crate) struct NetEnv {
    handle: Handle,
    provider: Arc<Provider<Resource>>,
    receiver: UnboundedReceiver<(usize, Resource)>,
}

impl NetEnv {
    /// Wire a network provider to the shared process-wide runtime.
    pub(crate) fn new() -> std::io::Result<Self> {
        let handle = shared_runtime()?.handle().clone();

        let (receiver, callback) = MpscCallback::new();
        let callback: SharedCallback<Resource> = Arc::new(callback);

        // `Provider::new` captures `Handle::current()`, so it must be created
        // while the runtime is entered.
        let provider = {
            let _guard = handle.enter();
            Arc::new(Provider::new(callback))
        };

        Ok(Self {
            handle,
            provider,
            receiver,
        })
    }

    /// The provider to install on the document's [`DocumentConfig`].
    pub(crate) fn provider(&self) -> Arc<dyn NetProvider<Resource>> {
        self.provider.clone()
    }

    /// Block until every in-flight resource request has completed (or the
    /// overall timeout elapses), applying each fetched resource to `document`.
    ///
    /// Applying a resource (e.g. a stylesheet) and re-resolving layout can
    /// trigger further requests (fonts, `@import`s, background images), so this
    /// loops until the provider reports no outstanding requests.
    pub(crate) fn load_resources(&mut self, document: &mut HtmlDocument) {
        let deadline = Instant::now() + OVERALL_TIMEOUT;

        loop {
            // Apply everything that has already arrived.
            let mut changed = false;
            while let Ok((_doc_id, resource)) = self.receiver.try_recv() {
                document.load_resource(resource);
                changed = true;
            }

            // Nothing ready but requests are still in flight: wait for the next.
            if !changed && !self.provider.is_empty() && Instant::now() < deadline {
                let handle = &self.handle;
                let rx = &mut self.receiver;
                let received = handle
                    .block_on(async { tokio::time::timeout(POLL_INTERVAL, rx.recv()).await });
                if let Ok(Some((_doc_id, resource))) = received {
                    document.load_resource(resource);
                    changed = true;
                }
            }

            if changed {
                // New resources may have invalidated layout and/or queued more
                // requests; re-resolve so those get dispatched.
                document.resolve(0.0);
            } else if self.provider.is_empty() || Instant::now() >= deadline {
                break;
            }
        }

        document.resolve(0.0);
    }
}

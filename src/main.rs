//! Mark: a small Markdown viewer for Omarchy.
//!
//! The document is served to an embedded WebKit view over a private `mark://`
//! scheme whose URL path is the file's absolute path, so relative image paths
//! in the Markdown resolve next to the file without any extra plumbing.

mod render;
mod theme;

use notify::{EventKind, RecursiveMode, Watcher, event::AccessKind, event::AccessMode, event::ModifyKind};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::unix::WindowExtUnix;
use tao::window::WindowBuilder;
use wry::http::{Request, Response, header::CONTENT_TYPE};
use wry::{WebViewBuilder, WebViewBuilderExtUnix};

const APP_ID: &str = "mark";

#[derive(Debug)]
enum UserEvent {
    DocChanged,
    ThemeChanged,
    Reload,
    Quit,
}

/// Everything the page renderer needs, shared with the protocol handler,
/// which runs on its own terms rather than inside the event loop.
struct Doc {
    path: Option<PathBuf>,
    theme_dir: Option<PathBuf>,
    radius: u32,
}

impl Doc {
    fn title(&self) -> String {
        match self.path.as_ref().and_then(|p| p.file_name()) {
            Some(name) => format!("{} — Mark", name.to_string_lossy()),
            None => "Mark".into(),
        }
    }

    fn theme_css(&self) -> String {
        theme::css(self.theme_dir.as_deref(), self.radius)
    }

    /// The rendered Markdown, or a message saying what went wrong. Problems
    /// are shown in the window rather than leaving it blank.
    fn body(&self) -> String {
        let Some(path) = &self.path else {
            return "<h1>Mark</h1><p>No file open. Run <code>mark FILE.md</code>, \
                    or open a Markdown file from the file manager.</p>"
                .into();
        };
        match std::fs::read_to_string(path) {
            Ok(source) => render::body(&source),
            Err(err) => format!(
                "<h1>Can't open file</h1><p><code>{}</code></p><p>{}</p>",
                render::escape(&path.display().to_string()),
                render::escape(&err.to_string()),
            ),
        }
    }

    fn page(&self) -> String {
        render::page(&self.title(), &self.body(), &self.theme_css())
    }

    fn url(&self) -> String {
        match &self.path {
            Some(p) => format!("mark://localhost{}", encode_path(p)),
            None => "mark://localhost/".into(),
        }
    }
}

fn main() {
    let arg = std::env::args_os().nth(1);
    match arg.as_ref().and_then(|a| a.to_str()) {
        Some("-h" | "--help") => {
            println!("Usage: mark [FILE.md]\n\nA Markdown viewer. Press ? in the window for shortcuts.");
            return;
        }
        Some("-V" | "--version") => {
            println!("mark {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        _ => {}
    }

    // File managers pass file:// URIs when a .desktop file uses %u; accept both.
    let path = arg.map(|a| {
        let p = match a.to_str().and_then(|s| s.strip_prefix("file://")) {
            Some(rest) => PathBuf::from(decode(rest)),
            None => PathBuf::from(a),
        };
        std::fs::canonicalize(&p).unwrap_or_else(|_| std::path::absolute(&p).unwrap_or(p))
    });

    let doc = Arc::new(Doc { path, theme_dir: theme::state_dir(), radius: theme::hypr_radius() });

    // GTK uses the program name as the Wayland app id, which is how Hyprland
    // and the launcher match the window to mark.desktop and its icon.
    gtk::glib::set_prgname(Some(APP_ID));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title(doc.title())
        .with_inner_size(tao::dpi::LogicalSize::new(900.0, 1000.0))
        .build(&event_loop)
        .expect("failed to create window");

    let start_url = doc.url();
    let protocol_doc = doc.clone();
    let ipc_proxy = proxy.clone();
    let nav_url = start_url.clone();

    let webview = WebViewBuilder::new()
        .with_url(&start_url)
        .with_background_color(bg_rgba(&doc.theme_css()))
        .with_custom_protocol("mark".into(), move |_id, request| serve(&protocol_doc, &request))
        .with_ipc_handler(move |req: Request<String>| {
            let event = match req.body().as_str() {
                "quit" => UserEvent::Quit,
                "reload" => UserEvent::Reload,
                _ => return,
            };
            let _ = ipc_proxy.send_event(event);
        })
        .with_navigation_handler(move |url| allow_navigation(&nav_url, &url))
        .build_gtk(window.default_vbox().expect("tao window has no GTK box"))
        .expect("failed to create web view");

    // Keep the watcher alive for the life of the event loop.
    let _watcher = watch(&doc, proxy);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } | Event::UserEvent(UserEvent::Quit) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::DocChanged | UserEvent::Reload) => {
                let html = serde_json::to_string(&doc.body()).unwrap_or_default();
                let _ = webview.evaluate_script(&format!("window.markUpdate({html})"));
            }
            Event::UserEvent(UserEvent::ThemeChanged) => {
                let css = serde_json::to_string(&doc.theme_css()).unwrap_or_default();
                let _ = webview.evaluate_script(&format!("window.markTheme({css})"));
            }
            _ => {}
        }
    });
}

/// Watches the document's directory (editors often save by writing a new file
/// and renaming it over the old one, which a watch on the file itself would
/// miss) and Omarchy's current theme.
fn watch(doc: &Doc, proxy: tao::event_loop::EventLoopProxy<UserEvent>) -> Option<notify::RecommendedWatcher> {
    let doc_path = doc.path.clone();
    let theme_dir = doc.theme_dir.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        let is_write = match event.kind {
            EventKind::Create(_) => true,
            EventKind::Modify(ModifyKind::Metadata(_)) => false,
            EventKind::Modify(_) => true,
            EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
            _ => false,
        };
        if !is_write {
            return;
        }
        for p in &event.paths {
            if doc_path.as_deref() == Some(p.as_path()) {
                let _ = proxy.send_event(UserEvent::DocChanged);
            } else if theme_dir.as_deref().is_some_and(|t| p.starts_with(t)) {
                let _ = proxy.send_event(UserEvent::ThemeChanged);
            }
        }
    })
    .map_err(|e| eprintln!("mark: can't watch for changes: {e}"))
    .ok()?;

    if let Some(dir) = doc.path.as_deref().and_then(Path::parent) {
        if let Err(e) = watcher.watch(dir, RecursiveMode::NonRecursive) {
            eprintln!("mark: can't watch {}: {e}", dir.display());
        }
    }
    if let Some(dir) = doc.theme_dir.as_deref().filter(|d| d.exists()) {
        let _ = watcher.watch(dir, RecursiveMode::Recursive);
    }
    Some(watcher)
}

/// Serves the rendered document at its own path, and images beside it so
/// relative `![](img.png)` links work. Nothing else on disk is exposed.
fn serve(doc: &Doc, request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = PathBuf::from(decode(request.uri().path()));
    let is_doc = match &doc.path {
        Some(p) => *p == path,
        None => path == Path::new("/"),
    };
    if is_doc {
        return respond(200, "text/html; charset=utf-8", doc.page().into_bytes());
    }
    match (image_mime(&path), std::fs::read(&path)) {
        (Some(mime), Ok(bytes)) => respond(200, mime, bytes),
        _ => respond(404, "text/plain", b"not found".to_vec()),
    }
}

fn respond(status: u16, mime: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, mime)
        .body(Cow::Owned(body))
        .expect("valid response")
}

/// Stays on the document (in-page `#anchor` links are fine), hands web links
/// to the browser, and ignores links to other local files.
fn allow_navigation(doc_url: &str, url: &str) -> bool {
    if url == "about:blank" || url.split('#').next() == Some(doc_url) {
        return true;
    }
    if url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:") {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    false
}

fn image_mime(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => return None,
    })
}

/// Picks the window's initial color out of the theme CSS so the window
/// doesn't flash white before the page loads.
fn bg_rgba(css: &str) -> (u8, u8, u8, u8) {
    let hex = css.split("--bg:#").nth(1).map(|s| &s[..6.min(s.len())]).unwrap_or("121212");
    let c = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("12"), 16).unwrap_or(0x12);
    (c(0), c(2), c(4), 255)
}

fn encode_path(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut out = String::new();
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(b) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_round_trip_through_urls() {
        let p = Path::new("/home/me/My Notes/ü #1.md");
        assert_eq!(decode(&encode_path(p)), p.to_str().unwrap());
    }

    #[test]
    fn navigation_stays_on_the_document() {
        let doc = "mark://localhost/a/b.md";
        assert!(allow_navigation(doc, doc));
        assert!(allow_navigation(doc, "mark://localhost/a/b.md#section"));
        assert!(!allow_navigation(doc, "mark://localhost/a/other.md"));
    }

    #[test]
    fn window_color_comes_from_theme() {
        assert_eq!(bg_rgba(":root{--bg:#05182e;--bg-alt:#000000}"), (0x05, 0x18, 0x2e, 255));
    }
}

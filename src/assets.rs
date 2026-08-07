// Copyright 2026 Enactic, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Embedded browser assets.
//!
//! Upstream serves `src/dora_openarm_webxr/static/` with `FastAPI`'s
//! `StaticFiles(directory=..., html=True)`, which for this four-file
//! directory means: `/` and `/index.html` serve `index.html`, and
//! `/ar.js`, `/panel.js` and `/stereo.js` serve their matching file.
//! `ar.js` is an ES module and imports the other two by name, so all three
//! have to be reachable at those paths. Every file is Enactic-authored (no
//! CDN or bundled third-party front-end dependencies) and is embedded
//! byte-for-byte via `include_str!` rather than read from disk at runtime,
//! so the binary is self-contained.

/// `static/index.html`, byte-identical to upstream
/// `src/dora_openarm_webxr/static/index.html`.
pub const INDEX_HTML: &str = include_str!("../static/index.html");

/// `static/ar.js`, byte-identical to upstream `src/dora_openarm_webxr/static/ar.js`.
pub const AR_JS: &str = include_str!("../static/ar.js");

/// `static/panel.js`, byte-identical to upstream
/// `src/dora_openarm_webxr/static/panel.js`: the room-fixed head camera
/// panel of the default `fixed` view.
pub const PANEL_JS: &str = include_str!("../static/panel.js");

/// `static/stereo.js`, byte-identical to upstream
/// `src/dora_openarm_webxr/static/stereo.js`: the head-locked one-image-per-eye
/// panel of the `stereo` view.
pub const STEREO_JS: &str = include_str!("../static/stereo.js");

/// The `Content-Type` served for `index.html`.
pub const INDEX_HTML_CONTENT_TYPE: &str = "text/html; charset=utf-8";

/// The `Content-Type` served for `ar.js`, `panel.js` and `stereo.js`.
///
/// Starlette's `StaticFiles` guesses this from the platform's `mimetypes`
/// database, which can return either `text/javascript` or
/// `application/javascript` depending on the host. `text/javascript` is the
/// value recommended by RFC 9239 and what current Python `mimetypes`
/// returns; this is not independently verified against every possible
/// upstream deployment host.
pub const JAVASCRIPT_CONTENT_TYPE: &str = "text/javascript; charset=utf-8";

/// Backwards-compatible name retained for callers that only serve `ar.js`.
pub const AR_JS_CONTENT_TYPE: &str = JAVASCRIPT_CONTENT_TYPE;

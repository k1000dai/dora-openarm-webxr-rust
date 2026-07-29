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
//! `StaticFiles(directory=..., html=True)`, which for this two-file
//! directory means: `/` and `/index.html` serve `index.html`, and `/ar.js`
//! serves `ar.js`. Both files are Enactic-authored (no CDN or bundled
//! third-party front-end dependencies) and are embedded byte-for-byte via
//! `include_str!` rather than read from disk at runtime, so the binary is
//! self-contained.

/// `static/index.html`, byte-identical to upstream
/// `src/dora_openarm_webxr/static/index.html`.
pub const INDEX_HTML: &str = include_str!("../static/index.html");

/// `static/ar.js`, byte-identical to upstream `src/dora_openarm_webxr/static/ar.js`.
pub const AR_JS: &str = include_str!("../static/ar.js");

/// The `Content-Type` served for `index.html`.
pub const INDEX_HTML_CONTENT_TYPE: &str = "text/html; charset=utf-8";

/// The `Content-Type` served for `ar.js`.
///
/// Starlette's `StaticFiles` guesses this from the platform's `mimetypes`
/// database, which can return either `text/javascript` or
/// `application/javascript` depending on the host. `text/javascript` is the
/// value recommended by RFC 9239 and what current Python `mimetypes`
/// returns; this is not independently verified against every possible
/// upstream deployment host.
pub const AR_JS_CONTENT_TYPE: &str = "text/javascript; charset=utf-8";

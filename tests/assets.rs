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

//! The embedded browser assets (`static/index.html`, `static/ar.js`) must be
//! byte-identical to upstream
//! `src/dora_openarm_webxr/static/{index.html,ar.js}` -- this crate ships no
//! other front-end code, and modifying either file would change the `WebXR`
//! protocol the browser speaks.
//!
//! Byte-for-byte identity against the upstream checkout was verified with
//! `diff` while vendoring these files (see the port's development notes);
//! these tests instead pin content that would break if the embed were
//! accidentally truncated, re-encoded, or edited, without depending on the
//! upstream checkout being present at build/test time.

use dora_openarm_webxr_rust::assets::{
    AR_JS, AR_JS_CONTENT_TYPE, INDEX_HTML, INDEX_HTML_CONTENT_TYPE,
};

#[test]
fn index_html_matches_upstream_byte_length_and_key_markup() {
    assert_eq!(
        INDEX_HTML.len(),
        1081,
        "index.html byte length changed -- re-diff against upstream"
    );
    assert!(INDEX_HTML.contains("<title>OpenArm VR teleoperation</title>"));
    assert!(INDEX_HTML.contains(r#"<script type="module" src="ar.js"></script>"#));
    assert!(INDEX_HTML.contains(r#"<button id="start""#));
    assert!(INDEX_HTML.contains("Copyright 2026 Enactic, Inc."));
}

#[test]
fn ar_js_matches_upstream_byte_length_and_key_logic() {
    assert_eq!(
        AR_JS.len(),
        6352,
        "ar.js byte length changed -- re-diff against upstream"
    );
    assert!(AR_JS.contains(r#"new WebSocket("wss://" + location.host + "/websocket")"#));
    assert!(AR_JS.contains(r#"type: "session-start""#));
    assert!(AR_JS.contains(r#"type: "frame""#));
    assert!(AR_JS.contains("pico-4u"));
    assert!(AR_JS.contains("meta-quest-touch-plus"));
    assert!(AR_JS.contains(r#"requestReferenceSpace("viewer")"#));
}

#[test]
fn content_types_are_html_and_javascript() {
    assert_eq!(INDEX_HTML_CONTENT_TYPE, "text/html; charset=utf-8");
    assert_eq!(AR_JS_CONTENT_TYPE, "text/javascript; charset=utf-8");
}

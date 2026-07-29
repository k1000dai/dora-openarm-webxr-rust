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

//! Parsing of WebSocket JSON messages sent by `static/ar.js`, matching
//! upstream `main.py`'s `json.loads(data)` + `response["type"]` handling.

// Comparing against a literal that round-trips exactly through JSON f64
// deserialization is an intentional exact check, not a computed-value
// comparison.
#![allow(clippy::float_cmp)]

use dora_openarm_webxr_rust::protocol::{ClientMessage, parse_message};

#[test]
fn session_start_message() {
    let msg = parse_message(r#"{"type": "session-start"}"#).unwrap();
    assert_eq!(msg, ClientMessage::SessionStart);
}

#[test]
fn unknown_type_is_silently_ignored() {
    // Upstream's if/elif has no else branch: any other `type` value results
    // in no outputs sent, not an error.
    let msg = parse_message(r#"{"type": "select-start", "buttons": [], "axes": []}"#).unwrap();
    assert_eq!(msg, ClientMessage::Other);
}

#[test]
fn missing_type_field_is_an_error() {
    // Upstream does `response["type"]`, an unhandled `KeyError` if absent.
    assert!(parse_message(r#"{"time": 1.0}"#).is_err());
}

#[test]
fn invalid_json_is_an_error() {
    assert!(parse_message("not json").is_err());
}

#[test]
fn frame_message_with_full_payload() {
    let raw = r#"{
        "type": "frame",
        "time": 123.0,
        "pose_right": {"x": 0.1, "y": 0.2, "z": 0.3, "qx": 0.0, "qy": 0.0, "qz": 0.0, "qw": 1.0},
        "trigger_right": 0.5,
        "joystick_right": [0.1, 0.2, 0.3, 0.4],
        "button_a": true,
        "button_b": false
    }"#;
    let msg = parse_message(raw).unwrap();
    let ClientMessage::Frame(frame) = msg else {
        panic!("expected Frame")
    };
    let pose = frame.pose_right.expect("pose_right");
    assert_eq!(pose.x, 0.1);
    assert_eq!(pose.qw, 1.0);
    assert_eq!(frame.trigger_right, Some(0.5));
    assert_eq!(frame.joystick_right, Some([0.1, 0.2, 0.3, 0.4]));
    assert_eq!(frame.button_a, Some(true));
    assert_eq!(frame.button_b, Some(false));
    assert_eq!(frame.pose_left, None);
    assert_eq!(frame.trigger_left, None);
    assert_eq!(frame.joystick_left, None);
    assert_eq!(frame.button_x, None);
    assert_eq!(frame.button_y, None);
}

#[test]
fn frame_message_with_minimal_payload() {
    let msg = parse_message(r#"{"type": "frame", "time": 1.0}"#).unwrap();
    let ClientMessage::Frame(frame) = msg else {
        panic!("expected Frame")
    };
    assert_eq!(frame.pose_right, None);
    assert_eq!(frame.pose_left, None);
    assert_eq!(frame.trigger_right, None);
    assert_eq!(frame.joystick_right, None);
    assert_eq!(frame.button_a, None);
}

#[test]
fn joystick_axes_array_of_wrong_length_is_an_error() {
    // Real `ar.js` only ever sends 4-element `axes` arrays (see
    // `static/ar.js`'s `sendFrame`); a shorter array mirrors upstream's
    // unhandled `IndexError` on `axes[3]`.
    let raw = r#"{"type": "frame", "joystick_right": [0.1, 0.2, 0.3]}"#;
    assert!(parse_message(raw).is_err());
}

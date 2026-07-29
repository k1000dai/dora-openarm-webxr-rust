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

//! WebSocket JSON message parsing, matching upstream `main.py`'s
//! `json.loads(data)` and `response["type"]` dispatch.
//!
//! The only messages the server ever needs to understand are the ones
//! `static/ar.js` sends (`session-start`, `frame`); every other `type`
//! upstream silently ignores (its `if`/`elif` chain has no `else`). A
//! missing `type` key or a malformed `frame` payload (e.g. a wrong-length
//! `axes` array) is an unhandled Python exception upstream (`KeyError`,
//! `IndexError`); this is modeled as [`ParseError`] here so the caller can
//! decide how to end the connection, rather than silently swallowing it.

use serde::Deserialize;
use std::fmt;

/// A `WebXR` controller pose as received over the `WebSocket`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct PoseJson {
    /// X position in meters.
    pub x: f64,
    /// Y position in meters.
    pub y: f64,
    /// Z position in meters.
    pub z: f64,
    /// Quaternion X component.
    pub qx: f64,
    /// Quaternion Y component.
    pub qy: f64,
    /// Quaternion Z component.
    pub qz: f64,
    /// Quaternion W (scalar) component.
    pub qw: f64,
}

/// The `frame` message payload: everything is optional because `ar.js` only
/// includes a key when it has fresh data for it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct FrameMessage {
    /// The right controller's pose, present when its `gripSpace` resolved.
    #[serde(default)]
    pub pose_right: Option<PoseJson>,
    /// The left controller's pose, present when its `gripSpace` resolved.
    #[serde(default)]
    pub pose_left: Option<PoseJson>,
    /// The right trigger value, `0.0` (released) to `1.0` (fully pressed).
    #[serde(default)]
    pub trigger_right: Option<f64>,
    /// The left trigger value, `0.0` (released) to `1.0` (fully pressed).
    #[serde(default)]
    pub trigger_left: Option<f64>,
    /// The right joystick's 4 gamepad axes.
    #[serde(default)]
    pub joystick_right: Option<[f64; 4]>,
    /// The left joystick's 4 gamepad axes.
    #[serde(default)]
    pub joystick_left: Option<[f64; 4]>,
    /// The A button (right controller), present only while pressed state changes are reported.
    #[serde(default)]
    pub button_a: Option<bool>,
    /// The B button (right controller).
    #[serde(default)]
    pub button_b: Option<bool>,
    /// The X button (left controller).
    #[serde(default)]
    pub button_x: Option<bool>,
    /// The Y button (left controller).
    #[serde(default)]
    pub button_y: Option<bool>,
}

/// A parsed WebSocket message from the browser.
///
/// One message is processed at a time (never buffered in bulk), so the size
/// difference between `Frame` and the unit variants isn't a hot-path
/// concern worth boxing `FrameMessage` for.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ClientMessage {
    /// `{"type": "session-start"}`: a `WebXR` session was started.
    SessionStart,
    /// `{"type": "frame", ...}`: one `WebXR` animation frame.
    Frame(FrameMessage),
    /// Any other `type` value (e.g. `select-start`). Upstream's dispatch
    /// has no `else` branch, so these produce no outputs.
    Other,
}

/// A message that could not be parsed or dispatched.
///
/// Mirrors an unhandled exception in the upstream Python handler
/// (`KeyError` for a missing `type`, `IndexError` for a malformed `frame`
/// payload); the caller should treat this the same way upstream's crash
/// does -- ending the connection, not silently ignoring the message.
#[derive(Debug)]
pub enum ParseError {
    /// The text wasn't valid JSON.
    InvalidJson(serde_json::Error),
    /// The JSON value had no `type` field.
    MissingType,
    /// `type` was `"frame"` but the rest of the payload didn't match the
    /// expected shape (e.g. a joystick `axes` array without exactly 4 elements).
    InvalidFrame(serde_json::Error),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidJson(err) => write!(f, "invalid JSON: {err}"),
            ParseError::MissingType => write!(f, "message has no \"type\" field"),
            ParseError::InvalidFrame(err) => write!(f, "invalid frame payload: {err}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses a raw WebSocket text message into a [`ClientMessage`].
///
/// # Errors
///
/// Returns [`ParseError`] if the text isn't JSON, has no `type` field, or is
/// a `frame` message whose payload doesn't match the expected shape.
pub fn parse_message(raw: &str) -> Result<ClientMessage, ParseError> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(ParseError::InvalidJson)?;
    let message_type = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or(ParseError::MissingType)?;

    match message_type {
        "session-start" => Ok(ClientMessage::SessionStart),
        "frame" => {
            let frame: FrameMessage =
                serde_json::from_value(value).map_err(ParseError::InvalidFrame)?;
            Ok(ClientMessage::Frame(frame))
        }
        _ => Ok(ClientMessage::Other),
    }
}

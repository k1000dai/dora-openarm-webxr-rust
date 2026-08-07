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

//! Head camera video downlink for the `WebXR` front-end.
//!
//! Ports upstream `video.py`. The node takes the JPEG images of the robot's
//! head camera on the `camera_head_right`/`camera_head_left` dora inputs and
//! forwards them to the VR device over a second `WebSocket` (`/video`), so
//! frames never delay the pose messages that feed IK. How the image is drawn
//! is described by a YAML view configuration file, served verbatim as JSON
//! on `/view_configuration` and read once at startup.
//!
//! Everything here is pure: the frame store is a plain value and the
//! per-connection send decision is a method on it, so both are tested
//! without a running dataflow or a browser. The `axum`/dora plumbing that
//! drives them lives in `src/main.rs`.

use serde_json::{Value, json};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

/// Which eye a head camera frame is rendered on.
///
/// Ports upstream's `CAMERA_INPUTS`, the dora input id -> eye mapping. The
/// default `fixed` view uses only the right eye; the `stereo` view uses both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Eye {
    /// The left eye, fed by `camera_head_left`.
    Left,
    /// The right eye, fed by `camera_head_right`.
    Right,
}

impl Eye {
    /// Both eyes, in upstream's `CAMERA_INPUTS` order.
    pub const ALL: [Eye; 2] = [Eye::Left, Eye::Right];

    /// The dora input id that feeds this eye.
    #[must_use]
    pub const fn input_id(self) -> &'static str {
        match self {
            Eye::Left => "camera_head_left",
            Eye::Right => "camera_head_right",
        }
    }

    /// The one byte every `/video` binary message is prefixed with,
    /// matching upstream's `EYE_PREFIX` and the `RIGHT_EYE_PREFIX` /
    /// `EYE_BY_PREFIX` constants in `static/panel.js` and `static/stereo.js`.
    #[must_use]
    pub const fn prefix(self) -> u8 {
        match self {
            Eye::Left => 0x00,
            Eye::Right => 0x01,
        }
    }

    /// The eye a dora input id feeds, or `None` if the id isn't a head
    /// camera input.
    #[must_use]
    pub fn from_input_id(id: &str) -> Option<Self> {
        match id {
            "camera_head_left" => Some(Eye::Left),
            "camera_head_right" => Some(Eye::Right),
            _ => None,
        }
    }

    const fn index(self) -> usize {
        match self {
            Eye::Left => 0,
            Eye::Right => 1,
        }
    }
}

/// The view configuration read at startup, kept as the free-form document it
/// is upstream (`yaml.safe_load` into a `dict` that `FastAPI` serializes to
/// JSON unchanged).
///
/// Only the keys this node itself acts on (`view` and `pose.frame_offset`)
/// are interpreted here; everything else -- `session.mode`, `panel.*`,
/// `camera.*`, `stereo.*` -- is passed through to the front-end untouched,
/// so a new front-end key needs no change on this side.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewConfiguration {
    value: Value,
}

/// An error reading a view configuration.
#[derive(Debug)]
pub enum ViewConfigurationError {
    /// The file could not be opened or read.
    Io(std::io::Error),
    /// The file was not valid YAML.
    Yaml(serde_yaml::Error),
    /// The document parsed but wasn't a mapping (e.g. an empty file, which
    /// YAML reads as `null`, or a top-level list).
    NotAMapping,
    /// `pose.frame_offset` was present but wasn't three numbers.
    InvalidFrameOffset,
}

impl fmt::Display for ViewConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewConfigurationError::Io(err) => write!(f, "{err}"),
            ViewConfigurationError::Yaml(err) => write!(f, "{err}"),
            ViewConfigurationError::NotAMapping => {
                write!(f, "the document is not a mapping")
            }
            ViewConfigurationError::InvalidFrameOffset => {
                write!(f, "pose: frame_offset: must be three numbers")
            }
        }
    }
}

impl std::error::Error for ViewConfigurationError {}

impl Default for ViewConfiguration {
    /// Upstream's `DEFAULT_VIEW_CONFIGURATION`, used when no
    /// `--view-configuration-file` is given.
    fn default() -> Self {
        Self {
            value: json!({
                "view": "fixed",
                "session": {"mode": "immersive-ar"},
                "panel": {"distance": 1.3, "width": 1.5},
            }),
        }
    }
}

impl ViewConfiguration {
    /// Parses a view configuration from YAML text.
    ///
    /// # Errors
    ///
    /// Returns [`ViewConfigurationError`] if the text isn't valid YAML or
    /// isn't a mapping.
    pub fn from_yaml_str(yaml: &str) -> Result<Self, ViewConfigurationError> {
        let value: Value = serde_yaml::from_str(yaml).map_err(ViewConfigurationError::Yaml)?;
        if !value.is_object() {
            return Err(ViewConfigurationError::NotAMapping);
        }
        Ok(Self { value })
    }

    /// Reads the view configuration at `path`, or the default when `path` is
    /// `None`.
    ///
    /// Ports upstream `configure()`: read once at startup (restart the
    /// dataflow to apply a change), and keep the default -- after reporting
    /// the problem on stderr -- so a broken file cannot stop the node.
    ///
    /// Upstream only catches `OSError`/`YAMLError`, so a file that parses to
    /// something other than a mapping (an empty file, or a top-level list)
    /// takes down the node later, when `main()` reaches for its `pose` key.
    /// That case falls back to the default here instead, which is what the
    /// upstream comment asks for.
    #[must_use]
    pub fn load(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };
        match Self::read(path) {
            Ok(configuration) => configuration,
            Err(error) => {
                eprintln!("cannot read {}: {error}", path.display());
                Self::default()
            }
        }
    }

    fn read(path: &Path) -> Result<Self, ViewConfigurationError> {
        let text = std::fs::read_to_string(path).map_err(ViewConfigurationError::Io)?;
        Self::from_yaml_str(&text)
    }

    /// The document as served on `/view_configuration`.
    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// The `view` key, or `None` when it is absent or not a string.
    ///
    /// `static/ar.js` treats anything that isn't `"stereo"` or `"none"` --
    /// including an absent key -- as the default `fixed` view.
    #[must_use]
    pub fn view(&self) -> Option<&str> {
        self.value.get("view").and_then(Value::as_str)
    }

    /// Whether the head-locked stereo view is selected
    /// (`_view_configuration.get("view") == "stereo"`).
    #[must_use]
    pub fn is_stereo(&self) -> bool {
        self.view() == Some("stereo")
    }

    /// The eyes a `/video` connection streams: both for the stereo view, the
    /// right one alone otherwise.
    ///
    /// `view: none` is not special-cased, matching upstream: the front-end
    /// simply never opens `/video`.
    #[must_use]
    pub fn eyes(&self) -> Vec<Eye> {
        if self.is_stereo() {
            vec![Eye::Left, Eye::Right]
        } else {
            vec![Eye::Right]
        }
    }

    /// The `pose.frame_offset` override for the neutral hand position, or
    /// `None` when the file doesn't set one.
    ///
    /// Ports `(view_configuration().get("pose") or {}).get("frame_offset")`
    /// followed by `np.array(..., dtype=np.float32).reshape(3)`: a present
    /// but malformed value is a startup failure upstream, and an error here.
    ///
    /// # Errors
    ///
    /// Returns [`ViewConfigurationError::InvalidFrameOffset`] if
    /// `pose.frame_offset` is present but isn't a list of exactly three
    /// numbers.
    #[allow(clippy::cast_possible_truncation)] // `dtype=np.float32`.
    pub fn frame_offset(&self) -> Result<Option<[f32; 3]>, ViewConfigurationError> {
        let pose = self.value.get("pose");
        // `... or {}`: a missing or null `pose` is an empty mapping.
        if matches!(pose, None | Some(Value::Null)) {
            return Ok(None);
        }
        let Some(offset) = pose.and_then(|pose| pose.get("frame_offset")) else {
            return Ok(None);
        };
        let Some(items) = offset.as_array() else {
            return Err(ViewConfigurationError::InvalidFrameOffset);
        };
        if items.len() != 3 {
            return Err(ViewConfigurationError::InvalidFrameOffset);
        }
        let mut result = [0.0f32; 3];
        for (slot, item) in result.iter_mut().zip(items) {
            let value = item
                .as_f64()
                .ok_or(ViewConfigurationError::InvalidFrameOffset)?;
            *slot = value as f32;
        }
        Ok(Some(result))
    }
}

/// One head camera frame: the JPEG bytes plus the sequence number that tells
/// a repeated frame from a new one.
///
/// The bytes are shared rather than copied so that handing the newest frame
/// to every open `/video` connection costs a refcount bump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The JPEG data exactly as it arrived on the dora input.
    pub data: Arc<[u8]>,
    /// How many frames this eye has received, incremented on every store.
    pub sequence: u64,
}

/// The latest frame per eye.
///
/// Ports upstream's module-level `_frames`/`_sequences` dicts: one slot per
/// eye holding only the most recent frame, so a VR device that cannot keep
/// up skips frames instead of falling behind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameSet {
    frames: [Option<Frame>; 2],
}

impl FrameSet {
    /// An empty set, before any frame has arrived.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `data` as `eye`'s newest frame, replacing any previous one and
    /// incrementing that eye's sequence number.
    pub fn store(&mut self, eye: Eye, data: Arc<[u8]>) {
        let slot = &mut self.frames[eye.index()];
        let sequence = slot.as_ref().map_or(0, |frame| frame.sequence) + 1;
        *slot = Some(Frame { data, sequence });
    }

    /// The newest frame for `eye`, if one has arrived.
    #[must_use]
    pub fn get(&self, eye: Eye) -> Option<&Frame> {
        self.frames[eye.index()].as_ref()
    }
}

/// The send decision of one `/video` connection.
///
/// Ports the `sent` bookkeeping of upstream's `_video_endpoint`: a frame is
/// sent only once every eye the view needs has one, and only when at least
/// one of them is newer than what this connection last sent. All of the
/// view's eyes are sent together, so the eyes never show different frames.
#[derive(Debug, Clone)]
pub struct VideoStream {
    eyes: Vec<Eye>,
    // Upstream starts at -1 with sequences at 0; sequences here start at 1
    // on the first stored frame, and an eye without a frame is never
    // compared, so 0 is the same "nothing sent yet".
    sent: [u64; 2],
}

impl VideoStream {
    /// Builds the stream for `configuration`'s view: both eyes for `stereo`,
    /// the right eye alone otherwise.
    #[must_use]
    pub fn new(configuration: &ViewConfiguration) -> Self {
        Self::for_eyes(configuration.eyes())
    }

    /// Builds a stream for an explicit set of eyes.
    #[must_use]
    pub fn for_eyes(eyes: Vec<Eye>) -> Self {
        Self { eyes, sent: [0; 2] }
    }

    /// The eyes this stream sends.
    #[must_use]
    pub fn eyes(&self) -> &[Eye] {
        &self.eyes
    }

    /// The binary `/video` messages to send for the current `frames`, each
    /// one an [`Eye::prefix`] byte followed by the JPEG data.
    ///
    /// Returns an empty vector when there is nothing new to send; the caller
    /// then waits for the next frame.
    #[must_use]
    pub fn next_messages(&mut self, frames: &FrameSet) -> Vec<Vec<u8>> {
        let Some(pending) = self
            .eyes
            .iter()
            .map(|&eye| frames.get(eye).map(|frame| (eye, frame)))
            .collect::<Option<Vec<_>>>()
        else {
            // At least one eye has no frame yet.
            return Vec::new();
        };
        if pending
            .iter()
            .all(|(eye, frame)| frame.sequence == self.sent[eye.index()])
        {
            return Vec::new();
        }
        pending
            .into_iter()
            .map(|(eye, frame)| {
                self.sent[eye.index()] = frame.sequence;
                let mut message = Vec::with_capacity(frame.data.len() + 1);
                message.push(eye.prefix());
                message.extend_from_slice(&frame.data);
                message
            })
            .collect()
    }
}

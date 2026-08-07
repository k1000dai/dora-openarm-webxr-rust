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

//! Per-connection state and the pure decision logic of `_websocket_endpoint`.
//!
//! [`FrameProcessor`] holds the two One Euro smoothers upstream constructs
//! fresh per WebSocket connection (`smoothers = {"right": ..., "left":
//! ...}` inside `_websocket_endpoint`) and turns a parsed [`ClientMessage`]
//! into the ordered list of dora outputs it should produce. It knows
//! nothing about dora-rs, Arrow, or sockets -- the adapter in `main.rs`
//! walks the returned [`Emission`]s and sends them.

// Trigger/joystick `as f32` narrowing matches upstream's explicit
// `pa.array(..., type=pa.float32())` casts.
#![allow(clippy::cast_possible_truncation)]

use crate::protocol::{ClientMessage, FrameMessage, PoseJson};
use crate::smoothing::OneEuroPoseSmoother;
use crate::transform::{
    DEFAULT_FRAME_OFFSET_CELL, RawPose, Side, adjust_pose_with_offset, map_trigger_to_gripper,
};

/// Output ids, matching upstream `main.py` and `README.md` exactly.
pub mod output_id {
    /// `"ready"` when a `WebXR` session is started.
    pub const STATUS: &str = "status";
    /// Timestamp (ns) when a frame is received from the VR device.
    pub const VR_RECEIVE_TIMES: &str = "vr_receive_times";
    /// Whether the A button is pressed.
    pub const BUTTON_A: &str = "button_a";
    /// Whether the B button is pressed.
    pub const BUTTON_B: &str = "button_b";
    /// Whether the X button is pressed.
    pub const BUTTON_X: &str = "button_x";
    /// Whether the Y button is pressed.
    pub const BUTTON_Y: &str = "button_y";
    /// The right controller pose plus gripper angle.
    pub const POSE_RIGHT: &str = "pose_right";
    /// The left controller pose plus gripper angle.
    pub const POSE_LEFT: &str = "pose_left";
    /// The right trigger value.
    pub const TRIGGER_RIGHT: &str = "trigger_right";
    /// The left trigger value.
    pub const TRIGGER_LEFT: &str = "trigger_left";
    /// The right joystick X axis.
    pub const JOYSTICK_X_RIGHT: &str = "joystick_x_right";
    /// The right joystick Y axis.
    pub const JOYSTICK_Y_RIGHT: &str = "joystick_y_right";
    /// The left joystick X axis.
    pub const JOYSTICK_X_LEFT: &str = "joystick_x_left";
    /// The left joystick Y axis.
    pub const JOYSTICK_Y_LEFT: &str = "joystick_y_left";
}

/// The value of one dora output, independent of how it gets encoded as Arrow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmissionValue {
    /// A status string (only ever `"ready"`).
    Status(&'static str),
    /// A nanosecond timestamp.
    Int64(i64),
    /// A button state.
    Bool(bool),
    /// A trigger value or joystick axis.
    Float32(f32),
    /// `[x, y, z, qw, qx, qy, qz, gripper]`.
    PoseWithGripper([f32; 8]),
}

/// One dora output produced while processing a single WebSocket message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emission {
    /// The dora output id.
    pub output_id: &'static str,
    /// The value to send.
    pub value: EmissionValue,
}

fn emit(emissions: &mut Vec<Emission>, output_id: &'static str, value: EmissionValue) {
    emissions.push(Emission { output_id, value });
}

/// Per-WebSocket-connection processing state: one One Euro smoother per
/// controller side, matching the `smoothers` dict upstream constructs fresh
/// for every new connection.
#[derive(Debug, Clone)]
pub struct FrameProcessor {
    right: OneEuroPoseSmoother,
    left: OneEuroPoseSmoother,
    frame_offset: [f32; 3],
}

impl Default for FrameProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameProcessor {
    /// Builds a fresh processor with the smoother parameters upstream uses
    /// in `_websocket_endpoint` (`min_cutoff=2.0, beta=0.04, d_cutoff=1.5`).
    #[must_use]
    pub fn new() -> Self {
        Self::with_frame_offset(DEFAULT_FRAME_OFFSET_CELL)
    }

    /// Builds a fresh processor with a custom neutral hand offset.
    #[must_use]
    pub fn with_frame_offset(frame_offset: [f32; 3]) -> Self {
        Self {
            right: OneEuroPoseSmoother::new(2.0, 0.04, 1.5),
            left: OneEuroPoseSmoother::new(2.0, 0.04, 1.5),
            frame_offset,
        }
    }

    /// Processes one parsed WebSocket message, returning the dora outputs
    /// to send in emission order.
    ///
    /// `timestamp_ns` is `time.time_ns()`'s equivalent, used both as the
    /// `vr_receive_times` payload and as every emission's `timestamp`
    /// metadata. `smoother_time` is `time.perf_counter()`'s equivalent, fed
    /// to the One Euro filters.
    #[must_use]
    pub fn process(
        &mut self,
        message: &ClientMessage,
        timestamp_ns: i64,
        smoother_time: f64,
    ) -> Vec<Emission> {
        match message {
            ClientMessage::SessionStart => vec![Emission {
                output_id: output_id::STATUS,
                value: EmissionValue::Status("ready"),
            }],
            ClientMessage::Frame(frame) => self.process_frame(frame, timestamp_ns, smoother_time),
            ClientMessage::Other => Vec::new(),
        }
    }

    fn process_frame(
        &mut self,
        frame: &FrameMessage,
        timestamp_ns: i64,
        smoother_time: f64,
    ) -> Vec<Emission> {
        let mut emissions = Vec::new();
        emit(
            &mut emissions,
            output_id::VR_RECEIVE_TIMES,
            EmissionValue::Int64(timestamp_ns),
        );

        if let Some(v) = frame.button_a {
            emit(&mut emissions, output_id::BUTTON_A, EmissionValue::Bool(v));
        }
        if let Some(v) = frame.button_b {
            emit(&mut emissions, output_id::BUTTON_B, EmissionValue::Bool(v));
        }
        if let Some(v) = frame.button_x {
            emit(&mut emissions, output_id::BUTTON_X, EmissionValue::Bool(v));
        }
        if let Some(v) = frame.button_y {
            emit(&mut emissions, output_id::BUTTON_Y, EmissionValue::Bool(v));
        }

        Self::process_side(
            &mut self.right,
            Side::Right,
            frame.pose_right,
            frame.trigger_right,
            frame.joystick_right,
            smoother_time,
            output_id::POSE_RIGHT,
            output_id::TRIGGER_RIGHT,
            output_id::JOYSTICK_X_RIGHT,
            output_id::JOYSTICK_Y_RIGHT,
            self.frame_offset,
            &mut emissions,
        );
        Self::process_side(
            &mut self.left,
            Side::Left,
            frame.pose_left,
            frame.trigger_left,
            frame.joystick_left,
            smoother_time,
            output_id::POSE_LEFT,
            output_id::TRIGGER_LEFT,
            output_id::JOYSTICK_X_LEFT,
            output_id::JOYSTICK_Y_LEFT,
            self.frame_offset,
            &mut emissions,
        );

        emissions
    }

    // `joystick_x_output_id`/`joystick_y_output_id` and `smoother`/`smoothed`
    // are meaningfully distinct, not a naming mistake.
    #[allow(clippy::too_many_arguments, clippy::similar_names)]
    fn process_side(
        smoother: &mut OneEuroPoseSmoother,
        side: Side,
        pose: Option<PoseJson>,
        trigger: Option<f64>,
        joystick: Option<[f64; 4]>,
        smoother_time: f64,
        pose_output_id: &'static str,
        trigger_output_id: &'static str,
        joystick_x_output_id: &'static str,
        joystick_y_output_id: &'static str,
        frame_offset: [f32; 3],
        emissions: &mut Vec<Emission>,
    ) {
        if let (Some(pose), Some(trigger)) = (pose, trigger) {
            let raw_pose = RawPose {
                x: pose.x,
                y: pose.y,
                z: pose.z,
                qx: pose.qx,
                qy: pose.qy,
                qz: pose.qz,
                qw: pose.qw,
            };
            let adjusted = adjust_pose_with_offset(&raw_pose, frame_offset);
            let smoothed = smoother.smooth(smoother_time, adjusted);
            let gripper = map_trigger_to_gripper(trigger, side) as f32;
            let mut pose_with_gripper = [0.0f32; 8];
            pose_with_gripper[..7].copy_from_slice(&smoothed);
            pose_with_gripper[7] = gripper;
            emit(
                emissions,
                pose_output_id,
                EmissionValue::PoseWithGripper(pose_with_gripper),
            );
        }
        if let Some(trigger) = trigger {
            emit(
                emissions,
                trigger_output_id,
                EmissionValue::Float32(trigger as f32),
            );
        }
        if let Some(axes) = joystick {
            let x = (axes[1] - axes[3]) as f32;
            let y = (axes[2] - axes[0]) as f32;
            emit(emissions, joystick_x_output_id, EmissionValue::Float32(x));
            emit(emissions, joystick_y_output_id, EmissionValue::Float32(y));
        }
    }
}

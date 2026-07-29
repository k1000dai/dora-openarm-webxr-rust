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

//! `WebXR` pose -> `OpenArm` workspace transform.
//!
//! Ports `_adjust_pose` and `_map_trigger_to_gripper` from upstream
//! `main.py`. The rotation math runs in `f64`, matching scipy's internal
//! quaternion representation, and casts down to `f32` only for the final
//! packed pose -- the same dtype ladder the upstream numpy/scipy pipeline
//! uses (numpy promotes float32 arrays to float64 for `Rotation.apply`
//! results before the explicit `dtype=np.float32` cast at the end).

// Every `as f32` in this module is the deliberate final narrowing step of
// that dtype ladder, matching upstream's explicit `dtype=np.float32` casts.
#![allow(clippy::cast_possible_truncation)]

use crate::quaternion::Quaternion;

/// The quaternion (scalar-last) for `_ROBOT_ROTATION_MATRIX =
/// [[0,0,-1],[-1,0,0],[0,1,0]]`.
///
/// Hardcoded rather than derived via a generic matrix-to-quaternion
/// conversion: `scipy.spatial.transform.Rotation.from_matrix(...).as_quat()`
/// gives exactly `[0.5, -0.5, -0.5, 0.5]` for this matrix (verified with
/// scipy 1.18.0), and every component is exactly representable, so using
/// the literal avoids any risk of a from-matrix algorithm picking the
/// opposite sign (`-q` represents the same rotation but would flip every
/// composed output's sign).
const ROBOT_ROTATION_QUAT: Quaternion = Quaternion {
    x: 0.5,
    y: -0.5,
    z: -0.5,
    w: 0.5,
};

/// Neutral hand position relative to the `arm_origin` site (chest level),
/// stored at `f32` precision because upstream builds this constant with
/// `dtype=np.float32` before it is (numpy-promoted) added to the `f64`
/// rotated position.
const FRAME_OFFSET_CELL: [f32; 3] = [-0.085, 0.0, -0.14];

/// Which controller/arm side a value belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The right controller / right arm.
    Right,
    /// The left controller / left arm.
    Left,
}

/// A `WebXR` controller pose as received over the `WebSocket`, before any unit
/// conversion. Fields keep the upstream JSON key names.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPose {
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

/// Converts a `WebXR` pose into the `OpenArm` workspace pose `[x, y, z, qw, qx,
/// qy, qz]`, matching upstream `_adjust_pose`'s output layout (position in
/// meters, orientation scalar-first).
#[must_use]
pub fn adjust_pose(pose: &RawPose) -> [f32; 7] {
    // Cast to f32 first, exactly like `np.array([...], dtype=np.float32)`.
    let position_f32 = [pose.x as f32, pose.y as f32, pose.z as f32];
    let position_f64 = [
        f64::from(position_f32[0]),
        f64::from(position_f32[1]),
        f64::from(position_f32[2]),
    ];

    let rotated = ROBOT_ROTATION_QUAT.rotate_vector(position_f64);
    let offset = [
        f64::from(FRAME_OFFSET_CELL[0]),
        f64::from(FRAME_OFFSET_CELL[1]),
        f64::from(FRAME_OFFSET_CELL[2]),
    ];
    let position = [
        rotated[0] + offset[0],
        rotated[1] + offset[1],
        rotated[2] + offset[2],
    ];

    let input_quat = Quaternion::from_xyzw([pose.qx, pose.qy, pose.qz, pose.qw]).normalize();
    let rotation_fix = Quaternion::from_z_rotation_degrees(90.0);
    let combined = ROBOT_ROTATION_QUAT
        .hamilton_product(input_quat)
        .hamilton_product(rotation_fix);
    let [qx, qy, qz, qw] = combined.to_xyzw();

    [
        position[0] as f32,
        position[1] as f32,
        position[2] as f32,
        qw as f32,
        qx as f32,
        qy as f32,
        qz as f32,
    ]
}

/// Maps a trigger value (`0.0` released, `1.0` fully pressed) to a gripper
/// joint angle in radians, matching upstream `_map_trigger_to_gripper`.
///
/// Right: `0 -> -1.57/2`, `1 -> 0`. Left is the mirror image: `0 -> 1.57/2`,
/// `1 -> 0`. Neither side clips its input.
#[must_use]
pub fn map_trigger_to_gripper(trigger: f64, side: Side) -> f64 {
    match side {
        Side::Right => (-1.57 / 2.0) * (1.0 - trigger),
        Side::Left => (1.57 / 2.0) * (1.0 - trigger),
    }
}

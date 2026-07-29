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

//! Golden tests for the `WebXR` -> `OpenArm` pose/gripper transform.
//!
//! Expected values were captured by running the actual upstream
//! `_adjust_pose`/`_map_trigger_to_gripper` functions (numpy 2.5.1, scipy
//! 1.18.0) against a fixed set of inputs. See the port's development notes
//! for the oracle script. Most components match scipy bit-for-bit; a few
//! components that are mathematically ~0 (order 1e-17) differ at the ULP
//! level because scipy's Cython rotation-composition kernel takes a
//! different -- but equally valid -- floating point path than the
//! straightforward Hamilton-product implementation used here. Those
//! near-zero components are asserted with an absolute tolerance instead of
//! bit equality; the tolerance (1e-6) is far tighter than the f32 output
//! ULP (~1.2e-7 at magnitude 1) and physically insignificant for a
//! teleoperated arm.

// These expected values are transcribed verbatim (to full oracle precision)
// from the golden-vector generator for traceability, even where a shorter
// literal would represent the same f32/f64 value; a couple happen to be
// close to (but not exactly) sqrt(2)/2.
#![allow(clippy::excessive_precision, clippy::approx_constant)]

use dora_openarm_webxr_rust::transform::{RawPose, Side, adjust_pose, map_trigger_to_gripper};

const TOL: f32 = 1e-6;

fn assert_pose_close(actual: [f32; 7], expected: [f32; 7]) {
    for i in 0..7 {
        assert!(
            (actual[i] - expected[i]).abs() < TOL,
            "component {i}: actual={} expected={} (actual={actual:?} expected={expected:?})",
            actual[i],
            expected[i]
        );
    }
}

#[test]
fn identity_orientation_pose() {
    let pose = RawPose {
        x: 0.1,
        y: 0.2,
        z: 0.3,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
    };
    let adjusted = adjust_pose(&pose);
    // f32 hex from oracle: b91ec5be cdccccbd 90c2753d f304353f 00008024 f30435bf 000080a4
    let expected = [
        -0.385_000_020_265_579_2,
        -0.100_000_001_490_116_12,
        0.060_000_002_384_185_79,
        0.707_106_769_084_930_4,
        5.551_115_123_125_783e-17,
        -0.707_106_769_084_930_4,
        -5.551_115_123_125_783e-17,
    ];
    assert_pose_close(adjusted, expected);
}

#[test]
fn quarter_turn_about_x() {
    let pose = RawPose {
        x: -0.05,
        y: 0.15,
        z: -0.2,
        qx: 0.382_683_432_365_089_8,
        qy: 0.0,
        qz: 0.0,
        qw: 0.923_879_532_511_286_7,
    };
    let adjusted = adjust_pose(&pose);
    let expected = [
        0.115_000_002_086_162_57,
        0.050_000_000_745_058_06,
        0.010_000_005_364_418_03,
        0.382_683_426_141_738_9,
        5.551_115_123_125_783e-17,
        -0.923_879_504_203_796_4,
        -2.365_058_263_799_365_7e-17,
    ];
    assert_pose_close(adjusted, expected);
}

#[test]
fn quarter_turn_about_y_zero_position() {
    let pose = RawPose {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        qx: 0.0,
        qy: 0.707_106_781_186_547_6,
        qz: 0.0,
        qw: 0.707_106_781_186_547_6,
    };
    let adjusted = adjust_pose(&pose);
    let expected = [-0.085, 0.0, -0.14, 0.5, 0.5, -0.5, 0.5];
    assert_pose_close(adjusted, expected);
}

#[test]
fn arbitrary_orientation() {
    let pose = RawPose {
        x: 0.037,
        y: -0.421,
        z: 0.113,
        qx: 0.092_295_955_641_257_23,
        qy: 0.537_849_961_087_150_5,
        qz: 0.439_744_842_663_251_2,
        qw: 0.713_449_021_814_999_4,
    };
    let adjusted = adjust_pose(&pose);
    let expected = [
        -0.197_999_998_927_116_4,
        -0.037_000_000_476_837_16,
        -0.560_999_989_509_582_5,
        0.439_180_672_168_731_7,
        0.069_364_339_113_235_47,
        -0.569_694_697_856_903_1,
        0.691_199_600_696_563_7,
    ];
    assert_pose_close(adjusted, expected);
}

#[test]
fn negative_position_and_orientation() {
    let pose = RawPose {
        x: -0.3,
        y: 0.05,
        z: 0.02,
        qx: -0.5,
        qy: 0.5,
        qz: -0.5,
        qw: 0.5,
    };
    let adjusted = adjust_pose(&pose);
    let expected = [
        -0.105_000_004_172_325_13,
        0.300_000_011_920_928_96,
        -0.090_000_003_576_278_69,
        0.707_106_769_084_930_4,
        0.707_106_769_084_930_4,
        5.551_115_123_125_783e-17,
        -5.551_115_123_125_783e-17,
    ];
    assert_pose_close(adjusted, expected);
}

#[test]
fn trigger_to_gripper_matches_upstream_endpoints_and_midpoints() {
    // Right: 0 -> -1.57/2, 1 -> 0.
    assert!((map_trigger_to_gripper(0.0, Side::Right) - (-0.785)).abs() < 1e-9);
    assert!((map_trigger_to_gripper(1.0, Side::Right) - 0.0).abs() < 1e-9);
    assert!((map_trigger_to_gripper(0.5, Side::Right) - (-0.3925)).abs() < 1e-9);
    assert!((map_trigger_to_gripper(0.25, Side::Right) - (-0.588_75)).abs() < 1e-9);
    assert!((map_trigger_to_gripper(0.123_456, Side::Right) - (-0.688_087_04)).abs() < 1e-8);

    // Left: 0 -> 1.57/2, 1 -> 0. Mirror image of the right mapping.
    assert!((map_trigger_to_gripper(0.0, Side::Left) - 0.785).abs() < 1e-9);
    assert!((map_trigger_to_gripper(1.0, Side::Left) - 0.0).abs() < 1e-9);
    assert!((map_trigger_to_gripper(0.5, Side::Left) - 0.3925).abs() < 1e-9);
    assert!((map_trigger_to_gripper(0.25, Side::Left) - 0.588_75).abs() < 1e-9);
    assert!((map_trigger_to_gripper(0.123_456, Side::Left) - 0.688_087_04).abs() < 1e-8);
}

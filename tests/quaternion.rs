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

//! Golden tests for quaternion math, matching `scipy.spatial.transform.Rotation`
//! conventions (scalar-last `[x, y, z, w]`, Hamilton product where
//! `(p * q).apply(v) == p.apply(q.apply(v))`).

// `x, y, z, w` are the natural, self-documenting names for quaternion
// components; expected values are close to (but not exactly) sqrt(2)/2 in
// one case.
#![allow(clippy::many_single_char_names, clippy::approx_constant)]

use dora_openarm_webxr_rust::quaternion::Quaternion;

const EPS: f64 = 1e-12;

fn assert_close(a: [f64; 4], b: [f64; 4]) {
    for i in 0..4 {
        assert!(
            (a[i] - b[i]).abs() < EPS,
            "component {i}: {} vs {} (a={a:?} b={b:?})",
            a[i],
            b[i]
        );
    }
}

#[test]
fn identity_hamilton_product_is_identity() {
    let q = Quaternion::from_xyzw([0.3, -0.1, 0.2, 0.9]).normalize();
    let identity = Quaternion::from_xyzw([0.0, 0.0, 0.0, 1.0]);
    assert_close(q.hamilton_product(identity).to_xyzw(), q.to_xyzw());
    assert_close(identity.hamilton_product(q).to_xyzw(), q.to_xyzw());
}

/// scipy: `Rotation.from_matrix([[0,0,-1],[-1,0,0],[0,1,0]]).as_quat()` is
/// exactly `[0.5, -0.5, -0.5, 0.5]` -- verified with scipy 1.18.0 (see
/// `_ROBOT_ROTATION_QUAT` in `src/transform.rs`).
#[test]
fn robot_rotation_quat_composition_matches_scipy_identity_case() {
    let robot = Quaternion::from_xyzw([0.5, -0.5, -0.5, 0.5]);
    let identity_input = Quaternion::from_xyzw([0.0, 0.0, 0.0, 1.0]);
    // 90 degree rotation about Z, scalar-last.
    let fix = Quaternion::from_z_rotation_degrees(90.0);

    let combined = robot.hamilton_product(identity_input).hamilton_product(fix);

    // Captured from `scipy` directly (see oracle in the port's development
    // notes): xyzw = [5.55111512e-17, -7.07106781e-01, -5.55111512e-17, 7.07106781e-01]
    let expected = [
        5.551_115_123_125_783e-17,
        -0.707_106_781_186_547_6,
        -5.551_115_123_125_783e-17,
        0.707_106_781_186_547_6,
    ];
    assert_close(combined.to_xyzw(), expected);
}

#[test]
fn rotate_vector_matches_scipy_apply_for_robot_rotation() {
    let robot = Quaternion::from_xyzw([0.5, -0.5, -0.5, 0.5]);
    // R @ (x, y, z) = (-z, -x, y) for the upstream `_ROBOT_ROTATION_MATRIX`.
    let cases: [([f64; 3], [f64; 3]); 4] = [
        ([0.1, 0.2, 0.3], [-0.3, -0.1, 0.2]),
        ([-0.05, 0.15, -0.2], [0.2, 0.05, 0.15]),
        ([0.037, -0.421, 0.113], [-0.113, -0.037, -0.421]),
        ([-0.3, 0.05, 0.02], [-0.02, 0.3, 0.05]),
    ];
    for (input, expected) in cases {
        let rotated = robot.rotate_vector(input);
        for i in 0..3 {
            assert!(
                (rotated[i] - expected[i]).abs() < EPS,
                "input={input:?} rotated={rotated:?} expected={expected:?}"
            );
        }
    }
}

#[test]
fn normalize_produces_unit_quaternion() {
    let q = Quaternion::from_xyzw([1.0, 2.0, 3.0, 4.0]).normalize();
    let [x, y, z, w] = q.to_xyzw();
    let norm_sq = x * x + y * y + z * z + w * w;
    assert!((norm_sq - 1.0).abs() < EPS);
}

#[test]
fn z_rotation_90_degrees_matches_expected_quaternion() {
    let fix = Quaternion::from_z_rotation_degrees(90.0);
    let expected_half = std::f64::consts::FRAC_1_SQRT_2;
    assert_close(fix.to_xyzw(), [0.0, 0.0, expected_half, expected_half]);
}

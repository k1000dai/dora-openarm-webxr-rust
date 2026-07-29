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

//! Golden tests for the One Euro pose filter, matching upstream
//! `smoothing.py`.
//!
//! Numerical note: upstream initializes `self.dp_prev = np.zeros(3)`, which
//! numpy gives `float64` dtype by default. Under numpy 2.x's NEP 50 type
//! promotion, mixing that `float64` array into `dp_filtered`'s computation
//! upgrades the whole recurrence (position, quaternion, velocity state) to
//! `float64` from the second `smooth()` call onward, even though the input
//! pose and each call's *returned* pose are `float32` (`smoothing.py` casts
//! down explicitly at the end of `smooth()`, but stores the pre-cast
//! `float64` values back into `self.p_prev`/`self.q_prev`/`self.dp_prev` for
//! the next call). Verified directly against numpy 2.5.1 (dtype of
//! `p_prev`/`q_prev`/`dp_prev` becomes `float64` starting with the second
//! `smooth()` call).
//!
//! This Rust port therefore keeps all filter state and arithmetic in `f64`,
//! widening the `f32` input once per call and narrowing only the returned
//! pose, which reproduces the real recurrence to within 1.2e-10 of the
//! upstream trace (cross-checked against a pure-numpy float64
//! reimplementation) -- far tighter than the 1e-6 tolerance used below.

// Expected values are transcribed verbatim from the golden-vector generator
// for traceability; some are close to (but not exactly) sqrt(2)/2.
#![allow(clippy::excessive_precision, clippy::approx_constant)]

use dora_openarm_webxr_rust::smoothing::{OneEuroPoseSmoother, slerp_quat};

const TOL: f32 = 1e-6;

fn assert_close4(actual: [f64; 4], expected: [f64; 4]) {
    for i in 0..4 {
        assert!(
            (actual[i] - expected[i]).abs() < 1e-9,
            "component {i}: actual={actual:?} expected={expected:?}"
        );
    }
}

fn assert_close7(actual: [f32; 7], expected: [f32; 7]) {
    for i in 0..7 {
        assert!(
            (actual[i] - expected[i]).abs() < TOL,
            "component {i}: actual={actual:?} expected={expected:?}"
        );
    }
}

#[test]
fn slerp_alpha_zero_returns_first_quat() {
    let q1 = [0.0, 0.0, 0.0, 1.0];
    let q2 = [0.0, 0.707_106_781_186_547_6, 0.0, 0.707_106_781_186_547_6];
    assert_close4(slerp_quat(q1, q2, 0.0), [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn slerp_alpha_one_returns_second_quat() {
    let q1 = [0.0, 0.0, 0.0, 1.0];
    let q2 = [0.0, 0.707_106_781_186_547_6, 0.0, 0.707_106_781_186_547_6];
    assert_close4(
        slerp_quat(q1, q2, 1.0),
        [0.0, 0.707_106_781_186_547_6, 0.0, 0.707_106_781_186_547_6],
    );
}

#[test]
fn slerp_alpha_half_matches_golden_value() {
    let q1 = [0.0, 0.0, 0.0, 1.0];
    let q2 = [0.0, 0.707_106_781_186_547_6, 0.0, 0.707_106_781_186_547_6];
    assert_close4(
        slerp_quat(q1, q2, 0.5),
        [0.0, 0.382_683_432_365_089_84, 0.0, 0.923_879_532_511_286_7],
    );
}

#[test]
fn slerp_alpha_quarter_matches_golden_value() {
    let q1 = [0.0, 0.0, 0.0, 1.0];
    let q2 = [0.0, 0.707_106_781_186_547_6, 0.0, 0.707_106_781_186_547_6];
    assert_close4(
        slerp_quat(q1, q2, 0.25),
        [0.0, 0.195_090_322_016_128_28, 0.0, 0.980_785_280_403_230_4],
    );
}

#[test]
fn slerp_near_identical_quats_uses_linear_branch() {
    // dot > 0.9995 triggers the linear-interpolate-then-normalize branch.
    let q1 = [0.0, 0.0, 0.0, 1.0];
    let q2 = [0.000_999_999_999_999_875, 0.0, 0.0, 0.999_999_499_999_875];
    assert_close4(
        slerp_quat(q1, q2, 0.5),
        [
            0.000_500_000_062_499_964_7,
            0.0,
            0.0,
            0.999_999_874_999_960_9,
        ],
    );
}

#[test]
fn slerp_opposite_hemisphere_flips_sign() {
    // dot < 0 triggers the hemisphere flip before interpolating.
    let q1 = [0.0, 0.0, 0.0, 1.0];
    let q2 = [0.010_009_508_544_689_92, 0.0, 0.0, -0.999_949_903_614_523];
    assert_close4(
        slerp_quat(q1, q2, 0.3),
        [
            -0.003_002_884_154_638_002_4,
            0.0,
            0.0,
            0.999_995_491_333_212_9,
        ],
    );
}

#[test]
fn first_sample_passes_through_unchanged() {
    let mut smoother = OneEuroPoseSmoother::new(2.0, 0.04, 1.5);
    let input = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let out = smoother.smooth(0.0, input);
    assert_close7(out, input);
}

#[test]
fn none_input_returns_none() {
    let mut smoother = OneEuroPoseSmoother::new(2.0, 0.04, 1.5);
    assert!(smoother.smooth_option(0.0, None).is_none());
}

#[test]
fn non_positive_dt_returns_input_unchanged() {
    let mut smoother = OneEuroPoseSmoother::new(2.0, 0.04, 1.5);
    let _ = smoother.smooth(0.048, [0.05, 0.02, -0.01, 0.0, 0.0, 0.1, 0.994_987_43]);
    let same_t_input = [0.06, 0.03, -0.02, 0.0, 0.0, 0.15, 0.988_771_08];
    let out = smoother.smooth(0.048, same_t_input);
    assert_close7(out, same_t_input);
}

/// Reproduces the exact `smooth_sequence` golden trace captured from the
/// upstream `OneEuroPoseSmoother(min_cutoff=2.0, beta=0.04, d_cutoff=1.5)`
/// (the construction used in `main.py`'s websocket handler).
#[test]
fn webxr_smoother_sequence_matches_golden_trace() {
    let mut smoother = OneEuroPoseSmoother::new(2.0, 0.04, 1.5);
    let steps: [(f64, [f32; 7], [f32; 7]); 6] = [
        (
            0.0,
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
        ),
        (
            0.016,
            [0.01, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            [0.001_676_316_955_126_822, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
        ),
        (
            0.032,
            [0.02, 0.01, 0.0, 0.0, 0.0, 0.03, 0.999_5],
            [
                0.004_755_805_246_531_963,
                0.001_680_605_462_752_282_6,
                0.0,
                0.0,
                0.0,
                0.005_042_633_507_400_751,
                0.999_985_873_699_188_2,
            ],
        ),
        (
            0.048,
            [0.05, 0.02, -0.01, 0.0, 0.0, 0.1, 0.994_987_43],
            [
                0.012_406_645_342_707_634,
                0.004_778_434_056_788_683,
                -0.001_691_010_198_555_886_7,
                0.0,
                0.0,
                0.021_126_786_246_895_79,
                0.999_775_826_930_999_8,
            ],
        ),
        (
            // Same timestamp as the previous sample -> dt <= 0 -> passthrough.
            0.048,
            [0.06, 0.03, -0.02, 0.0, 0.0, 0.15, 0.988_771_08],
            [0.06, 0.03, -0.02, 0.0, 0.0, 0.15, 0.988_771_08],
        ),
        (
            0.064,
            [0.06, 0.03, -0.02, 0.0, 0.0, 0.15, 0.988_771_08],
            [
                0.020_504_856,
                0.009_069_99,
                -0.004_806_362_6,
                0.0,
                0.0,
                0.043_138_865,
                0.999_070_9,
            ],
        ),
    ];

    for (t, input, expected) in steps {
        let out = smoother.smooth(t, input);
        assert_close7(out, expected);
    }
}

#[test]
fn reset_clears_state_so_next_sample_passes_through() {
    let mut smoother = OneEuroPoseSmoother::new(2.0, 0.04, 1.5);
    let _ = smoother.smooth(0.0, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    let _ = smoother.smooth(0.016, [0.01, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    smoother.reset();
    let input = [5.0, 5.0, 5.0, 0.0, 0.0, 0.0, 1.0];
    let out = smoother.smooth(1.0, input);
    assert_close7(out, input);
}

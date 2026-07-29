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

//! Tests for [`FrameProcessor`], the pure port of `main.py`'s
//! `_websocket_endpoint` message-handling body (everything except the
//! socket I/O itself). These pin down output id **order** and the
//! conditional logic upstream expresses as nested `if`s -- the numeric
//! transform/smoothing math itself is covered by `tests/transform.rs` and
//! `tests/smoothing.rs`.

use dora_openarm_webxr_rust::protocol::{ClientMessage, FrameMessage, PoseJson, parse_message};
use dora_openarm_webxr_rust::state::{EmissionValue, FrameProcessor, output_id};

fn ids(emissions: &[dora_openarm_webxr_rust::state::Emission]) -> Vec<&'static str> {
    emissions.iter().map(|e| e.output_id).collect()
}

#[test]
fn session_start_emits_only_status_ready() {
    let mut processor = FrameProcessor::new();
    let emissions = processor.process(&ClientMessage::SessionStart, 1, 0.0);
    assert_eq!(emissions.len(), 1);
    assert_eq!(emissions[0].output_id, output_id::STATUS);
    assert_eq!(emissions[0].value, EmissionValue::Status("ready"));
}

#[test]
fn other_message_types_emit_nothing() {
    let mut processor = FrameProcessor::new();
    assert!(processor.process(&ClientMessage::Other, 1, 0.0).is_empty());
}

#[test]
fn empty_frame_emits_only_vr_receive_times() {
    let mut processor = FrameProcessor::new();
    let emissions = processor.process(&ClientMessage::Frame(FrameMessage::default()), 42, 0.0);
    assert_eq!(emissions.len(), 1);
    assert_eq!(emissions[0].output_id, output_id::VR_RECEIVE_TIMES);
    assert_eq!(emissions[0].value, EmissionValue::Int64(42));
}

#[test]
fn button_order_is_a_b_x_y_and_only_present_buttons_emit() {
    let mut processor = FrameProcessor::new();
    let frame = FrameMessage {
        button_b: Some(true),
        button_y: Some(false),
        ..Default::default()
    };
    let emissions = processor.process(&ClientMessage::Frame(frame), 1, 0.0);
    assert_eq!(
        ids(&emissions),
        vec![
            output_id::VR_RECEIVE_TIMES,
            output_id::BUTTON_B,
            output_id::BUTTON_Y
        ]
    );
}

#[test]
fn full_frame_emission_order_is_vr_times_buttons_right_then_left() {
    let mut processor = FrameProcessor::new();
    let pose = PoseJson {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
    };
    let frame = FrameMessage {
        pose_right: Some(pose),
        trigger_right: Some(0.5),
        joystick_right: Some([0.0, 0.1, 0.2, 0.3]),
        pose_left: Some(pose),
        trigger_left: Some(0.5),
        joystick_left: Some([0.0, 0.1, 0.2, 0.3]),
        button_a: Some(true),
        button_b: Some(true),
        button_x: Some(true),
        button_y: Some(true),
    };
    let emissions = processor.process(&ClientMessage::Frame(frame), 1, 0.0);
    assert_eq!(
        ids(&emissions),
        vec![
            output_id::VR_RECEIVE_TIMES,
            output_id::BUTTON_A,
            output_id::BUTTON_B,
            output_id::BUTTON_X,
            output_id::BUTTON_Y,
            output_id::POSE_RIGHT,
            output_id::TRIGGER_RIGHT,
            output_id::JOYSTICK_X_RIGHT,
            output_id::JOYSTICK_Y_RIGHT,
            output_id::POSE_LEFT,
            output_id::TRIGGER_LEFT,
            output_id::JOYSTICK_X_LEFT,
            output_id::JOYSTICK_Y_LEFT,
        ]
    );
}

#[test]
fn trigger_without_pose_still_emits_trigger_but_not_pose() {
    let mut processor = FrameProcessor::new();
    let frame = FrameMessage {
        trigger_right: Some(0.3),
        ..Default::default()
    };
    let emissions = processor.process(&ClientMessage::Frame(frame), 1, 0.0);
    assert_eq!(
        ids(&emissions),
        vec![output_id::VR_RECEIVE_TIMES, output_id::TRIGGER_RIGHT]
    );
}

#[test]
fn pose_without_trigger_emits_nothing_for_that_side() {
    let mut processor = FrameProcessor::new();
    let pose = PoseJson {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
    };
    let frame = FrameMessage {
        pose_right: Some(pose),
        ..Default::default()
    };
    let emissions = processor.process(&ClientMessage::Frame(frame), 1, 0.0);
    assert_eq!(ids(&emissions), vec![output_id::VR_RECEIVE_TIMES]);
}

#[test]
fn joystick_axis_mapping_matches_upstream_x_y_formula() {
    // Upstream: x = axes[1] - axes[3]; y = axes[2] - axes[0].
    let mut processor = FrameProcessor::new();
    let frame = FrameMessage {
        joystick_right: Some([1.0, 2.0, 3.0, 4.0]),
        ..Default::default()
    };
    let emissions = processor.process(&ClientMessage::Frame(frame), 1, 0.0);
    let x = emissions
        .iter()
        .find(|e| e.output_id == output_id::JOYSTICK_X_RIGHT)
        .unwrap();
    let y = emissions
        .iter()
        .find(|e| e.output_id == output_id::JOYSTICK_Y_RIGHT)
        .unwrap();
    assert_eq!(x.value, EmissionValue::Float32(2.0 - 4.0));
    assert_eq!(y.value, EmissionValue::Float32(3.0 - 1.0));
}

#[test]
fn pose_with_gripper_is_eight_floats_with_gripper_last() {
    let mut processor = FrameProcessor::new();
    let pose = PoseJson {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
    };
    let frame = FrameMessage {
        pose_right: Some(pose),
        trigger_right: Some(1.0),
        ..Default::default()
    };
    let emissions = processor.process(&ClientMessage::Frame(frame), 1, 0.0);
    let pose_emission = emissions
        .iter()
        .find(|e| e.output_id == output_id::POSE_RIGHT)
        .unwrap();
    let EmissionValue::PoseWithGripper(values) = pose_emission.value else {
        panic!("expected PoseWithGripper")
    };
    // trigger=1.0 -> gripper angle 0.0 for both sides.
    assert!((values[7] - 0.0).abs() < 1e-6);
}

#[test]
fn right_and_left_gripper_mapping_diverge() {
    let mut processor = FrameProcessor::new();
    let pose = PoseJson {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
    };
    let frame = FrameMessage {
        pose_right: Some(pose),
        trigger_right: Some(0.0),
        pose_left: Some(pose),
        trigger_left: Some(0.0),
        ..Default::default()
    };
    let emissions = processor.process(&ClientMessage::Frame(frame), 1, 0.0);
    let right = emissions
        .iter()
        .find(|e| e.output_id == output_id::POSE_RIGHT)
        .unwrap();
    let left = emissions
        .iter()
        .find(|e| e.output_id == output_id::POSE_LEFT)
        .unwrap();
    let EmissionValue::PoseWithGripper(right_values) = right.value else {
        panic!()
    };
    let EmissionValue::PoseWithGripper(left_values) = left.value else {
        panic!()
    };
    assert!((right_values[7] - (-0.785)).abs() < 1e-6);
    assert!((left_values[7] - 0.785).abs() < 1e-6);
}

#[test]
fn smoother_state_is_retained_across_calls_per_side() {
    // Two identical poses one step apart should smooth toward, not jump to,
    // the target -- proving the per-connection smoother persists in
    // `FrameProcessor` rather than being reconstructed every call.
    let mut processor = FrameProcessor::new();
    let start = PoseJson {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
    };
    let moved = PoseJson {
        x: 1.0,
        y: 0.0,
        z: 0.0,
        qx: 0.0,
        qy: 0.0,
        qz: 0.0,
        qw: 1.0,
    };

    let frame1 = FrameMessage {
        pose_right: Some(start),
        trigger_right: Some(0.0),
        ..Default::default()
    };
    let _ = processor.process(&ClientMessage::Frame(frame1), 1, 0.0);

    let frame2 = FrameMessage {
        pose_right: Some(moved),
        trigger_right: Some(0.0),
        ..Default::default()
    };
    let emissions = processor.process(&ClientMessage::Frame(frame2), 2, 0.016);
    let pose_emission = emissions
        .iter()
        .find(|e| e.output_id == output_id::POSE_RIGHT)
        .unwrap();
    let EmissionValue::PoseWithGripper(values) = pose_emission.value else {
        panic!()
    };

    // `_ROBOT_ROTATION_MATRIX` maps output index 1 to `-x_input`, so moving
    // the raw pose along X shows up there. A fresh (unsmoothed) smoother's
    // first sample is a passthrough and would jump straight to the target;
    // the continued processor's smoothed output must lag behind it.
    let unsmoothed_frame = FrameMessage {
        pose_right: Some(moved),
        trigger_right: Some(0.0),
        ..Default::default()
    };
    let mut fresh_processor = FrameProcessor::new();
    let fresh_emissions = fresh_processor.process(&ClientMessage::Frame(unsmoothed_frame), 1, 0.0);
    let fresh_pose = fresh_emissions
        .iter()
        .find(|e| e.output_id == output_id::POSE_RIGHT)
        .unwrap();
    let EmissionValue::PoseWithGripper(fresh_values) = fresh_pose.value else {
        panic!()
    };

    assert!((values[1] - fresh_values[1]).abs() > 1e-3);
}

#[test]
fn parsed_websocket_frame_message_round_trips_through_processor() {
    let raw = r#"{"type": "frame", "trigger_right": 0.2}"#;
    let message = parse_message(raw).unwrap();
    let mut processor = FrameProcessor::new();
    let emissions = processor.process(&message, 1, 0.0);
    assert_eq!(
        ids(&emissions),
        vec![output_id::VR_RECEIVE_TIMES, output_id::TRIGGER_RIGHT]
    );
}

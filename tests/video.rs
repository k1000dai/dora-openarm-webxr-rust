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

#![allow(missing_docs)]

use std::sync::Arc;

use dora_openarm_webxr_rust::video::{Eye, FrameSet, VideoStream, ViewConfiguration};

#[test]
fn fixed_view_sends_only_the_newest_right_frame_with_prefix() {
    let config = ViewConfiguration::from_yaml_str("view: fixed\n").unwrap();
    let mut frames = FrameSet::new();
    let mut stream = VideoStream::new(&config);
    assert_eq!(stream.eyes(), &[Eye::Right]);
    assert!(stream.next_messages(&frames).is_empty());

    frames.store(Eye::Right, Arc::from(&b"old"[..]));
    assert_eq!(
        stream.next_messages(&frames),
        vec![vec![1, b'o', b'l', b'd']]
    );
    assert!(stream.next_messages(&frames).is_empty());

    frames.store(Eye::Right, Arc::from(&b"new"[..]));
    assert_eq!(
        stream.next_messages(&frames),
        vec![vec![1, b'n', b'e', b'w']]
    );
}

#[test]
fn stereo_view_waits_for_both_eyes_and_emits_both_together() {
    let config = ViewConfiguration::from_yaml_str("view: stereo\n").unwrap();
    let mut frames = FrameSet::new();
    let mut stream = VideoStream::new(&config);
    assert_eq!(stream.eyes(), &[Eye::Left, Eye::Right]);

    frames.store(Eye::Right, Arc::from(&b"r"[..]));
    assert!(stream.next_messages(&frames).is_empty());
    frames.store(Eye::Left, Arc::from(&b"l"[..]));
    assert_eq!(
        stream.next_messages(&frames),
        vec![vec![0, b'l'], vec![1, b'r']]
    );

    frames.store(Eye::Left, Arc::from(&b"l2"[..]));
    assert_eq!(
        stream.next_messages(&frames),
        vec![vec![0, b'l', b'2'], vec![1, b'r']]
    );
}

#[test]
fn view_configuration_preserves_yaml_and_reads_frame_offset() {
    let config = ViewConfiguration::from_yaml_str(
        "view: none\nsession:\n  mode: immersive-vr\npose:\n  frame_offset: [1, 2.5, -3]\n",
    )
    .unwrap();
    assert_eq!(config.view(), Some("none"));
    assert_eq!(config.frame_offset().unwrap(), Some([1.0, 2.5, -3.0]));
    assert_eq!(config.value()["session"]["mode"], "immersive-vr");
}

#[test]
fn unknown_camera_input_is_not_mapped() {
    assert_eq!(Eye::from_input_id("camera_head_middle"), None);
}

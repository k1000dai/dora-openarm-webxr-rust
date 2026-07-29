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

//! Arrow payload shapes, matching upstream `main.py`'s `pa.array(...)`
//! calls exactly: type, nesting and element count.

use arrow::array::{
    Array, BooleanArray, Float32Array, Int64Array, ListArray, StringArray, StructArray,
};
use arrow::datatypes::DataType;
use dora_openarm_webxr_rust::output::{build_array, status_ready};
use dora_openarm_webxr_rust::state::EmissionValue;

#[test]
fn status_is_a_length_one_utf8_array_containing_ready() {
    let array = status_ready();
    let strings = array.as_any().downcast_ref::<StringArray>().unwrap();
    assert_eq!(strings.len(), 1);
    assert_eq!(strings.value(0), "ready");
    assert_eq!(array.data_type(), &DataType::Utf8);
}

#[test]
fn vr_receive_times_is_int64() {
    let array = build_array(EmissionValue::Int64(1_700_000_000_000_000_000));
    let ints = array.as_any().downcast_ref::<Int64Array>().unwrap();
    assert_eq!(ints.len(), 1);
    assert_eq!(ints.value(0), 1_700_000_000_000_000_000);
}

#[test]
fn button_is_boolean() {
    let array = build_array(EmissionValue::Bool(true));
    let bools = array.as_any().downcast_ref::<BooleanArray>().unwrap();
    assert_eq!(bools.len(), 1);
    assert!(bools.value(0));
}

#[test]
fn trigger_and_joystick_are_float32() {
    let array = build_array(EmissionValue::Float32(0.42));
    let floats = array.as_any().downcast_ref::<Float32Array>().unwrap();
    assert_eq!(floats.len(), 1);
    assert!((floats.value(0) - 0.42).abs() < 1e-6);
}

#[test]
fn pose_is_a_length_one_struct_with_a_pose_list_field_of_8_float32() {
    let pose = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let array = build_array(EmissionValue::PoseWithGripper(pose));
    let structs = array.as_any().downcast_ref::<StructArray>().unwrap();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs.num_columns(), 1);
    assert_eq!(structs.column_names(), vec!["pose"]);

    let pose_list = structs
        .column(0)
        .as_any()
        .downcast_ref::<ListArray>()
        .unwrap();
    assert_eq!(pose_list.len(), 1);
    assert_eq!(pose_list.value_type(), DataType::Float32);

    let values = pose_list.value(0);
    let values = values.as_any().downcast_ref::<Float32Array>().unwrap();
    assert_eq!(values.len(), 8);
    for (i, v) in pose.iter().enumerate() {
        assert!((values.value(i) - v).abs() < 1e-6);
    }
}

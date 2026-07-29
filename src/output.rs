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

//! Arrow array builders for dora outputs, matching upstream `main.py`'s
//! `pa.array(...)` payloads exactly:
//!
//! - `status`: `Utf8Array` len 1 (`"ready"`).
//! - `vr_receive_times`: `Int64Array` len 1.
//! - `button_{a,b,x,y}`: `BooleanArray` len 1.
//! - `trigger_{side}`, `joystick_{x,y}_{side}`: `Float32Array` len 1.
//! - `pose_{side}`: `StructArray` len 1 with one field, `pose: List<Float32>`
//!   of length 8 (`[x, y, z, qw, qx, qy, qz, gripper]`), matching upstream's
//!   `pa.struct({"pose": pa.list_(pa.float32())})`.

use crate::state::EmissionValue;
use arrow::array::{
    ArrayRef, BooleanArray, Float32Array, Int64Array, ListArray, StringArray, StructArray,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Fields};
use std::sync::Arc;

/// Builds the `status` output value, `"ready"`.
#[must_use]
pub fn status_ready() -> ArrayRef {
    Arc::new(StringArray::from(vec!["ready"]))
}

/// Builds the Arrow array for one [`EmissionValue`].
#[must_use]
pub fn build_array(value: EmissionValue) -> ArrayRef {
    match value {
        EmissionValue::Status(text) => Arc::new(StringArray::from(vec![text])),
        EmissionValue::Int64(v) => Arc::new(Int64Array::from(vec![v])),
        EmissionValue::Bool(v) => Arc::new(BooleanArray::from(vec![v])),
        EmissionValue::Float32(v) => Arc::new(Float32Array::from(vec![v])),
        EmissionValue::PoseWithGripper(pose) => pose_with_gripper(pose),
    }
}

fn pose_with_gripper(pose: [f32; 8]) -> ArrayRef {
    let item_field = Arc::new(Field::new("item", DataType::Float32, true));
    let values: ArrayRef = Arc::new(Float32Array::from(pose.to_vec()));
    let offsets = OffsetBuffer::from_lengths([8]);
    let pose_list = ListArray::new(item_field.clone(), offsets, values, None);

    let pose_field = Arc::new(Field::new("pose", DataType::List(item_field), true));
    Arc::new(StructArray::new(
        Fields::from(vec![pose_field]),
        vec![Arc::new(pose_list)],
        None,
    ))
}

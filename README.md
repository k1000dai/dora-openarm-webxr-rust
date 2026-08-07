# dora-openarm-webxr-rust

Behavior-compatible Rust port of [`enactic/dora-openarm-webxr`](https://github.com/enactic/dora-openarm-webxr).

The installed `dora-openarm-webxr` binary serves the upstream WebXR front end over HTTPS, accepts controller frames over WebSocket, transforms and smooths controller poses, and publishes them to a Dora dataflow.

## Build

```sh
cargo build --release
```

Rust 1.85 or later is required.

## Run

A TLS certificate and matching private key are required, as in the upstream node:

```sh
dora-openarm-webxr \
  --host 0.0.0.0 \
  --port 8443 \
  --tls-certificate-file cert.pem \
  --tls-key-file key.pem
```

The equivalent environment variables are `HOST`, `PORT`, `TLS_CERTIFICATE_FILE`, and `TLS_KEY_FILE`. The optional `--view-configuration-file` also accepts `VIEW_CONFIGURATION_FILE`; the defaults are `0.0.0.0:8443` and the built-in fixed camera view.

HTTP routes:

- `/` and `/index.html`: embedded upstream WebXR page
- `/ar.js`, `/panel.js`, `/stereo.js`: embedded WebXR JavaScript assets
- `/websocket`: WebXR JSON WebSocket protocol
- `/view_configuration`: startup YAML configuration served as JSON
- `/video`: binary JPEG WebSocket. Each message is one eye-prefix byte (`0x00` left, `0x01` right) followed by the original `UInt8` JPEG payload.

When the view configuration selects `fixed` (the default), `/video` sends the newest right-head-camera frame. `stereo` waits for both `camera_head_left` and `camera_head_right` and sends both together; `none` does not open the browser video panel. Camera inputs are stored independently from the pose WebSocket and stale frames are skipped.

Example configuration files are included in [`example/view_camera.yaml`](example/view_camera.yaml) and [`example/view_camera_stereo.yaml`](example/view_camera_stereo.yaml). The optional `pose.frame_offset: [x, y, z]` entry overrides the default neutral hand offset.

## Dora outputs

The node preserves the upstream output identifiers, Arrow types, order, and `timestamp` metadata:

- `status`
- `vr_receive_times`
- `button_a`, `button_b`, `button_x`, `button_y`
- `pose_right`, `pose_left`
- `trigger_right`, `trigger_left`
- `joystick_x_right`, `joystick_y_right`
- `joystick_x_left`, `joystick_y_left`

Pose outputs are length-one struct arrays containing an eight-element `Float32` pose/gripper list. Coordinate conversion, quaternion composition, One Euro smoothing, gripper mapping, and frame emission order are covered by retained golden and edge-case tests.

## Validation scope

The HTTP assets, WebSocket protocol parser, Arrow output shapes, transforms, smoothing, state transitions, and release binary are tested locally and in CI. No Meta Quest, PICO, OpenArm hardware, or browser WebXR runtime was available during validation, so headset pairing and physical robot behavior are not claimed as tested.

## License

Apache License 2.0. See [`LICENSE`](LICENSE) and [`NOTICE`](NOTICE). The embedded browser assets and behavior are derived from the upstream Enactic project with attribution retained.

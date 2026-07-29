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

//! `WebXR` server node for `OpenArm` teleoperation.
//!
//! This dora-rs node serves the `WebXR` front-end over HTTPS and accepts a
//! `WebSocket` connection from a VR device such as Meta Quest 3 or PICO 4.
//! For each frame received from the device, it converts the controller
//! pose from `WebXR` coordinates into the `OpenArm` workspace, smooths it with
//! a One Euro filter, and publishes the pose, trigger, joystick and button
//! state as dora-rs outputs.
//!
//! The published poses are expressed in the scene's `arm_origin` site frame
//! (chest-level origin between the arms), not in world coordinates.
//! Downstream IK interprets targets in the same frame.
//!
//! The Web server and the dora-rs event loop run concurrently on the same
//! tokio runtime; the server shuts down when the dora-rs node receives a
//! `STOP` event. This adapter is intentionally thin: all pose/filter/wire
//! logic lives in the library (`src/lib.rs` and its modules) and is tested
//! there without a running dataflow or a browser.

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum_server::tls_rustls::RustlsConfig;
use clap::error::ErrorKind;
use dora_node_api::dora_core::config::DataId;
use dora_node_api::futures::SinkExt;
use dora_node_api::{DoraNode, Event, EventStream, MetadataParameters, Parameter};
use dora_openarm_webxr_rust::assets::{
    AR_JS, AR_JS_CONTENT_TYPE, INDEX_HTML, INDEX_HTML_CONTENT_TYPE,
};
use dora_openarm_webxr_rust::cli::{Args, ArgsError};
use dora_openarm_webxr_rust::output::build_array;
use dora_openarm_webxr_rust::protocol::parse_message;
use dora_openarm_webxr_rust::state::FrameProcessor;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct AppState {
    node: Arc<Mutex<DoraNode>>,
    should_exit: Arc<AtomicBool>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = match Args::parse() {
        Ok(args) => args,
        // `--help`/`--version` aren't failures: print them and exit
        // cleanly, matching `argparse`'s behavior for the same flags.
        Err(ArgsError::InvalidArguments(err))
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            err.exit();
        }
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    };

    let (node, events) = DoraNode::init_from_env()?;
    let should_exit = Arc::new(AtomicBool::new(false));
    let handle = axum_server::Handle::new();

    let dora_task = tokio::spawn(run_dora_event_loop(
        events,
        should_exit.clone(),
        handle.clone(),
    ));

    let state = AppState {
        node: Arc::new(Mutex::new(node)),
        should_exit,
    };
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/ar.js", get(serve_ar_js))
        .route("/websocket", get(websocket_upgrade))
        .with_state(state);

    let tls_config =
        RustlsConfig::from_pem_file(&args.tls_certificate_file, &args.tls_key_file).await?;
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    axum_server::bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service())
        .await?;

    let _ = dora_task.await;
    Ok(())
}

/// Watches the dora event stream and triggers HTTP server shutdown once a
/// `STOP` event (or stream closure) arrives, matching upstream's
/// `_main_dora` loop (`if event["type"] == "STOP": break`) followed by
/// `server.should_exit = True`.
async fn run_dora_event_loop(
    mut events: EventStream,
    should_exit: Arc<AtomicBool>,
    handle: axum_server::Handle,
) {
    loop {
        match events.recv_async().await {
            Some(Event::Stop(_)) | None => break,
            Some(_) => {}
        }
    }
    should_exit.store(true, Ordering::SeqCst);
    handle.shutdown();
}

async fn serve_index() -> impl IntoResponse {
    ([(CONTENT_TYPE, INDEX_HTML_CONTENT_TYPE)], INDEX_HTML)
}

async fn serve_ar_js() -> impl IntoResponse {
    ([(CONTENT_TYPE, AR_JS_CONTENT_TYPE)], AR_JS)
}

async fn websocket_upgrade(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Ports `_websocket_endpoint`'s message loop: `while not server.should_exit:
/// data = await websocket.receive_text()`. The `should_exit` check happens
/// only before each receive, so (matching upstream) a connection with no
/// pending message keeps blocking in `recv()` until the next message
/// arrives or the socket closes, even after shutdown has been requested.
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut processor = FrameProcessor::new();
    let start = Instant::now();

    'messages: while !state.should_exit.load(Ordering::SeqCst) {
        let Some(received) = socket.recv().await else {
            break;
        };
        let Ok(message) = received else { break };
        let text = match message {
            Message::Text(text) => text,
            // `Close` is a normal disconnect. `ar.js` only ever sends JSON
            // text frames, so a binary frame has no upstream equivalent to
            // preserve (`receive_text()` raises) -- end the connection the
            // same way upstream's unhandled exception would.
            Message::Close(_) | Message::Binary(_) => break,
            Message::Ping(_) | Message::Pong(_) => continue,
        };

        let Ok(parsed) = parse_message(&text) else {
            break;
        };

        let timestamp_ns = now_ns();
        let smoother_time = start.elapsed().as_secs_f64();
        let emissions = processor.process(&parsed, timestamp_ns, smoother_time);

        let mut parameters = MetadataParameters::new();
        parameters.insert("timestamp".to_string(), Parameter::Integer(timestamp_ns));

        let mut node = state
            .node
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for emission in emissions {
            let array = build_array(emission.value);
            if node
                .send_output(
                    DataId::from(emission.output_id.to_owned()),
                    parameters.clone(),
                    array,
                )
                .is_err()
            {
                break 'messages;
            }
        }
    }

    let _ = socket.close().await;
}

fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
}

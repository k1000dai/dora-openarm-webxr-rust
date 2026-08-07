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
//! The adapter owns the HTTPS/WebSocket server and the dora event loop. Pure
//! pose processing lives in the library; head-camera frames are retained in a
//! small latest-frame store and sent over an independent `/video` WebSocket so
//! they never block pose messages.

use arrow::array::{Array, UInt8Array};
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
    AR_JS, INDEX_HTML, INDEX_HTML_CONTENT_TYPE, JAVASCRIPT_CONTENT_TYPE, PANEL_JS, STEREO_JS,
};
use dora_openarm_webxr_rust::cli::{Args, ArgsError};
use dora_openarm_webxr_rust::output::build_array;
use dora_openarm_webxr_rust::protocol::parse_message;
use dora_openarm_webxr_rust::state::FrameProcessor;
use dora_openarm_webxr_rust::video::{Eye, FrameSet, VideoStream, ViewConfiguration};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;

#[derive(Clone)]
struct AppState {
    node: Arc<Mutex<DoraNode>>,
    should_exit: Arc<AtomicBool>,
    view_configuration: ViewConfiguration,
    frames: Arc<Mutex<FrameSet>>,
    frame_notify: Arc<Notify>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = match Args::parse() {
        Ok(args) => args,
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

    let view_configuration = ViewConfiguration::load(args.view_configuration_file.as_deref());
    let _frame_offset = view_configuration
        .frame_offset()
        .map_err(|error| eyre::eyre!("invalid view configuration: {error}"))?
        .unwrap_or(dora_openarm_webxr_rust::transform::DEFAULT_FRAME_OFFSET_CELL);

    let (node, events) = DoraNode::init_from_env()?;
    let should_exit = Arc::new(AtomicBool::new(false));
    let handle = axum_server::Handle::new();
    let frames = Arc::new(Mutex::new(FrameSet::new()));
    let frame_notify = Arc::new(Notify::new());

    let dora_task = tokio::spawn(run_dora_event_loop(
        events,
        should_exit.clone(),
        handle.clone(),
        frames.clone(),
        frame_notify.clone(),
    ));

    let state = AppState {
        node: Arc::new(Mutex::new(node)),
        should_exit,
        view_configuration,
        frames,
        frame_notify,
    };
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/ar.js", get(serve_ar_js))
        .route("/panel.js", get(serve_panel_js))
        .route("/stereo.js", get(serve_stereo_js))
        .route("/websocket", get(websocket_upgrade))
        .route("/view_configuration", get(serve_view_configuration))
        .route("/video", get(video_upgrade))
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

/// Watches dora events, stores head-camera inputs, and shuts down the server
/// on `STOP` or stream closure.
async fn run_dora_event_loop(
    mut events: EventStream,
    should_exit: Arc<AtomicBool>,
    handle: axum_server::Handle,
    frames: Arc<Mutex<FrameSet>>,
    frame_notify: Arc<Notify>,
) {
    loop {
        match events.recv_async().await {
            Some(Event::Input { id, data, .. }) => {
                if let Some(eye) = Eye::from_input_id(id.as_str())
                    && let Some(array) = data.as_any().downcast_ref::<UInt8Array>()
                {
                    let bytes: Arc<[u8]> = Arc::from(array.values().to_vec());
                    frames
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .store(eye, bytes);
                    frame_notify.notify_waiters();
                }
            }
            Some(Event::Stop(_)) | None => break,
            Some(_) => {}
        }
    }
    should_exit.store(true, Ordering::SeqCst);
    frame_notify.notify_waiters();
    handle.shutdown();
}

async fn serve_index() -> impl IntoResponse {
    ([(CONTENT_TYPE, INDEX_HTML_CONTENT_TYPE)], INDEX_HTML)
}

async fn serve_ar_js() -> impl IntoResponse {
    ([(CONTENT_TYPE, JAVASCRIPT_CONTENT_TYPE)], AR_JS)
}

async fn serve_panel_js() -> impl IntoResponse {
    ([(CONTENT_TYPE, JAVASCRIPT_CONTENT_TYPE)], PANEL_JS)
}

async fn serve_stereo_js() -> impl IntoResponse {
    ([(CONTENT_TYPE, JAVASCRIPT_CONTENT_TYPE)], STEREO_JS)
}

async fn serve_view_configuration(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.view_configuration.value().clone())
}

async fn websocket_upgrade(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn video_upgrade(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_video_socket(socket, state))
}

/// Ports `_websocket_endpoint`'s pose/control message loop.
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let frame_offset = state
        .view_configuration
        .frame_offset()
        .ok()
        .flatten()
        .unwrap_or(dora_openarm_webxr_rust::transform::DEFAULT_FRAME_OFFSET_CELL);
    let mut processor = FrameProcessor::with_frame_offset(frame_offset);
    let start = Instant::now();

    'messages: while !state.should_exit.load(Ordering::SeqCst) {
        let Some(received) = socket.recv().await else {
            break;
        };
        let Ok(message) = received else { break };
        let text = match message {
            Message::Text(text) => text,
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

async fn handle_video_socket(mut socket: WebSocket, state: AppState) {
    let mut stream = VideoStream::new(&state.view_configuration);
    while !state.should_exit.load(Ordering::SeqCst) {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            state.frame_notify.notified(),
        )
        .await;
        let messages = {
            let frames = state
                .frames
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            stream.next_messages(&frames)
        };
        for message in messages {
            if socket.send(Message::Binary(message.into())).await.is_err() {
                return;
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

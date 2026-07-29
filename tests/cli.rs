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

//! CLI/env option parsing, matching upstream `main.py`'s `argparse` setup:
//! `--host`/`HOST` (default `0.0.0.0`), `--port`/`PORT` (default `8443`),
//! and required `--tls-certificate-file`/`TLS_CERTIFICATE_FILE`,
//! `--tls-key-file`/`TLS_KEY_FILE` (required only when the matching
//! environment variable is also unset).

use dora_openarm_webxr_rust::cli::Args;
use std::path::PathBuf;

#[test]
fn defaults_with_only_required_tls_flags() {
    let args = Args::try_parse_from(
        [
            "dora-openarm-webxr",
            "--tls-certificate-file",
            "server.crt",
            "--tls-key-file",
            "server.key",
        ],
        |_| None,
    )
    .unwrap();
    assert_eq!(args.host, "0.0.0.0");
    assert_eq!(args.port, 8443);
    assert_eq!(args.tls_certificate_file, PathBuf::from("server.crt"));
    assert_eq!(args.tls_key_file, PathBuf::from("server.key"));
}

#[test]
fn cli_flags_override_env_and_defaults() {
    let args = Args::try_parse_from(
        [
            "dora-openarm-webxr",
            "--host",
            "127.0.0.1",
            "--port",
            "9000",
            "--tls-certificate-file",
            "a.crt",
            "--tls-key-file",
            "a.key",
        ],
        |key| match key {
            "HOST" => Some("10.0.0.1".to_string()),
            "PORT" => Some("1234".to_string()),
            _ => None,
        },
    )
    .unwrap();
    assert_eq!(args.host, "127.0.0.1");
    assert_eq!(args.port, 9000);
}

#[test]
fn env_vars_are_used_when_flags_are_absent() {
    let args = Args::try_parse_from(["dora-openarm-webxr"], |key| match key {
        "HOST" => Some("192.168.1.1".to_string()),
        "PORT" => Some("9443".to_string()),
        "TLS_CERTIFICATE_FILE" => Some("env.crt".to_string()),
        "TLS_KEY_FILE" => Some("env.key".to_string()),
        _ => None,
    })
    .unwrap();
    assert_eq!(args.host, "192.168.1.1");
    assert_eq!(args.port, 9443);
    assert_eq!(args.tls_certificate_file, PathBuf::from("env.crt"));
    assert_eq!(args.tls_key_file, PathBuf::from("env.key"));
}

#[test]
fn missing_tls_certificate_without_flag_or_env_is_an_error() {
    let result = Args::try_parse_from(["dora-openarm-webxr", "--tls-key-file", "a.key"], |_| None);
    assert!(result.is_err());
}

#[test]
fn missing_tls_key_without_flag_or_env_is_an_error() {
    let result = Args::try_parse_from(
        ["dora-openarm-webxr", "--tls-certificate-file", "a.crt"],
        |_| None,
    );
    assert!(result.is_err());
}

#[test]
fn host_and_port_defaults_come_from_env_when_present_matching_argparse_semantics() {
    // Upstream: `default=int(os.getenv("PORT", "8443"))` -- the env var is
    // read once as the argparse default, so a present-but-unset-by-flag
    // port still comes from the environment.
    let args = Args::try_parse_from(
        [
            "dora-openarm-webxr",
            "--tls-certificate-file",
            "a.crt",
            "--tls-key-file",
            "a.key",
        ],
        |key| {
            if key == "PORT" {
                Some("8000".to_string())
            } else {
                None
            }
        },
    )
    .unwrap();
    assert_eq!(args.port, 8000);
}

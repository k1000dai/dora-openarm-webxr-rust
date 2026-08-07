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

//! Command-line/environment option parsing, matching upstream `main.py`'s
//! `argparse` setup (including `video.add_arguments`): for each option, `CLI
//! flag > environment variable > hard-coded default`, and
//! `--tls-certificate-file`/`--tls-key-file` are required only when their
//! environment variable is also unset (`argparse`:
//! `required=tls_certificate_file_default is None`).
//!
//! The environment lookup is injected as a closure rather than read directly
//! from the process environment, so option resolution is a pure, testable
//! function instead of something that mutates/reads global state.

use clap::Parser;
use std::fmt;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "dora-openarm-webxr", about = "WebXR server")]
struct RawArgs {
    /// Server port (default: 8443).
    #[arg(long)]
    port: Option<u16>,
    /// Server host (default: 0.0.0.0).
    #[arg(long)]
    host: Option<String>,
    /// TLS certificate file.
    #[arg(long = "tls-certificate-file")]
    tls_certificate_file: Option<PathBuf>,
    /// TLS key file for the certificate file.
    #[arg(long = "tls-key-file")]
    tls_key_file: Option<PathBuf>,
    /// YAML file with the head camera panel parameters.
    #[arg(long = "view-configuration-file")]
    view_configuration_file: Option<PathBuf>,
}

/// Resolved server configuration: CLI flag, then environment variable, then
/// hard-coded default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// The host the web server listens on (`--host`/`HOST`, default `0.0.0.0`).
    pub host: String,
    /// The port the web server listens on (`--port`/`PORT`, default `8443`).
    pub port: u16,
    /// The TLS certificate file for HTTPS (`--tls-certificate-file`/`TLS_CERTIFICATE_FILE`).
    pub tls_certificate_file: PathBuf,
    /// The TLS key file for the certificate (`--tls-key-file`/`TLS_KEY_FILE`).
    pub tls_key_file: PathBuf,
    /// The YAML file describing how the head camera is drawn in the VR
    /// device (`--view-configuration-file`/`VIEW_CONFIGURATION_FILE`).
    /// `None` keeps the built-in default view configuration.
    pub view_configuration_file: Option<PathBuf>,
}

/// An error resolving [`Args`].
#[derive(Debug)]
pub enum ArgsError {
    /// The CLI arguments themselves were invalid (bad flag, bad syntax, `--help`/`--version`).
    InvalidArguments(clap::Error),
    /// `PORT`'s value (from the flag or the environment) wasn't a valid port number.
    InvalidPort(String),
    /// Neither `--tls-certificate-file` nor `TLS_CERTIFICATE_FILE` was set.
    MissingTlsCertificateFile,
    /// Neither `--tls-key-file` nor `TLS_KEY_FILE` was set.
    MissingTlsKeyFile,
}

impl fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgsError::InvalidArguments(err) => write!(f, "{err}"),
            ArgsError::InvalidPort(value) => write!(f, "invalid port: {value:?}"),
            ArgsError::MissingTlsCertificateFile => {
                write!(
                    f,
                    "--tls-certificate-file (or TLS_CERTIFICATE_FILE) is required"
                )
            }
            ArgsError::MissingTlsKeyFile => {
                write!(f, "--tls-key-file (or TLS_KEY_FILE) is required")
            }
        }
    }
}

impl std::error::Error for ArgsError {}

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8443;

impl Args {
    /// Resolves options from `iter` (an argv-like sequence, first element
    /// the program name) and `env`, a lookup for environment variable
    /// values.
    ///
    /// # Errors
    ///
    /// Returns [`ArgsError`] if the arguments are malformed, `PORT` isn't a
    /// valid port number, or a required TLS file is missing from both the
    /// flag and the environment.
    pub fn try_parse_from<I, T>(
        iter: I,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ArgsError>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let raw = RawArgs::try_parse_from(iter).map_err(ArgsError::InvalidArguments)?;

        let host = raw
            .host
            .or_else(|| env("HOST"))
            .unwrap_or_else(|| DEFAULT_HOST.to_string());

        let port = match raw.port {
            Some(port) => port,
            None => match env("PORT") {
                Some(value) => value
                    .parse::<u16>()
                    .map_err(|_| ArgsError::InvalidPort(value))?,
                None => DEFAULT_PORT,
            },
        };

        let tls_certificate_file = raw
            .tls_certificate_file
            .or_else(|| env("TLS_CERTIFICATE_FILE").map(PathBuf::from))
            .ok_or(ArgsError::MissingTlsCertificateFile)?;

        let tls_key_file = raw
            .tls_key_file
            .or_else(|| env("TLS_KEY_FILE").map(PathBuf::from))
            .ok_or(ArgsError::MissingTlsKeyFile)?;

        // Optional: without it the node keeps the built-in default view
        // configuration (`argparse`: no `required=`, `default=os.getenv(...)`).
        let view_configuration_file = raw
            .view_configuration_file
            .or_else(|| env("VIEW_CONFIGURATION_FILE").map(PathBuf::from));

        Ok(Self {
            host,
            port,
            tls_certificate_file,
            tls_key_file,
            view_configuration_file,
        })
    }

    /// Resolves options from the real process arguments and environment.
    ///
    /// # Errors
    ///
    /// See [`Args::try_parse_from`].
    pub fn parse() -> Result<Self, ArgsError> {
        Self::try_parse_from(std::env::args_os(), |key| std::env::var(key).ok())
    }
}

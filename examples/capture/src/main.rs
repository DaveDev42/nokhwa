/*
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use clap::{Parser, Subcommand};
use color_eyre::Report;
use nokhwa::error::NokhwaError;
use nokhwa::format_types::Mjpeg;
use nokhwa::frame::{Frame, IntoRgb};
use nokhwa::utils::{frame_formats, CameraFormat, CameraIndex, FrameFormat, Resolution};
use nokhwa::{
    native_api_backend, nokhwa_initialize, open, query, CameraRunner, OpenRequest, OpenedCamera,
    RunnerConfig, StreamCamera,
};
use std::str::FromStr;
use std::time::Duration;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone)]
enum IndexKind {
    String(String),
    Index(u32),
}

impl FromStr for IndexKind {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u32>() {
            Ok(p) => Ok(IndexKind::Index(p)),
            Err(_) => Ok(IndexKind::String(s.to_string())),
        }
    }
}

impl From<&IndexKind> for CameraIndex {
    fn from(k: &IndexKind) -> Self {
        match k {
            IndexKind::String(s) => CameraIndex::String(s.clone()),
            IndexKind::Index(i) => CameraIndex::Index(*i),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    ListDevices,
    ListProperties {
        device: Option<IndexKind>,
        kind: Option<PropertyKind>,
    },
    Stream {
        device: Option<IndexKind>,
        requested: Option<RequestedCliFormat>,
    },
    Single {
        device: Option<IndexKind>,
        save: Option<String>,
        requested: Option<RequestedCliFormat>,
    },
}

#[derive(Clone)]
struct RequestedCliFormat {
    format_type: String,
    format_option: Option<String>,
}

impl FromStr for RequestedCliFormat {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let splitted = s.split(':').collect::<Vec<&str>>();
        if splitted.is_empty() {
            return Err(Report::msg("empty string"));
        }

        Ok(RequestedCliFormat {
            format_type: splitted[0].to_string(),
            format_option: splitted.get(1).map(|x| (*x).to_string()),
        })
    }
}

impl RequestedCliFormat {
    /// Translate the CLI format into an [`OpenRequest`]. Only the `Exact`
    /// variant is honoured precisely; other variants fall back to
    /// `OpenRequest::any()` (backend picks highest resolution).
    fn into_open_request(self) -> Option<OpenRequest> {
        match self.format_type.as_str() {
            "AbsoluteHighestResolution" | "AbsoluteHighestFrameRate" | "None" => {
                Some(OpenRequest::any())
            }
            "HighestResolution" | "HighestFrameRate" => Some(OpenRequest::any()),
            "Exact" | "Closest" => {
                let fmtv = self.format_option?;
                let values = fmtv.split(',').collect::<Vec<&str>>();
                let x = values[0].parse::<u32>().ok()?;
                let y = values[1].parse::<u32>().ok()?;
                let fps = values[2].parse::<u32>().ok()?;
                let fourcc = values[3].parse::<FrameFormat>().ok()?;
                let camera_format = CameraFormat::new(Resolution::new(x, y), fourcc, fps);
                Some(OpenRequest::with_format(camera_format))
            }
            _ => None,
        }
    }
}

#[derive(Copy, Clone)]
enum PropertyKind {
    All,
    Controls,
    CompatibleFormats,
}

impl FromStr for PropertyKind {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "All" | "ALL" | "all" => Ok(PropertyKind::All),
            "Controls" | "controls" | "CONTROLS" | "ctrls" => Ok(PropertyKind::Controls),
            "CompatibleFormats" | "compatibleformats" | "COMPATIBLEFORMATS" | "cf"
            | "compatfmts" => Ok(PropertyKind::CompatibleFormats),
            _ => Err(Report::msg(format!("unknown PropertyKind: {s}"))),
        }
    }
}

fn main() {
    nokhwa_initialize(|x| {
        println!("Nokhwa Initalized: {x}");
        if let Err(e) = nokhwa_main() {
            eprintln!("error: {e}");
        }
    });
    std::thread::sleep(Duration::from_millis(2000));
}

fn nokhwa_main() -> Result<(), NokhwaError> {
    let cli = Cli::parse();

    let cmd = match cli.command {
        Some(cmd) => cmd,
        None => {
            println!("Unknown command \"\". Do --help for info.");
            return Ok(());
        }
    };

    match cmd {
        Commands::ListDevices => {
            let backend = native_api_backend()
                .ok_or_else(|| NokhwaError::general("no native API backend on this platform"))?;
            let devices = query(backend)?;
            println!("There are {} available cameras.", devices.len());
            for device in devices {
                println!("{device}");
            }
        }
        Commands::ListProperties { device, kind } => {
            let kind = kind.unwrap_or_else(|| {
                println!(
                    "Expected Positional Argument \"All\", \"Controls\", or \"CompatibleFormats\""
                );
                PropertyKind::All
            });
            let index = CameraIndex::from(device.as_ref().unwrap_or(&IndexKind::Index(0)));
            let opened = open(index, OpenRequest::any())?;
            let OpenedCamera::Stream(mut cam) = opened else {
                return Err(NokhwaError::general("expected stream-capable camera"));
            };
            match kind {
                PropertyKind::All => {
                    camera_print_controls(&cam)?;
                    camera_compatible_formats(&mut cam)?;
                }
                PropertyKind::Controls => {
                    camera_print_controls(&cam)?;
                }
                PropertyKind::CompatibleFormats => {
                    camera_compatible_formats(&mut cam)?;
                }
            }
        }
        Commands::Stream { device, requested } => {
            let req = requested
                .and_then(RequestedCliFormat::into_open_request)
                .unwrap_or_else(OpenRequest::any);
            let index = CameraIndex::from(device.as_ref().unwrap_or(&IndexKind::Index(0)));
            let opened = open(index, req)?;
            let runner = CameraRunner::spawn(opened, RunnerConfig::default())?;
            let frames = runner
                .frames()
                .ok_or_else(|| NokhwaError::general("runner exposes no frames channel"))?;
            loop {
                match frames.recv_timeout(Duration::from_secs(2)) {
                    Ok(buf) => println!("Captured frame of size {}", buf.buffer().len()),
                    Err(e) => {
                        return Err(NokhwaError::general(e.to_string()));
                    }
                }
            }
        }
        Commands::Single {
            device,
            save,
            requested,
        } => {
            let req = requested
                .and_then(RequestedCliFormat::into_open_request)
                .unwrap_or_else(OpenRequest::any);
            let index = CameraIndex::from(device.as_ref().unwrap_or(&IndexKind::Index(0)));
            let opened = open(index, req)?;
            let OpenedCamera::Stream(mut camera) = opened else {
                return Err(NokhwaError::general("expected stream-capable camera"));
            };
            camera.open()?;
            let buffer = camera.frame()?;
            camera.close()?;
            println!("Captured Single Frame of {}", buffer.buffer().len());
            let frame: Frame<Mjpeg> = Frame::new(buffer);
            let decoded = frame.into_rgb().materialize()?;
            println!("DecodedFrame of {}", decoded.len());

            if let Some(path) = save {
                println!("Saving to {path}");
                decoded
                    .save(path)
                    .map_err(|e| NokhwaError::general(e.to_string()))?;
            }
        }
    }

    Ok(())
}

fn camera_print_controls(cam: &StreamCamera) -> Result<(), NokhwaError> {
    let ctrls = cam.controls()?;
    let index = cam.info().index();
    println!("Controls for camera {index}");
    for ctrl in ctrls {
        println!("{ctrl}");
    }
    Ok(())
}

fn camera_compatible_formats(cam: &mut StreamCamera) -> Result<(), NokhwaError> {
    let formats = cam.compatible_formats()?;
    for ffmt in frame_formats() {
        let mut by_format: Vec<&CameraFormat> =
            formats.iter().filter(|f| f.format() == *ffmt).collect();
        if by_format.is_empty() {
            continue;
        }
        by_format.sort_by_key(|a| a.resolution());
        println!("{ffmt}:");
        for fmt in by_format {
            println!(
                " - {}: {}fps",
                fmt.resolution(),
                fmt.frame_rate()
            );
        }
    }
    Ok(())
}

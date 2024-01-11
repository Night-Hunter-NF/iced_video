//! # Iced Player
//! A video player built with [iced] and gstreamer.
//! this is a WIP project, so expect bugs and missing features.
//!
//! this is a simple to use viper player api, that can be used to build a video player with iced.
//!
//! ## Features
//! - [x] Play videos from local files and streams
//! - [] Play videos in fullscreen
//! - [x] has a overlay for video controls
#![deny(
    missing_debug_implementations,
    missing_docs,
    unused_results,
    clippy::extra_unused_lifetimes,
    clippy::from_over_into,
    clippy::needless_borrow,
    clippy::new_without_default,
    clippy::useless_conversion,
    unsafe_code
)]
#![forbid(rust_2018_idioms)]
#![allow(clippy::inherent_to_string, clippy::type_complexity)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod backends;

pub mod helpers;
pub mod overlay;
pub mod player_handler;
pub mod viewer;

pub use playbin_core::*;

#[cfg(all(feature = "gstreamer", not(feature = "ffmpeg")))]
pub use gstreamer_playbin::*;

#[cfg(all(feature = "ffmpeg", not(feature = "gstreamer")))]
pub use ffmpeg_playbin::*;


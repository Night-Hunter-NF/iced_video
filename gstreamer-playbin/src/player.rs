use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

pub use crate::error::GstreamerError;
use crate::{extra_functions::send_seek_event, unsafe_functions::is_initialized};
use gst::{
    glib::{Cast, ObjectExt},
    prelude::{ElementExtManual, GstBinExtManual},
    traits::{ElementExt, PadExt},
    BusSyncReply, FlowError, FlowSuccess,
};
use playbin_core::{
    image, smol::lock::Mutex, AdvancedPlayer, BasicPlayer, PlayerBuilder, PlayerMessage,
};
use tracing::{debug, error, info};

/// A gstreamer backend for the player.
#[derive(Debug, Clone)]
pub struct Player {
    playbin: gst::Element,
    bin: gst::Bin,
    ghost_pad: gst::GhostPad,

    settings: PlayerBuilder,

    video_details: Option<VideoDetails>,
    playback_rate: Arc<Mutex<f64>>,
    loop_track: Arc<AtomicBool>,
}

/// stores some details about the video.
#[derive(Clone, Debug)]
pub struct VideoDetails {
    width: i32,
    height: i32,
    framerate: f64,
}

// /// The message that is sent to the main thread.
// #[derive(Debug, Clone)]
// pub enum GstreamerMessage {
//     /// The player id and the player.
//     Player(String, GstreamerBackend),
//     /// The player id and the image.
//     Image(String, image::Handle),
//     /// The player id and the message.
//     Message(String, gst::Message),
// }

impl Player {
    /// Creates a gstreamer player.
    pub fn new(
        settings: PlayerBuilder,
    ) -> (
        Self,
        playbin_core::smol::channel::Receiver<playbin_core::PlayerMessage<Self>>,
    ) {
        let (sender, receiver) = playbin_core::smol::channel::unbounded::<PlayerMessage<Self>>();
        let sender1 = sender.clone();
        let sender2 = sender.clone();
        let id = settings.id.clone();
        let id1 = settings.id.clone();
        let id2 = settings.id.clone();
        let _id3 = settings.id.clone();
        let loop_track = Arc::new(AtomicBool::new(false));
        let loop_track_clone = loop_track.clone();

        let player = Self::build_player(
            settings,
            move |sink: &gst_app::AppSink| {
                let sample = sink.pull_sample().map_err(|_| FlowError::Eos)?;
                let buffer = sample.buffer().ok_or(FlowError::Error)?;
                let map = buffer.map_readable().map_err(|_| FlowError::Error)?;

                let pad = sink.static_pad("sink").ok_or(FlowError::Error)?;

                let caps = pad.current_caps().ok_or(FlowError::Error)?;
                let s = caps.structure(0).ok_or(FlowError::Error)?;
                let width = s.get::<i32>("width").map_err(|_| FlowError::Error)?;
                let height = s.get::<i32>("height").map_err(|_| FlowError::Error)?;

                let res = sender.try_send(PlayerMessage::Frame(
                    id1.clone(),
                    image::Handle::from_pixels(
                        width as u32,
                        height as u32,
                        map.as_slice().to_owned(),
                    ),
                ));

                if res.is_err() {
                    return Err(FlowError::Error);
                }

                Ok(FlowSuccess::Ok)
            },
            move |_, msg, playbin| {
                let mes = msg.view();

                if let gst::MessageView::Eos(_) = mes {
                    println!("eos");
                    if loop_track.load(std::sync::atomic::Ordering::Relaxed) {
                        println!("looping");
                        let pos = Duration::from_secs(2).as_nanos() as u64;
                        playbin
                            .seek(
                                1.0,
                                gst::SeekFlags::FLUSH,
                                gst::SeekType::Set,
                                pos * gst::ClockTime::NSECOND,
                                gst::SeekType::None,
                                gst::ClockTime::NONE,
                            )
                            .unwrap();
                    }
                }
                // let res = sender1.send(GstreamerMessage::Message(id2.clone(), msg.clone()));

                // if res.is_err() {
                //     error!("Error sending message");
                // }
                BusSyncReply::Pass
            },
            loop_track_clone,
        )
        .unwrap();
        (player, receiver)
    }

    /// Builds the player.
    pub fn build_player<C, F>(
        video_settings: PlayerBuilder,
        frame_callback: C,
        message_callback: F,
        loop_track: Arc<AtomicBool>,
    ) -> Result<Self, GstreamerError>
    where
        Self: Sized,
        C: FnMut(&gst_app::AppSink) -> Result<gst::FlowSuccess, gst::FlowError> + Send + 'static,
        F: Fn(&gst::Bus, &gst::Message, gst::Element) -> BusSyncReply + Send + Sync + 'static,
    {
        info!("Initializing Player");

        if !is_initialized() {
            debug!("Initialize GStreamer");
            gst::init()?;
        }

        let playbin = gst::ElementFactory::make("playbin3").build()?;

        playbin.set_property("instant-uri", true);

        let video_convert = gst::ElementFactory::make("videoconvert").build()?;

        let scale = gst::ElementFactory::make("videoscale").build()?;

        let app_sink = gst::ElementFactory::make("appsink")
            .name("sink")
            .build()?
            .dynamic_cast::<gst_app::AppSink>()
            .expect("unable to cast appsink");

        app_sink.set_property("emit-signals", true);

        app_sink.set_caps(Some(
            &gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Rgba)
                .pixel_aspect_ratio(gst::Fraction::new(1, 1))
                .build(),
        ));

        debug!("Create the sink bin and linking");
        // Create the sink bin, add the elements and link them
        let bin = gst::Bin::new();
        bin.add_many(&[&video_convert, &scale, app_sink.as_ref()])?;
        gst::Element::link_many(&[&video_convert, &scale, app_sink.as_ref()])?;

        // callback for video sink
        // creates then sends video handle to subscription
        app_sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(frame_callback)
                .build(),
        );

        let bus = playbin
            .bus()
            .expect("Pipeline without bus. Shouldn't happen!");

        let _id = video_settings.id.clone();

        let playbin_clone = playbin.clone();

        bus.set_sync_handler(move |b, m| message_callback(b, m, playbin_clone.clone()));

        debug!("Create ghost pad");
        let pad = video_convert
            .static_pad("sink")
            .ok_or(GstreamerError::MissingElement("no ghost pad"))?;
        let ghost_pad = gst::GhostPad::with_target(&pad)?;
        ghost_pad.set_active(true)?;
        bin.add_pad(&ghost_pad)?;

        let mut backend = Player {
            playbin,
            bin,
            ghost_pad,
            settings: video_settings,
            video_details: None,
            playback_rate: Arc::new(Mutex::new(1.0)),
            loop_track,
        };

        if let Some(url) = backend.settings.uri.clone() {
            backend.set_source(&url)?;
        };

        info!("player initialized");
        Ok(backend)
    }
}

impl BasicPlayer for Player {
    type Error = GstreamerError;
    fn create(
        player_builder: PlayerBuilder,
    ) -> (
        Self,
        playbin_core::smol::channel::Receiver<playbin_core::PlayerMessage<Self>>,
    )
    where
        Self: Sized,
    {
        Self::new(player_builder)
    }

    fn set_source(&mut self, uri: &std::path::PathBuf) -> Result<(), Self::Error> {
        info!("Setting source to {:?}", uri);
        self.playbin.set_property("uri", &uri);

        self.playbin.set_property("video-sink", &self.bin);

        let _ = self.playbin.set_state(gst::State::Playing)?;

        debug!("Waiting for decoder to get source capabilities");
        // wait for up to 5 seconds until the decoder gets the source capabilities
        let _ = self.playbin.state(gst::ClockTime::from_seconds(5)).0?;
        let caps = self
            .ghost_pad
            .current_caps()
            .ok_or(GstreamerError::MissingElement("current_caps"))?;

        let s = caps
            .structure(0)
            .ok_or(GstreamerError::MissingElement("caps"))?;

        let framerate = s.get::<gst::Fraction>("framerate")?;

        self.video_details = Some(VideoDetails {
            width: s.get::<i32>("width")?,
            height: s.get::<i32>("height")?,
            framerate: framerate.numer() as f64 / framerate.denom() as f64,
        });

        debug!("source capabilities: {:?}", self.video_details);

        if !self.settings.auto_start {
            debug!("auto start false setting state to paused");
            let _ = self.playbin.set_state(gst::State::Paused)?;
        }

        debug!("source set");

        Ok(())
    }

    fn get_source(&self) -> Option<String> {
        self.playbin.property("current-uri")
    }

    fn pause(&self) {
        debug!("set state to paused");
        let _ = self
            .playbin
            .set_state(gst::State::Paused)
            .map_err(|_| GstreamerError::CustomError("Element failed to change its state"))
            .unwrap();
    }

    fn play(&self) {
        debug!("set state to playing");
        let _ = self
            .playbin
            .set_state(gst::State::Playing)
            .map_err(|_| GstreamerError::CustomError("Element failed to change its state"))
            .unwrap();
    }

    fn is_playing(&self) -> bool {
        match self.playbin.state(None).1 {
            gst::State::Playing => true,
            _ => false,
        }
    }

    fn stop(&mut self) {
        debug!("exiting");
        let _ = self.playbin.send_event(gst::event::Eos::new());
    }
}

impl AdvancedPlayer for Player {
    fn set_volume(&self, volume: f64) {
        debug!("volume set to: {}", volume);
        self.playbin.set_property("volume", &volume);
    }

    fn get_volume(&self) -> f64 {
        self.playbin.property("volume")
    }

    fn set_muted(&self, mute: bool) {
        debug!("muted set to: {}", mute);
        self.playbin.set_property("mute", &mute);
    }

    fn get_muted(&self) -> bool {
        self.playbin.property("mute")
    }

    fn set_looping(&self, looping: bool) {
        debug!("looping set to: {}", looping);
        self.loop_track
            .store(looping, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_looping(&self) -> bool {
        self.loop_track.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn seek(&self, time: Duration) -> Result<(), Self::Error> {
        let pos = time.as_nanos() as u64;
        debug!("seeking to: {}", time.as_secs());
        self.playbin
            .seek_simple(gst::SeekFlags::FLUSH, pos * gst::ClockTime::NSECOND)?;
        Ok(())
    }

    fn get_position(&self) -> Duration {
        std::time::Duration::from_nanos(
            self.playbin
                .query_position::<gst::ClockTime>()
                .map_or(0, |pos| pos.nseconds()),
        )
    }

    fn get_duration(&self) -> Duration {
        std::time::Duration::from_nanos(
            self.playbin
                .query_duration::<gst::ClockTime>()
                .map_or(0, |pos| pos.nseconds()),
        )
    }

    fn set_playback_rate(&self, rate: f64) -> Result<(), Self::Error> {
        debug!("set rate to: {}", rate);
        if let Some(mut playback_rate) = self.playback_rate.try_lock() {
            *playback_rate = rate;
            send_seek_event(&self.playbin, rate)?;
        }

        Ok(())
    }

    fn get_playback_rate(&self) -> f64 {
        self.playback_rate.try_lock().unwrap().clone()
    }

    fn restart_stream(&self) -> Result<(), Self::Error> {
        self.play();
        self.seek(Duration::ZERO)?;
        Ok(())
    }
}

// impl PlayerBackend for GstreamerBackend {
//     type Error = GstreamerError;

//     fn set_source(&mut self, uri: &str) -> Result<(), Self::Error> {
//         info!("Setting source to {}", uri);
//         self.playbin.set_property("uri", &uri);

//         self.playbin.set_property("video-sink", &self.bin);

//         let _ = self.playbin.set_state(gst::State::Playing)?;

//         debug!("Waiting for decoder to get source capabilities");
//         // wait for up to 5 seconds until the decoder gets the source capabilities
//         let _ = self.playbin.state(gst::ClockTime::from_seconds(5)).0?;
//         let caps = self
//             .ghost_pad
//             .current_caps()
//             .ok_or(GstreamerError::MissingElement("current_caps"))?;

//         let s = caps
//             .structure(0)
//             .ok_or(GstreamerError::MissingElement("caps"))?;

//         let framerate = s.get::<gst::Fraction>("framerate")?;

//         self.video_details = Some(VideoDetails {
//             width: s.get::<i32>("width")?,
//             height: s.get::<i32>("height")?,
//             framerate: framerate.numer() as f64 / framerate.denom() as f64,
//         });

//         debug!("source capabilities: {:?}", self.video_details);

//         if !self.settings.auto_start {
//             debug!("auto start false setting state to paused");
//             let _ = self.playbin.set_state(gst::State::Paused)?;
//         }

//         debug!("source set");
//         Ok(())
//     }

//     fn get_source(&self) -> Option<String> {
//         self.playbin.property("current-uri")
//     }

//     fn set_volume(&mut self, volume: f64) {
//         debug!("volume set to: {}", volume);
//         self.playbin.set_property("volume", &volume);
//     }

//     fn get_volume(&self) -> f64 {
//         self.playbin.property("volume")
//     }

//     fn set_muted(&mut self, mute: bool) {
//         debug!("muted set to: {}", mute);
//         self.playbin.set_property("mute", &mute);
//     }

//     fn get_muted(&self) -> bool {
//         self.playbin.property("mute")
//     }

//     fn set_looping(&mut self, _looping: bool) {
//         todo!()
//     }

//     fn get_looping(&self) -> bool {
//         todo!()
//     }

//     fn set_paused(&mut self, paused: bool) -> Result<(), Self::Error> {
//         debug!("set paused state to: {}", paused);
//         let _ = self
//             .playbin
//             .set_state(if paused {
//                 gst::State::Paused
//             } else {
//                 gst::State::Playing
//             })
//             .map_err(|_| GstreamerError::CustomError("Element failed to change its state"))?;

//         Ok(())
//     }

//     fn get_paused(&self) -> bool {
//         match self.playbin.state(None).1 {
//             gst::State::Playing => false,
//             _ => true,
//         }
//     }

//     fn seek(&mut self, position: std::time::Duration) -> Result<(), Self::Error> {
//         let pos = position.as_nanos() as u64;
//         debug!("seeking to: {}", position.as_secs());
//         self.playbin
//             .seek_simple(gst::SeekFlags::FLUSH, pos * gst::ClockTime::NSECOND)?;
//         Ok(())
//     }

//     fn get_position(&self) -> std::time::Duration {
//         std::time::Duration::from_nanos(
//             self.playbin
//                 .query_position::<gst::ClockTime>()
//                 .map_or(0, |pos| pos.nseconds()),
//         )
//     }

//     fn get_duration(&self) -> std::time::Duration {
//         std::time::Duration::from_nanos(
//             self.playbin
//                 .query_duration::<gst::ClockTime>()
//                 .map_or(0, |pos| pos.nseconds()),
//         )
//     }

//     fn get_rate(&self) -> f64 {
//         self.playback_rate
//     }

//     fn next_frame(&mut self) -> Result<(), Self::Error> {
//         if let Some(video_sink) = self.playbin.property::<Option<gst::Element>>("video-sink") {
//             debug!("Stepping one frame");
//             // Send the event
//             let step =
//                 gst::event::Step::new(gst::format::Buffers::ONE, self.playback_rate, true, false);
//             match video_sink.send_event(step) {
//                 true => Ok(()),
//                 false => Err("Failed to send seek event to the sink".into()),
//             }
//         } else {
//             Err("No video sink found".into())
//         }
//     }

//     fn previous_frame(&mut self) -> Result<(), Self::Error> {
//         if let Some(video_sink) = self.playbin.property::<Option<gst::Element>>("video-sink") {
//             debug!("Stepping one frame");
//             // Send the event
//             let step =
//                 gst::event::Step::new(gst::format::Buffers::ONE, self.playback_rate, true, false);
//             match video_sink.send_event(step) {
//                 true => Ok(()),
//                 false => Err("Failed to send seek event to the sink".into()),
//             }
//         } else {
//             Err("No video sink found".into())
//         }
//     }

//     fn set_rate(&mut self, rate: f64) -> Result<(), Self::Error> {
//         debug!("set rate to: {}", rate);
//         self.playback_rate = rate;
//         send_seek_event(&self.playbin, rate)?;
//         Ok(())
//     }

//     fn exit(&mut self) -> Result<(), Self::Error> {
//         debug!("exiting");
//         let _ = self.playbin.send_event(gst::event::Eos::new());
//         Ok(())
//     }

//     fn restart_stream(&mut self) -> Result<(), Self::Error> {
//         self.set_paused(false)?;
//         self.seek(Duration::ZERO)?;
//         Ok(())
//     }
// }

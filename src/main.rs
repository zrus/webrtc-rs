// GStreamer
//
// Copyright (C) 2019 Sebastian Dröge <sebastian@centricular.com>
// Copyright (C) 2020 Philippe Normand <philn@igalia.com>
//
// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Library General Public
// License as published by the Free Software Foundation; either
// version 2 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Library General Public License for more details.
//
// You should have received a copy of the GNU Library General Public
// License along with this library; if not, write to the
// Free Software Foundation, Inc., 51 Franklin St, Fifth Floor,
// Boston, MA 02110-1301, USA.

#![recursion_limit = "256"]

use anyhow::bail;
use gst::prelude::*;
use std::sync::{Arc, Weak};

use crate::janus::Args;
use structopt::StructOpt;

#[macro_use]
extern crate log;

mod janus;

// Strong reference to our application state
#[derive(Debug, Clone)]
struct App(Arc<AppInner>);

// Weak reference to our application state
#[derive(Debug, Clone)]
struct AppWeak(Weak<AppInner>);

// Actual application state
#[derive(Debug)]
struct AppInner {
    pipeline: gst::Pipeline,
}

// To be able to access the App's fields directly
impl std::ops::Deref for App {
    type Target = AppInner;

    fn deref(&self) -> &AppInner {
        &self.0
    }
}

impl AppWeak {
    // Try upgrading a weak reference to a strong one
    fn upgrade(&self) -> Option<App> {
        self.0.upgrade().map(App)
    }
}

impl App {
    // Downgrade the strong reference to a weak reference
    fn downgrade(&self) -> AppWeak {
        AppWeak(Arc::downgrade(&self.0))
    }

    fn new(url: String) -> Result<Self, anyhow::Error> {
        // println!("XXXXXXXXXXXXXX Create new App: {}", url);
        //let pipeline = gst::parse_launch(
        //    &format!("webrtcbin name=sendrecv stun-server=stun://stun.l.google.com:19302 \
        //        rtspsrc location={} ! capsfilter caps=\"application/x-rtp,pt=96,clock-rate=90000,media=video,encodeing-name=H264\" ! \
        //        rtph264depay ! avdec_h264 ! videoconvert ! videoscale ! video/x-raw,width=720,height=480 ! x264enc ! rtph264pay ! queue ! \
        //        capsfilter caps=\"application/x-rtp,pt=96,clock-rate=90000,media=video,encoding-name=H264\" ! sendrecv."
        //    , url.as_str())
        //)?;

        let pipeline = gst::parse_launch(
            &format!("webrtcbin name=sendrecv stun-server=stun://stun.l.google.com:19302 \
            rtspsrc location={} ! queue ! \
            rtph264depay ! avdec_h264 ! videoconvert ! videoscale ! video/x-raw,width=1920,height=1080 ! x264enc bitrate=1000 ! \
            rtph264pay ! queue ! capsfilter caps=\"application/x-rtp,pt=96,clock-rate=90000,media=video,encoding-name=H264\" ! sendrecv."
                , url.as_str())
        )?;

        // let pipeline = gst::parse_launch(
        //     &"webrtcbin name=sendrecv stun-server=stun://stun.l.google.com:19302 \
        //     rtspsrc location=rtsp://10.50.13.252/1/h264major ! queue ! \
        //     rtph264depay ! avdec_h264 ! videoconvert ! videoscale ! video/x-raw,width=640,height=480 ! vp8enc target-bitrate=100000 ! \
        //     rtpvp8pay picture-id-mode=2 ! queue ! capsfilter caps=\"application/x-rtp,pt=96,clock-rate=90000,media=video,encoding-name=VP8\" ! sendrecv."
        //         .to_string(),
        // )?;

        // let pipeline = gst::parse_launch(
        //     &"webrtcbin name=sendrecv stun-server=stun://stun.l.google.com:19302 \
        //     rtspsrc location=rtsp://10.50.13.252/1/h264major ! queue ! \
        //     rtph264depay ! avdec_h264 ! videoconvert ! videoscale ! video/x-raw,width=1920,height=1080 ! x264enc bitrate=1000 ! \
        //     rtph264pay ! queue ! capsfilter caps=\"application/x-rtp,pt=96,clock-rate=90000,media=video,encoding-name=H264\" ! sendrecv."
        //         .to_string(),
        // )?;

        // let pipeline = gst::parse_launch(
        //    &"webrtcbin name=sendrecv stun-server=stun://stun.l.google.com:19302 
        //    videotestsrc pattern=ball ! video/x-raw,width=640,height=480 ! videoconvert ! queue !
        //    vp8enc target-bitrate=100000 overshoot=25 undershoot=100 deadline=33000 keyframe-max-dist=1 ! rtpvp8pay ! queue !
        //    application/x-rtp,media=video,encoding-name=VP8,payload=96 ! sendrecv. "
        //    .to_string(),
        // )?;

    //    let pipeline = gst::parse_launch(
    //          &"webrtcbin name=sendrecv stun-server=stun://stun.l.google.com:19302 \
    //          videotestsrc pattern=ball ! video/x-raw,width=640,height=480 ! videoconvert ! queue !
    //          x264enc ! rtph264pay ! queue ! application/x-rtp,media=video,encoding-name=H264,payload=96 ! sendrecv."
    //          .to_string(),
    //      )?;


        let pipeline = pipeline
            .downcast::<gst::Pipeline>()
            .expect("Couldn't downcast pipeline");

        let bus = pipeline.bus().unwrap();
        let app = App(Arc::new(AppInner { pipeline }));

        let app_weak = app.downgrade();
        bus.add_watch_local(move |_bus, msg| {
            let app = upgrade_weak!(app_weak, glib::Continue(false));

            if app.handle_pipeline_message(msg).is_err() {
                return glib::Continue(false);
            }
            glib::Continue(true)
        })
        .expect("Unable to add bus watch");

        Ok(app)
    }

    // fn handle_pipeline_message(&self) -> Result<(), anyhow::Error> {
    //     let bus = self.pipeline.bus().unwrap();

    //     for msg in bus.iter_timed(gst::ClockTime::NONE) {
    //         use gst::message::MessageView;
            
    //         match msg.view() {
    //             MessageView::Error(err) => bail!(
    //                 "Error from element {}: {} ({})",
    //                 err.src()
    //                 .map(|s| String::from(s.path_string()))
    //                 .unwrap_or_else(|| String::from("None")),
    //                 err.error(),
    //                 err.debug().unwrap_or_else(|| String::from("None")),
    //             ),
    //             MessageView::Warning(warning) => {
    //                 println!("Warning: \"{}\"", warning.debug().unwrap());
    //             }
    //             MessageView::Eos(..) => {
    //                 return Ok(())
    //             }
    //             _ => (),
    //         }
    //     }

    //     Ok(())
    // }
    fn handle_pipeline_message(&self, message: &gst::Message) -> Result<(), anyhow::Error> {
        use gst::message::MessageView;

        match message.view() {
            MessageView::Error(err) => bail!(
                "Error from element {}: {} ({})",
                err.src()
                    .map(|s| String::from(s.path_string()))
                    .unwrap_or_else(|| String::from("None")),
                err.error(),
                err.debug().unwrap_or_else(|| String::from("None")),
            ),
            MessageView::Warning(warning) => {
                println!("Warning: \"{}\"", warning.debug().unwrap());
            }
            _ => (),
        }
        Ok(())
    }

    // pub async fn run(&self, url: String) -> Result<(), anyhow::Error> {
    //     let bin = self.pipeline.clone().upcast::<gst::Bin>();
    //     let mut gw = janus::JanusGateway::new(bin, url).await?;

    //     // Asynchronously set the pipeline to Playing
    //     self.pipeline.call_async(|pipeline| {
    //         // If this fails, post an error on the bus so we exit
    //         if pipeline.set_state(gst::State::Playing).is_err() {
    //             gst::element_error!(
    //                 pipeline,
    //                 gst::LibraryError::Failed,
    //                 ("Failed to set pipeline to Playing")
    //             );
    //         }
    //     });

    //     gw.run().await?;
    //     Ok(())
    // }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let bin = self.pipeline.clone().upcast::<gst::Bin>();
        let mut gw = janus::JanusGateway::new(bin).await?;

        // Asynchronously set the pipeline to Playing
        self.pipeline.call_async(|pipeline| {
            // If this fails, post an error on the bus so we exit
            if pipeline.set_state(gst::State::Playing).is_err() {
                gst::element_error!(
                    pipeline,
                    gst::LibraryError::Failed,
                    ("Failed to set pipeline to Playing")
                );
            } else {
                println!("PIPELINE START PLAYING");
            }
        });

        gw.run().await?;
        Ok(())
    }
}

// Make sure to shut down the pipeline when it goes out of scope
// to release any system resources
impl Drop for AppInner {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

// Check if all GStreamer plugins we require are available
fn check_plugins() -> Result<(), anyhow::Error> {
    let needed = [
        "videotestsrc",
        "videoconvert",
        "autodetect",
        "vpx",
        "webrtc",
        "nice",
        "dtls",
        "srtp",
        "rtpmanager",
        "rtp",
    ];

    let registry = gst::Registry::get();
    let missing = needed
        .iter()
        .filter(|n| registry.find_plugin(n).is_none())
        .cloned()
        .collect::<Vec<_>>();

    if !missing.is_empty() {
        bail!("Missing plugins: {:?}", missing);
    } else {
        Ok(())
    }
}

async fn async_main(url: String) -> Result<(), anyhow::Error> {
    gst::init()?;
    check_plugins()?;
    let app = App::new(url)?;
    app.run().await?;
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let args = Args::from_args();
    println!("{:?}", args);
    let url = args.feed_id;
    println!("URL: {}", url);
    let main_context = glib::MainContext::default();
    main_context.block_on(async_main(url))
}

// async fn async_main(url: String) {
//     println!("XXXXXXXXXXXXXXXXX Start cam: {}", url);
//     gst::init().unwrap();
//     check_plugins().unwrap();
//     let app = App::new(url.clone()).unwrap();
//     app.run(url).await.unwrap();
// }

// fn main() {
//     env_logger::init();
//     // let main_context = glib::MainContext::default();
//     // main_context.block_on(async_main())
//     let urls = [
//         "rtsp://10.50.13.252/1/h264major",
//         "rtsp://10.50.13.253/1/h264major",
//         "rtsp://10.50.13.254/1/h264major",
//     ];
//     for url in urls {
//         let url = url.to_owned();
//         std::thread::spawn(async move || {
//             async_main(url).await;
//         }
//         );
//     }

//     loop {}
// }

use crate::wasm_rxing::{convert_js_image_to_luma, decode_barcode};
use gloo::timers::callback::Interval;
use gloo::utils::errors::JsError;
use gloo::utils::window;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement, MediaStream,
    MediaStreamConstraints, MediaStreamTrack, MediaTrackConstraints, VideoFacingModeEnum,
};
use yew::prelude::*;

pub struct Scanner {
    video_ref: NodeRef,
    canvas_ref: NodeRef,
    stream: Option<MediaStream>,
    is_scanning: bool,
    detected_code: Option<String>,
    is_flashlight_on: bool,
    interval: Option<Interval>,
}

pub enum ScannerMessage {
    ReceivedStream(MediaStream),
    CapturedImage,
    Error(JsError),
    ToggleScanner,
    CloseScanner,
    CodeDetected(rxing::RXingResult),
    ToggleFlashlight,
    VideoTimeUpdate,
}

#[derive(Properties, PartialEq, Clone)]
pub struct ScannerProps {
    #[prop_or_default]
    pub onscan: Callback<rxing::RXingResult>,
    #[prop_or_default]
    pub onerror: Callback<JsError>,
    #[prop_or_default]
    pub onclose: Callback<()>,
    #[prop_or(500)]
    pub refresh_milliseconds: u32,
}

impl Scanner {
    fn get_resolution(&self) -> (u32, u32) {
        let video = match self.video_ref.cast::<HtmlVideoElement>() {
            Some(video) => video,
            None => return (300, 300),
        };
        let mut video_height = video.video_height();
        let mut video_width = video.video_width();

        let max_resolution = 800;

        if video_height > max_resolution || video_width > max_resolution {
            let ratio = video_width as f64 / video_height as f64;
            if video_height > video_width {
                video_height = max_resolution;
                video_width = (max_resolution as f64 * ratio) as u32;
            } else {
                video_width = max_resolution;
                video_height = (max_resolution as f64 / ratio) as u32;
            }
        }
        (video_width, video_height)
    }
}

impl Component for Scanner {
    type Message = ScannerMessage;
    type Properties = ScannerProps;

    fn create(_ctx: &Context<Self>) -> Self {
        log::info!("Creating scanner component");
        Self {
            video_ref: NodeRef::default(),
            canvas_ref: NodeRef::default(),
            stream: None,
            is_scanning: false,
            detected_code: None,
            is_flashlight_on: false,
            interval: None,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let time_update = ctx.link().callback(|_| ScannerMessage::VideoTimeUpdate);
        let toggle_scanner = ctx.link().callback(|_| ScannerMessage::ToggleScanner);
        let close_scanner = ctx.link().callback(|_| ScannerMessage::CloseScanner);
        let toggle_flashlight = ctx.link().callback(|_| ScannerMessage::ToggleFlashlight);
        let (video_width, video_height) = self.get_resolution();
        log::info!("Canvas width: {} height: {}", video_width, video_height);
        html! {
            <>
                <style>
                    {"
                        .modal {
                            display: block;
                            position: fixed;
                            z-index: 1;
                            padding-top: 200px;
                            left: 0;
                            top: 0;
                            width: 100%;
                            height: 100%;
                            overflow: auto;
                            background-color: rgb(0,0,0);
                            background-color: rgba(0,0,0,0.4);
                        }
                        .modal-content {
                            background-color: #fefefe;
                            margin: auto;
                            padding: 5px;
                            border: 1px solid #888;
                            width: 80%;
                            max-width: 300px;
                        }
                        .close {
                            color: #aaa;
                            float: right;
                            font-size: 34px;
                            font-weight: bold;
                        }
                        .close:hover,
                        .close:focus {
                            color: black;
                            text-decoration: none;
                            cursor: pointer;
                        }
                    "}
                </style>
                // Button to start or stop the scanner
            if !self.is_scanning {
                <button onclick={toggle_scanner}>{ "Start Scanner" }</button>
            } else {
                <button onclick={&close_scanner}>{ "Stop Scanner" }</button>
            }
            // Modal for the scanner
            if self.is_scanning {
                <div class="modal">
                    <div class="modal-content">
                    <button onclick={&toggle_flashlight}>{ if self.is_flashlight_on { "Turn off Flashlight" } else { "Turn on Flashlight" } }</button> // Add this line
                        <span class="close" onclick={&close_scanner}>{ "×" }</span>
                        <video ref={&self.video_ref} autoPlay="true" style="width:300px;height:300px;" ontimeupdate={time_update}/>
                        <canvas ref={&self.canvas_ref} width={video_width.to_string()} height={video_height.to_string()} style="display: none;"></canvas>
                    </div>
                </div>
                }
            </>
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ScannerMessage::VideoTimeUpdate => {
                if self.interval.is_some() {
                    return false;
                }
                log::info!("Starting interval");
                let link = ctx.link().clone();
                self.interval = Some(Interval::new(ctx.props().refresh_milliseconds, move || {
                    link.send_message(ScannerMessage::CapturedImage);
                }));
                false
            }
            ScannerMessage::ReceivedStream(stream) => {
                self.stream = Some(stream);
                let video = self
                    .video_ref
                    .cast::<HtmlVideoElement>()
                    .expect("video should be an HtmlVideoElement");

                video.set_src_object(self.stream.as_ref().clone());
                true
            }

            ScannerMessage::CapturedImage => {
                if !self.is_scanning {
                    return false;
                }

                let (video_width, video_height) = self.get_resolution();
                log::info!("Capturing image");

                let canvas = self
                    .canvas_ref
                    .cast::<HtmlCanvasElement>()
                    .expect("canvas should be an HtmlCanvasElement");

                let context = canvas
                    .get_context("2d")
                    .expect("context should be available")
                    .unwrap()
                    .unchecked_into::<CanvasRenderingContext2d>();

                let video = self
                    .video_ref
                    .cast::<HtmlVideoElement>()
                    .expect("video should be an HtmlVideoElement");
                match context.draw_image_with_html_video_element(&video, 0.0, 0.0) {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Error: {:?}", e);
                        return true;
                    }
                }

                let image_data =
                    match context.get_image_data(0.0, 0.0, video_width as f64, video_height as f64)
                    {
                        Ok(image_data) => image_data,
                        Err(error) => {
                            log::error!("Error: {:?}", error);
                            return true;
                        }
                    };

                log::info!(
                    "IMAGE SIZE : {} x {}",
                    image_data.width(),
                    image_data.height()
                );
                let decode_result = decode_barcode(
                    convert_js_image_to_luma(image_data.data().as_ref()),
                    image_data.width(),
                    image_data.height(),
                    Some(true),
                    Some(false),
                );
                match decode_result {
                    Ok(s) => {
                        ctx.props().onscan.emit(s.clone());
                        ctx.link()
                            .send_message(ScannerMessage::CodeDetected(s.clone()));
                        ctx.link().send_message(ScannerMessage::CloseScanner);
                    }
                    Err(e) => {
                        ctx.link().send_message(ScannerMessage::Error(JsError::from(
                            js_sys::Error::new(e.to_string().as_str()),
                        )));
                    }
                }
                true
            }
            ScannerMessage::Error(e) => {
                ctx.props().onerror.emit(e);
                true
            }
            ScannerMessage::ToggleScanner => {
                ctx.link().send_future(async {
                    let mut constraints = MediaStreamConstraints::new();
                    let mut video_constraints = MediaTrackConstraints::new();

                    let advanced_constraints = js_sys::Array::new();
                    let torch_constraint = js_sys::Object::new();
                    js_sys::Reflect::set(
                        &torch_constraint,
                        &JsValue::from_str("torch"),
                        &JsValue::from_bool(false),
                    )
                    .unwrap();
                    advanced_constraints.push(&torch_constraint);
                    video_constraints.advanced(&advanced_constraints);

                    video_constraints
                        .facing_mode(&VideoFacingModeEnum::Environment.into())
                        .frame_rate(&20.into());

                    constraints.video(&video_constraints);
                    match window().navigator().media_devices() {
                        Ok(devs) => match devs.get_user_media_with_constraints(&constraints) {
                            Ok(promise) => {
                                match wasm_bindgen_futures::JsFuture::from(promise).await {
                                    Ok(stream) => {
                                        ScannerMessage::ReceivedStream(stream.unchecked_into())
                                    }
                                    Err(e) => ScannerMessage::Error(JsError::try_from(e).unwrap()),
                                }
                            }
                            Err(e) => ScannerMessage::Error(JsError::try_from(e).unwrap()),
                        },
                        Err(e) => ScannerMessage::Error(JsError::try_from(e).unwrap()),
                    }
                });
                self.is_scanning = !self.is_scanning;
                true
            }
            ScannerMessage::CodeDetected(code) => {
                self.detected_code = Some(code.getText().to_string());
                true
            }
            ScannerMessage::CloseScanner => {
                // close event
                if let Some(stream) = self.stream.as_ref() {
                    for track in stream.get_tracks().iter() {
                        if let Ok(track) = track.dyn_into::<MediaStreamTrack>() {
                            track.stop();
                        }
                    }
                }

                self.is_scanning = false;
                self.stream = None;
                self.is_flashlight_on = false;
                if let Some(interval) = self.interval.take() {
                    interval.cancel();
                }
                ctx.props().onclose.emit(());
                true
            }
            ScannerMessage::ToggleFlashlight => {
                if let Some(stream) = &self.stream {
                    let track = stream
                        .get_video_tracks()
                        .get(0)
                        .dyn_into::<MediaStreamTrack>();
                    let constraints = js_sys::Object::new();
                    js_sys::Reflect::set(
                        &constraints,
                        &JsValue::from_str("torch"),
                        &JsValue::from_bool(!self.is_flashlight_on),
                    )
                    .unwrap();
                    let advanced_constraints = js_sys::Array::new();
                    advanced_constraints.push(&constraints);
                    let mut video_constraints = MediaTrackConstraints::new();
                    video_constraints
                        .advanced(&advanced_constraints)
                        .facing_mode(&VideoFacingModeEnum::Environment.into())
                        .frame_rate(&20.into());
                    let _ = track
                        .expect("Cannot apply constrait")
                        .apply_constraints_with_constraints(&video_constraints)
                        .unwrap();
                    self.is_flashlight_on = !self.is_flashlight_on;
                }
                true
            }
        }
    }
}

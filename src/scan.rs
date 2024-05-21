use bardecoder::default_decoder;
use gloo::timers::callback::Interval;
use gloo::utils::errors::JsError;
use gloo::utils::window;
use image::DynamicImage;
use js_sys::Uint8Array;
use std::sync::Arc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{
    Blob, CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement, MediaStream,
    MediaStreamConstraints, MediaStreamTrack, MediaTrackConstraints, VideoFacingModeEnum,
};
use yew::prelude::*;

pub struct Scanner {
    video_ref: NodeRef,
    canvas_ref: NodeRef,
    stream: Option<MediaStream>,
    scanner_interval: Option<Interval>,
    canvas_closure: Option<Arc<Closure<dyn Fn(Blob) -> ()>>>,
    resolution: (u32, u32),
    is_scanning: bool,
    detected_code: Option<String>,
}

pub enum ScannerMessage {
    ReceivedStream(MediaStream),
    CapturedImage(Blob),
    Error(JsError),
    DynamicImage(DynamicImage),
    ToggleScanner,
    CloseScanner,
    CodeDetected(String),
}

#[derive(Properties, PartialEq, Clone)]
pub struct ScannerProps {
    #[prop_or_default]
    pub onscan: Callback<String>,
    #[prop_or_default]
    pub onerror: Callback<JsError>,
    #[prop_or_default]
    pub onclose: Callback<()>,
}

impl Component for Scanner {
    type Message = ScannerMessage;
    type Properties = ScannerProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            video_ref: NodeRef::default(),
            canvas_ref: NodeRef::default(),
            stream: None,
            scanner_interval: None,
            canvas_closure: None,
            resolution: (512, 512),
            is_scanning: false,
            detected_code: None,
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let toggle_scanner = ctx.link().callback(|_| ScannerMessage::ToggleScanner);
        let close_scanner = ctx.link().callback(|_| ScannerMessage::CloseScanner);
        html! {
            <>
                // if scanner is not scanning, show the start scanner button
                // else show the close scanner button
                if !self.is_scanning {
                    <button onclick={toggle_scanner}>{ "Start Scanner" }</button>
                } else {
                    <button onclick={close_scanner}>{ "Stop Scanner" }</button>
                }
                if self.is_scanning {
                    <div>
                        <video ref={&self.video_ref} autoPlay="true" style="width:300px;height:300px;" />
                        <canvas ref={&self.canvas_ref} width={self.resolution.0.to_string()} height={self.resolution.1.to_string()} style="display: none;"></canvas>
                    </div>
                }
            </>
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, _first_render: bool) {
        if !self.is_scanning {
            return;
        }

        let video = self
            .video_ref
            .cast::<HtmlVideoElement>()
            .expect("video should be an HtmlVideoElement");
        let mut video_height = video.video_height();
        let mut video_width = video.video_width();

        if video_height > 800 || video_width > 800 {
            let ratio = video_width as f64 / video_height as f64;
            if video_height > video_width {
                video_height = 800;
                video_width = (800.0 * ratio) as u32;
            } else {
                video_width = 800;
                video_height = (800.0 / ratio) as u32;
            }
        }

        self.resolution = (video_width.max(50), video_height.max(50));

        video.set_src_object(self.stream.as_ref().clone());

        let canvas = self
            .canvas_ref
            .cast::<HtmlCanvasElement>()
            .expect("canvas should be an HtmlCanvasElement");

        let context = canvas
            .get_context("2d")
            .expect("context should be available")
            .unwrap()
            .unchecked_into::<CanvasRenderingContext2d>();
        let link = ctx.link().clone();
        let link_callback = link.clone();
        self.canvas_closure = Some(Arc::new(Closure::wrap(Box::new(move |blob: Blob| {
            link_callback.send_message(ScannerMessage::CapturedImage(blob));
        }) as Box<dyn Fn(Blob)>)));
        let canvas_closure_ref = self.canvas_closure.as_ref().unwrap().clone();

        let width = canvas.width() as f64;
        let height = canvas.height() as f64;

        self.scanner_interval = Some(Interval::new(500, move || {
            context
                .draw_image_with_html_video_element_and_dw_and_dh(&video, 0.0, 0.0, width, height)
                .expect("rendering to canvas should work");
            canvas
                .to_blob(canvas_closure_ref.as_ref().as_ref().unchecked_ref())
                .expect("getting blob failed");
        }));
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ScannerMessage::ReceivedStream(stream) => {
                self.stream = Some(stream);
                true
            }

            ScannerMessage::DynamicImage(image) => {
                let decoder = default_decoder();
                for decode_result in decoder.decode(&image).iter() {
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
                }
                true
            }
            ScannerMessage::CapturedImage(image_src) => {
                ctx.link().send_future(async move {
                    match wasm_bindgen_futures::JsFuture::from(image_src.array_buffer()).await {
                        Ok(array_buffer) => {
                            let array = Uint8Array::new(&array_buffer);
                            let bytes: Vec<u8> = array.to_vec();
                            match image::load_from_memory(&bytes) {
                                Ok(image) => ScannerMessage::DynamicImage(image),
                                Err(e) => {
                                    let error = js_sys::Error::new(e.to_string().as_str());
                                    ScannerMessage::Error(JsError::from(error))
                                }
                            }
                        }
                        Err(e) => ScannerMessage::Error(JsError::try_from(e).unwrap()),
                    }
                });
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
                self.detected_code = Some(code);
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
                self.scanner_interval = None;
                ctx.props().onclose.emit(());
                true
            }
        }
    }
}

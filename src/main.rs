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
    MediaStreamConstraints, MediaTrackConstraints, VideoFacingModeEnum,
};
use yew::prelude::*;

pub struct Scanner {
    video_ref: NodeRef,
    canvas_ref: NodeRef,
    stream: Option<MediaStream>,
    scanner_interval: Option<Interval>,
    canvas_closure: Option<Arc<Closure<dyn Fn(Blob) -> ()>>>,
    resolution: (u32, u32),
}

pub enum ScannerMessage {
    ReceivedStream(MediaStream),
    CapturedImage(Blob),
    Error(JsError),
    DynamicImage(DynamicImage),
}

#[derive(Properties, PartialEq, Clone)]
pub struct ScannerProps {
    #[prop_or_default]
    pub onscan: Callback<String>,
    #[prop_or_default]
    pub onerror: Callback<JsError>,
}

impl Component for Scanner {
    type Message = ScannerMessage;
    type Properties = ScannerProps;

    fn create(ctx: &Context<Self>) -> Self {
        ctx.link().send_future(async {
            let mut constraints = MediaStreamConstraints::new();
            let mut video_constraints = MediaTrackConstraints::new();
            video_constraints
                .facing_mode(&VideoFacingModeEnum::Environment.into())
                .frame_rate(&4.into());
            constraints.video(&video_constraints);
            match window().navigator().media_devices() {
                Ok(devs) => match devs.get_user_media_with_constraints(&constraints) {
                    Ok(promise) => match wasm_bindgen_futures::JsFuture::from(promise).await {
                        Ok(stream) => ScannerMessage::ReceivedStream(stream.unchecked_into()),
                        Err(e) => ScannerMessage::Error(JsError::try_from(e).unwrap()),
                    },
                    Err(e) => ScannerMessage::Error(JsError::try_from(e).unwrap()),
                },
                Err(e) => ScannerMessage::Error(JsError::try_from(e).unwrap()),
            }
        });
        Self {
            video_ref: NodeRef::default(),
            canvas_ref: NodeRef::default(),
            stream: None,
            scanner_interval: None,
            canvas_closure: None,
            resolution: (512, 512),
        }
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
                        }
                        Err(e) => {
                            log::error!("Error MINE: {}", e);
                            ctx.link().send_message(ScannerMessage::Error(JsError::from(
                                js_sys::Error::new(e.to_string().as_str()),
                            )))
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
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let (video_width, video_height) = self.resolution;
        let html = html! {
            <>
            <video ref={&self.video_ref} autoPlay="true" style="width:300px;height:300px;" />
            <canvas ref={&self.canvas_ref} width={video_width.to_string()} height={video_height.to_string()} style="display: none;"></canvas>
            </>
        };
        html
    }

    fn rendered(&mut self, ctx: &Context<Self>, _first_render: bool) {
        let video = self
            .video_ref
            .cast::<HtmlVideoElement>()
            .expect("video should be an HtmlVideoElement");
        let mut video_height = video.video_height();
        let mut video_width = video.video_width();

        // if height or width is higher than 512, set it to 512 and adjust the other with the same ratio
        // as the original video
        if video_height > 512 || video_width > 512 {
            let ratio = video_width as f64 / video_height as f64;
            if video_height > video_width {
                video_height = 512;
                video_width = (512.0 * ratio) as u32;
            } else {
                video_width = 512;
                video_height = (512.0 / ratio) as u32;
            }
        }

        self.resolution = (video_width.max(50), video_height.max(50));

        log::info!("Video size: {}x{}", video_width, video_height);
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
        log::info!("Canvas size: {}x{}", width, height);

        self.scanner_interval = Some(Interval::new(500, move || {
            context
                .draw_image_with_html_video_element_and_dw_and_dh(&video, 0.0, 0.0, width, height)
                .expect("rendering to canvas should work");
            canvas
                .to_blob(canvas_closure_ref.as_ref().as_ref().unchecked_ref())
                .expect("getting blob failed");
        }));
    }
}

#[function_component(App)]
fn app() -> Html {
    let on_scan = Callback::from(|s: String| {
        log::info!("Scanned: {}", s);
        let window = web_sys::window().expect("window should be available");
        window.alert_with_message(&s).expect("alert should work");
    });

    html! {
        <Scanner onscan={on_scan}
        onerror={Callback::from(|e: JsError| {
            log::error!("Error: {}", e);
        })}
        />
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<App>();
}

use gloo::utils::errors::JsError;
use yew::prelude::*;

pub mod scan;
pub mod wasm_rxing;
use crate::scan::Scanner;

#[function_component(App)]
fn app() -> Html {
    let value = use_state(String::new);
    let on_scan = {
        let value = value.clone();
        Callback::from(move |s: rxing::RXingResult| {
            value.set(s.getText().to_string());
        })
    };

    html! {
        <div>
        <Scanner onscan={on_scan}
        onerror={Callback::from(|e: JsError| {
            log::error!("Error: {}", e);
        })}
        />
        <label>{&*value}</label>
        </div>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app::<App>();
}

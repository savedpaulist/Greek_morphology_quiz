mod app;
mod db;
mod models;
#[cfg(test)]
mod paradigm;
mod quiz;

#[cfg(not(target_os = "android"))]
use dioxus_desktop::{Config, tao::window::Icon};

#[cfg(target_os = "android")]
fn main() {
    dioxus::LaunchBuilder::mobile().launch(app::app_root);
}

#[cfg(not(target_os = "android"))]
fn main() {
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_icon(load_desktop_icon()))
        .launch(app::app_root);
}

#[cfg(not(target_os = "android"))]
fn load_desktop_icon() -> Icon {
    let png_bytes = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/dioxus/assets/icon-512.png"));
    let image = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
        .expect("failed to decode desktop icon PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).expect("failed to build desktop icon")
}


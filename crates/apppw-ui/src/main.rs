mod adapters;
mod application;
mod domain;

use adapters::{
    demo::{demo_events, demo_processes},
    egui_ui::AppWatch,
};
use application::AppState;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("AppWatch")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1040.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AppWatch",
        options,
        Box::new(|context| {
            let state = AppState::new(demo_processes(), demo_events());
            Ok(Box::new(AppWatch::new(context, state)))
        }),
    )
}

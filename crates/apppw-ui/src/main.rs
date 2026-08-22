mod adapters;
mod application;
mod domain;
mod runtime;

use adapters::egui_ui::AppWatch;
use application::AppState;
use runtime::Runtime;

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
            let runtime = Runtime::start(context.egui_ctx.clone());
            let state = AppState::new(runtime.processes.clone(), Vec::new());
            Ok(Box::new(AppWatch::new(context, state, runtime)))
        }),
    )
}

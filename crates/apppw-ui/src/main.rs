mod adapters;
mod application;
mod domain;
mod runtime;

use std::{env, path::Path};

use adapters::egui_ui::AppWatch;
use application::AppState;
use apppw_windows::{launch_through_proxy, relaunch_through_proxy};
use runtime::Runtime;

const PROXY_URL: &str = "http://127.0.0.1:8877";

fn main() -> eframe::Result {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if !arguments.is_empty() {
        return match run_cli(&arguments) {
            Ok(()) => Ok(()),
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(1);
            }
        };
    }

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

fn run_cli(arguments: &[String]) -> Result<(), String> {
    match arguments[0].as_str() {
        "launch" => {
            let executable = arguments.get(1).ok_or_else(|| usage().to_owned())?;
            let pid = launch_through_proxy(Path::new(executable), &arguments[2..], PROXY_URL)?;
            println!("Launched {executable} through {PROXY_URL} (PID {pid}).");
            println!(
                "Start the AppWatch GUI if it is not running, otherwise the proxy is offline."
            );
            Ok(())
        }
        "relaunch" => {
            let pid: u32 = arguments
                .get(1)
                .ok_or_else(|| usage().to_owned())?
                .parse()
                .map_err(|_| format!("relaunch expects a numeric PID\n\n{}", usage()))?;
            relaunch_through_proxy(pid, PROXY_URL)?;
            println!("Relaunched PID {pid} through {PROXY_URL}.");
            Ok(())
        }
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n\n{}", usage())),
    }
}

fn usage() -> &'static str {
    "Usage:\n  \
        apppw-ui                      Start the AppWatch GUI\n  \
        apppw-ui launch <exe> [args]  Launch an executable through the proxy\n  \
        apppw-ui relaunch <pid>       Restart a running process through the proxy\n\n\
    The AppWatch GUI must be running for proxy traffic to be captured."
}

// SPDX-License-Identifier: Apache-2.0
//! OpenStage Capture Express. Run with nothing after its name it opens the window (Home and the Editor, over an example
//! guide: milestone M5). It cannot capture yet (milestone M6). `export-demo --out <folder>` makes every export of a made-up guide.

#[cfg(windows)]
mod capture_win;
#[cfg(windows)]
mod check_computer;
#[cfg(windows)]
mod clicks_test;
mod demo_run;
mod guide_writer;
mod library;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
OpenStage Capture Express (the window has an example guide; capturing is not built yet)

  openstage-capture-express                          open the window
  openstage-capture-express --screen editor          start in the editor (or: home)
  openstage-capture-express --theme dark             use the dark or the light colours (or: light) instead of Windows' setting
  openstage-capture-express --screenshot <file.png>  save a picture of the window, then close it
  openstage-capture-express --close-after <seconds>  close the window by itself
  openstage-capture-express export-demo --out <dir>  make every export of a made-up guide in a new folder inside <dir>
  openstage-capture-express check-computer           check that this computer is ready to capture (sends no keys, moves nothing)
  openstage-capture-express test-clicks --out <dir> [--wait <seconds>]
                                                     click and type in a test window while capturing, and check the steps (moves the pointer;
                                                     waits for a quiet computer, up to 90 seconds or <seconds>)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => match ui::run(ui::DevOptions::default()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("The window could not be opened: {error}");
                ExitCode::FAILURE
            }
        },
        Some("--version") | Some("--help") => {
            println!(
                "OpenStage Capture Express {} (the window has an example guide; capturing is not built yet)",
                env!("CARGO_PKG_VERSION")
            );
            if args.first().map(String::as_str) == Some("--help") {
                print!("{USAGE}");
            }
            ExitCode::SUCCESS
        }
        Some("export-demo") => {
            let out = match args.as_slice() {
                [_, flag, folder] if flag == "--out" => PathBuf::from(folder),
                _ => {
                    eprintln!("export-demo needs a folder: export-demo --out <dir>");
                    return ExitCode::from(2);
                }
            };
            match demo_run::run(&out) {
                Ok(report) => {
                    print!("{report}");
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("{message}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("check-computer") => {
            #[cfg(windows)]
            {
                if check_computer::run(&library::default_folder()) {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            #[cfg(not(windows))]
            {
                eprintln!("The computer check needs Windows.");
                ExitCode::from(2)
            }
        }
        Some("test-window") => {
            #[cfg(windows)]
            {
                match openstage_express_windows::testwin::run_controls_window() {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(message) => {
                        eprintln!("{message}");
                        ExitCode::FAILURE
                    }
                }
            }
            #[cfg(not(windows))]
            {
                eprintln!("The click test needs Windows.");
                ExitCode::from(2)
            }
        }
        Some("test-clicks") => {
            let (out, wait) = match args.as_slice() {
                [_, flag, folder] if flag == "--out" => (PathBuf::from(folder), 90),
                [_, flag, folder, wait_flag, seconds] if flag == "--out" && wait_flag == "--wait" => {
                    match seconds.parse::<u64>() {
                        Ok(seconds) => (PathBuf::from(folder), seconds),
                        Err(_) => {
                            eprintln!("--wait needs a number of seconds");
                            return ExitCode::from(2);
                        }
                    }
                }
                _ => {
                    eprintln!("test-clicks needs a folder: test-clicks --out <dir> [--wait <seconds>]");
                    return ExitCode::from(2);
                }
            };
            #[cfg(windows)]
            {
                if clicks_test::run(&out, std::time::Duration::from_secs(wait)) {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            #[cfg(not(windows))]
            {
                let _ = (out, wait);
                eprintln!("The click test needs Windows.");
                ExitCode::from(2)
            }
        }
        Some(flag) if flag.starts_with("--") => match ui::DevOptions::parse(&args) {
            Ok(options) => match ui::run(options) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("The window could not be opened: {error}");
                    ExitCode::FAILURE
                }
            },
            Err(message) => {
                eprintln!("{message}\n{USAGE}");
                ExitCode::from(2)
            }
        },
        Some(other) => {
            eprintln!("Unknown command {other:?}.\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

// SPDX-License-Identifier: Apache-2.0
//! OpenStage Capture Express. Run with nothing after its name it opens the window (Home and the Editor, over an example
//! guide: milestone M5). It cannot capture yet (milestone M6). `export-demo --out <folder>` makes every export of a made-up guide.

mod demo_run;
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

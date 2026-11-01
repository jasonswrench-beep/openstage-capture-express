// SPDX-License-Identifier: Apache-2.0
//! OpenStage Capture Express. So far it has no window (the window comes in milestone M5) and cannot capture (milestone M6).
//! What it can do is run the export demo (`export-demo --out <folder>`), which makes every export of a made-up guide.

mod demo_run;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
OpenStage Capture Express (build skeleton: no window and no capture yet)

  openstage-capture-express                          say what this is
  openstage-capture-express export-demo --out <dir>  make every export of a made-up guide in a new folder inside <dir>
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("--version") | Some("--help") => {
            println!(
                "OpenStage Capture Express {} (build skeleton: no window and no capture yet)",
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
        Some(other) => {
            eprintln!("Unknown command {other:?}.\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

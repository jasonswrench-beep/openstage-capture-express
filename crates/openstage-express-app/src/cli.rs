// SPDX-License-Identifier: Apache-2.0
//! `openstage-capture-express-cli.exe`: the same program as `openstage-capture-express.exe`, built as a console program for the scripts
//! (`EXPORT DEMO.bat`, `CHECK EXPRESS COMPUTER.bat`, `TEST EXPRESS CLICKS.bat`, `BUILD EXPRESS.bat`). A command prompt waits for a
//! console program and passes its exit code on, and what it prints can be sent to a log file. It is not shipped.

fn main() -> std::process::ExitCode {
    openstage_express_app::run(false)
}

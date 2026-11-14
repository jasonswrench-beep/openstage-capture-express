# Check for updates

Since 0.1.0 alpha 4, Express has **Help > Check for updates...**, and a **Check for updates** button on the About screen. It follows the
pattern every tool of the OpenStage suite uses for this button.

## What it does

* **Only when chosen.** Nothing is checked at start-up or on a timer, and nothing runs in the background.
* **Notify only.** It says whether a newer release is published and, if so, offers that release's page on GitHub (what changed, the
  checksum, the signed setup). It downloads and installs nothing. Running the new setup updates the copy in place and keeps the
  guides, the exports and the settings.
* **Where it looks.** One HTTPS request to `https://api.github.com/repos/jasonswrench-beep/openstage-capture-express/releases?per_page=30`,
  made with Windows' own HTTPS code (WinHTTP), so no web library is added to the program. Windows checks the certificate. No redirect is
  followed, no cookie is kept, at most 2 MB is read, and each step gives up after 15 seconds. Pre-releases count, because every release
  so far is one; drafts never do. The request carries only a plain name for the program (`OpenStageCaptureExpress-update-check`).
  Nothing about the person, their guides or their computer is sent.
* **What it compares.** This copy's exact release with the highest published tag (`vMAJOR.MINOR.PATCH`, or that with `-alpha.N`,
  `-beta.N` or `-rc.N`; a release sorts after its own pre-releases). The release is recorded when the program is built:
  `installer\release.txt` becomes `OPENSTAGE_RELEASE` in `MAKE EXPRESS INSTALLER.bat` and `PACK EXPRESS.bat`. Raise it for every
  release. A copy built any other way is told which release is the newest.
* **Links.** Only a page of Express's releases (the list, or `.../releases/tag/v...`) is ever offered, and Windows opens it in the
  person's own browser.
* **Turning it off.** Set `OPENSTAGE_UPDATE_CHECK=off` in the environment, or put a file `update-check.txt` that says `off` beside
  `openstage-capture-express.exe` (the way an IT department turns it off for the copies it installs). The answer then says that
  checking is turned off on this computer, and nothing is asked of the network.
* **Signing.** No update-signing key is needed, because nothing is installed automatically. The setup itself is signed with the
  publisher's code-signing certificate, and the release page carries its SHA-256.

It is the one network request Express makes. `tools/check_rules.py` fails if WinHTTP, or any other way of reaching the network, is
used anywhere but `crates/openstage-express-windows/src/updates.rs`, or if that file is pointed at any host but GitHub's API.

## Code and tests

* `crates/openstage-express-core/src/updates.rs`: the comparison, the answers in words, which pages may be opened, and the switch.
  Its tests need no network.
* `crates/openstage-express-windows/src/updates.rs`: the WinHTTP request and opening the page. One test reaches GitHub, and runs only
  by hand (`cargo test -p openstage-express-windows -- --ignored`): it reads the real list and checks that a redirect is refused.
* `crates/openstage-express-app/src/ui/about.rs` and `app.rs`: the menu command, the check on a thread of its own, and the answer on
  the About screen. `ui/tests.rs` drives it with a stand-in that never reaches the network.

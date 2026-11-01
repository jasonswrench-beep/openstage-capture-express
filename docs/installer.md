# The installer

OpenStage Capture Express has two ways to be given to someone: the portable zip (`PACK EXPRESS.bat`, spec A6) and a setup file
(`MAKE EXPRESS INSTALLER.bat`). Both put the product icon, `assets/icon/OpenCapture_Express.ico`, on the program file. Neither is
signed yet.

## Making it

Double-click `MAKE EXPRESS INSTALLER.bat`. It builds the program, then runs `tools/make-installer.ps1`, which

1. copies the built program into a temporary folder and adds the product icon to that copy (`tools/set-exe-icon.ps1`, which uses
   Windows' own resource calls and downloads nothing), so the program in `target/` is never changed;
2. adds `LICENSE`, `NOTICE`, `THIRD-PARTY.md` and a `START-HERE.txt` written for the installed program;
3. draws the setup wizard's pictures from the product icon (a tall picture for the first and last pages in three sizes, for screens
   set to 100, 150 and 200 percent, and a small picture for the corner of the other pages);
4. has Inno Setup's compiler make `dist/OpenStage-Capture-Express-<version>-x64-setup.exe` from `installer/express.iss`, and writes
   its SHA-256 beside it (`...-setup.exe.sha256.txt`).

It never replaces a setup file that is already there; move the old one away first. The full log is `installer-log.txt`.

## Inno Setup

The installer is made with **Inno Setup** (free; <https://jrsoftware.org/isinfo.php>), version 7.1.1 on this PC, installed in
`C:\Users\Jason\InnoSetup7` (the build looks there, then in the usual Program Files places, or in the full path given in the
environment variable `EXPRESS_ISCC`). Its licence (`license.txt` in its folder) allows use "for any purpose, including commercial
applications", and the installers it makes carry no licence conditions beyond keeping Inno Setup's own copyright notices in the setup
file, which it does by itself. Its authors ask commercial users to buy a licence and anyone to donate; that is a request, not a
condition. The download was checked before it was run: its Authenticode signature is valid and names Pyrsys B.V. (the publisher the
Inno Setup site says to expect), and its SHA-256 matches the hash on the project's immutable GitHub release
(`8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0` for `innosetup-7.1.1-x64.exe`).

Inno Setup is a build tool, not part of the program: nothing in `Cargo.lock` or `THIRD-PARTY.md` changes. The setup file itself
contains Inno Setup's setup program, so `NOTICE` acknowledges it.

## What the setup file does

* Installs for the person using it, with no administrator rights, into `%LOCALAPPDATA%\Programs\OpenStage Capture Express`. The
  person may choose "install for all users" (Windows then asks for administrator rights) or another folder.
* Needs Windows 10 version 2004 or later (build 19041), 64-bit. It is also allowed on Windows 11 for Arm, where x64 programs run in
  emulation; that has not been tried.
* Puts the program, `LICENSE`, `NOTICE`, `THIRD-PARTY.md`, `START-HERE.txt` and `OpenCapture_Express.ico` in the install folder.
* Makes a Start menu shortcut, and a desktop shortcut only if the person ticks it. Both use the product icon.
* Writes one registry entry: the program's entry in Windows' list of installed programs (the person's own `HKCU` side), which shows
  the product icon. No file types, no context menu, no startup entry, no network.
* Offers to open the program when it finishes.
* If the program is running, asks to close it first.
* The wizard shows the product icon in its first and last pages, the small version in the corner of the others, and the setup file's
  own icon is the product icon.

Removing it (Settings, Apps, or the uninstaller in the install folder) deletes only what the setup file put there. Guides, exports
and settings are never deleted, and the program says so when it finishes. A newer setup file installs over the old program; the id in `installer/express.iss` (`AppId`) must never change, or Windows would treat a new version
as a second program.

For a quiet install by a person who manages computers: `setup.exe /VERYSILENT /CURRENTUSER /NORESTART` (add `/DIR="C:\Some folder"`
to choose the folder, `/TASKS="desktopicon"` for the desktop shortcut, and `/LOG="C:\file.txt"` for a log).

## Not done

* **Signing.** The setup file and the program are unsigned, so Windows SmartScreen may warn when the setup file is run, and the
  wizard's first page says so. Signing is spec A10 and a separate step with its own costs; sign the program first and the setup file
  after, because adding the icon changes a program file and breaks its signature.
* No automatic update (spec A6: none).
* English only.
* The installer has been run, but not by hand through its screens: see `TESTING-TODO.md`.

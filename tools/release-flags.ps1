# SPDX-License-Identifier: Apache-2.0
# Prints the Rust flags a release of Express is built with, for MAKE EXPRESS INSTALLER.bat and PACK EXPRESS.bat to put in
# CARGO_ENCODED_RUSTFLAGS (its pieces are joined by the character 31, which keeps a space in a folder name whole).
#
# Each folder whose name should not be written into the program is mapped to a plain name: this computer's user folder, the Rust
# folders and this repository (the security review of 2026-11-13: the program held the Windows user name and the folder it was built
# in). When several prefixes match, Rust takes the last, so the broadest (the user folder) comes first and the repository last. The
# linker is told to name the debug file by its name alone. The folders are worked out here, on the computer that builds; none is
# written into the repository.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$userHome = [Environment]::GetFolderPath('UserProfile')
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $userHome '.cargo' }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $userHome '.rustup' }
$flags = @(
    "--remap-path-prefix=$userHome=/home",
    "--remap-path-prefix=$cargoHome=/cargo",
    "--remap-path-prefix=$rustupHome=/rustup",
    "--remap-path-prefix=$root=/express",
    '-Clink-arg=/PDBALTPATH:%_PDB%'
)
[Console]::Out.Write(($flags -join [char]31))

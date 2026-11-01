# Makes the portable folder and zip of OpenStage Capture Express: the program, its licence and notices, and a short guide.
# Used by PACK EXPRESS.bat. It never replaces a folder or a zip that is already there.
param([Parameter(Mandatory = $true)][string]$OutDir)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\openstage-capture-express.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "The program has not been built: $exe is missing." }
$cargo = Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw
if ($cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw 'The version could not be read from Cargo.toml.' }
$version = $Matches[1]
$arch = switch ($env:PROCESSOR_ARCHITECTURE) { 'ARM64' { 'arm64' } default { 'x64' } }
$name = "OpenStage-Capture-Express-$version-$arch"
$stage = Join-Path $OutDir $name
$zip = "$stage.zip"
if ((Test-Path -LiteralPath $stage) -or (Test-Path -LiteralPath $zip)) { throw "$name is already in $OutDir. Nothing was replaced; move it away first." }
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item -LiteralPath $exe -Destination $stage
foreach ($file in 'LICENSE', 'NOTICE', 'THIRD-PARTY.md') { Copy-Item -LiteralPath (Join-Path $root $file) -Destination $stage }
$start = @"
OpenStage Capture Express $version

What this is
  A free program that turns what you do on screen into a step-by-step guide: a picture for each step with a marker where you
  clicked, a title already written, light editing, and export to a web page, Markdown, JSON, pictures, an animated GIF, PDF, Word
  and PowerPoint. It has no account and makes no network connection.

To start
  Double-click openstage-capture-express.exe. There is nothing to install and nothing is written outside your own folders. To
  remove it, delete this folder.

If Windows warns you
  This program is not signed yet, so Windows SmartScreen may say it protected your PC. Choose "More info", then "Run anyway".
  Some antivirus programs are wary of any program that watches the mouse and keyboard; that is how capturing works, and the
  program reads no key (it reads the text of a field when you finish typing, never for a password field).

Where things are kept
  Guides:    your Documents folder, in OpenStage Express\Guides (you can choose another folder in Settings).
  Exports:   your Documents folder, in OpenStage Express\Exports (also changeable in Settings).
  Settings:  %APPDATA%\OpenStage Capture Express\settings.json
  Pictures of your screen are saved in your guides. If your Documents folder is synced to OneDrive, they are synced too, so
  choose another folder in Settings if you do not want that.

Needs
  Windows 10 version 2004 or later, or Windows 11. PDF export needs the Microsoft Edge WebView2 Runtime, which comes with
  Windows 11 and Microsoft 365.

Licence
  Apache License 2.0. See LICENSE and NOTICE. The libraries it is built from are listed in THIRD-PARTY.md and in the program
  itself (Home, About and licences).
"@
Set-Content -LiteralPath (Join-Path $stage 'START-HERE.txt') -Value ($start -replace "`r?`n", "`r`n") -Encoding Ascii
Compress-Archive -LiteralPath $stage -DestinationPath $zip
$size = [math]::Round((Get-Item -LiteralPath $zip).Length / 1MB, 1)
Write-Output "Made the folder $stage"
Write-Output "and the zip    $zip ($size MB)"

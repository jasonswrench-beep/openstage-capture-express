# SPDX-License-Identifier: Apache-2.0
# Makes the installer of OpenStage Capture Express with Inno Setup (a free program; see docs\installer.md).
# Used by MAKE EXPRESS INSTALLER.bat. It never replaces an installer that is already there.
#
# It puts the built program (with the product icon added to it), the licence files and a short guide in a folder, draws the pictures
# for the setup wizard from the product icon, and has Inno Setup's compiler (ISCC.exe) make one setup file from installer\express.iss.
#
# With -Sign, the program (after its icon is added) and then the setup file and its uninstaller are signed with Azure Artifact Signing,
# each signature timestamped, by tools\sign_windows.py (which says what it needs: three OPENSTAGE_SIGN_ settings and a sign-in).
# The SHA-256 is worked out after that, so it is the hash of the signed setup file.
param([Parameter(Mandatory = $true)][string]$OutDir, [switch]$Sign)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\release\openstage-capture-express.exe'
$icon = Join-Path $root 'assets\icon\OpenCapture_Express.ico'
$iconPngs = Join-Path $root 'assets\icon\PNG'
$script = Join-Path $root 'installer\express.iss'
if (-not (Test-Path -LiteralPath $exe)) { throw "The program has not been built: $exe is missing." }
if (-not (Test-Path -LiteralPath $icon)) { throw "The product icon is missing: $icon" }
$cargo = Get-Content -LiteralPath (Join-Path $root 'Cargo.toml') -Raw
if ($cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw 'The version could not be read from Cargo.toml.' }
$version = $Matches[1]
$arch = switch ($env:PROCESSOR_ARCHITECTURE) { 'ARM64' { 'arm64' } default { 'x64' } }

# Where Inno Setup's compiler is: the folder named in EXPRESS_ISCC, the folder this PC's setup used, or the usual places.
$places = @(
    $env:EXPRESS_ISCC,
    (Join-Path $env:USERPROFILE 'InnoSetup7\ISCC.exe'),
    (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 7\ISCC.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 7\ISCC.exe'),
    (Join-Path $env:ProgramFiles 'Inno Setup 7\ISCC.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
    (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
) | Where-Object { $_ }
$iscc = $places | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $iscc) {
    throw "Inno Setup was not found. It is free (https://jrsoftware.org/isdl.php). Install it, or set EXPRESS_ISCC to the full path of ISCC.exe, then try again."
}

$signer = Join-Path $PSScriptRoot 'sign_windows.py'
if ($Sign) {
    $python = (Get-Command python -ErrorAction SilentlyContinue).Source
    if (-not $python) { throw 'Signing needs Python (for tools\sign_windows.py), and python was not found.' }
    & $python $signer --ready
    if ($LASTEXITCODE -ne 0) { throw 'Signing cannot start: see what is missing above.' }
}

$name = "OpenStage-Capture-Express-$version-$arch-setup"
$setup = Join-Path $OutDir "$name.exe"
if (Test-Path -LiteralPath $setup) { throw "$name.exe is already in $OutDir. Nothing was replaced; move it away first." }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$work = Join-Path ([IO.Path]::GetTempPath()) ("express-installer-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
try {
    # The folder the installer takes its files from. The icon goes into this copy of the program, not into the one in target\.
    $stage = Join-Path $work 'stage'
    New-Item -ItemType Directory -Path $stage | Out-Null
    Copy-Item -LiteralPath $exe -Destination $stage
    # The program must not name this computer's user folder or the folder it was built in (the security review of 2026-11-13).
    & (Join-Path $PSScriptRoot 'check-exe-paths.ps1') -Exe (Join-Path $stage 'openstage-capture-express.exe') -Root $root
    & (Join-Path $PSScriptRoot 'set-exe-icon.ps1') -Exe (Join-Path $stage 'openstage-capture-express.exe') -Icon $icon
    if ($Sign) {
        # Signed after the icon is added (changing the program afterwards would break its signature).
        & $python $signer --description 'OpenStage Capture Express' (Join-Path $stage 'openstage-capture-express.exe')
        if ($LASTEXITCODE -ne 0) { throw 'The program could not be signed.' }
        $warning = @"
  This program is signed by Jason Wrench. Because it is new, Windows SmartScreen may still say it protected your PC at first;
  if it does, choose "More info", then "Run anyway".
"@
    }
    else {
        $warning = @"
  This program is not signed yet, so Windows SmartScreen may say it protected your PC. Choose "More info", then "Run anyway".
"@
    }
    foreach ($file in 'LICENSE', 'NOTICE', 'THIRD-PARTY.md') { Copy-Item -LiteralPath (Join-Path $root $file) -Destination $stage }
    $start = @"
OpenStage Capture Express $version

What this is
  A free program that turns what you do on screen into a step-by-step guide: a picture for each step with a marker where you
  clicked, a title already written, light editing, and export to a web page, Markdown, JSON, pictures, an animated GIF, PDF, Word
  and PowerPoint. It has no account and makes no network connection.

To start
  Open OpenStage Capture Express from the Start menu (or the desktop shortcut, if you chose one).

If Windows warns you
$warning
  Some antivirus programs are wary of any program that watches the mouse and keyboard; that is how capturing works, and the
  program reads no key (it reads the text of a field when you finish typing, never for a password field).

Where things are kept
  Guides:    your Documents folder, in OpenStage Express\Guides (you can choose another folder in Settings).
  Exports:   your Documents folder, in OpenStage Express\Exports (also changeable in Settings).
  Settings:  %APPDATA%\OpenStage Capture Express\settings.json
  Pictures of your screen are saved in your guides. If your Documents folder is synced to OneDrive, they are synced too, so
  choose another folder in Settings if you do not want that.

Removing it
  Use Settings, Apps, Installed apps (or Add or remove programs) and choose OpenStage Capture Express. Your guides, exports and
  settings are not deleted: they are yours, and stay where they are.

Needs
  Windows 10 version 2004 or later, or Windows 11. PDF export needs the Microsoft Edge WebView2 Runtime, which comes with
  Windows 11 and Microsoft 365.

Licence
  Apache License 2.0. See LICENSE and NOTICE. The libraries it is built from are listed in THIRD-PARTY.md and in the program
  itself (Home, About and licences).
"@
    Set-Content -LiteralPath (Join-Path $stage 'START-HERE.txt') -Value ($start -replace "`r?`n", "`r`n") -Encoding Ascii

    # The pictures for the setup wizard, drawn from the product icon: a tall one for the first and last pages (in three sizes, for
    # screens set to 100, 150 and 200 percent) and a small one for the corner of the other pages.
    Add-Type -AssemblyName System.Drawing
    $wizard = Join-Path $work 'wizard'
    New-Item -ItemType Directory -Path $wizard | Out-Null
    function New-TallPicture([int]$width, [int]$height, [string]$iconPicture, [string]$path) {
        $bitmap = New-Object System.Drawing.Bitmap $width, $height
        $g = [System.Drawing.Graphics]::FromImage($bitmap)
        $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
        $area = New-Object System.Drawing.Rectangle 0, 0, $width, $height
        $navy = [System.Drawing.ColorTranslator]::FromHtml('#102037')
        $deep = [System.Drawing.ColorTranslator]::FromHtml('#0B172A')
        $fill = New-Object System.Drawing.Drawing2D.LinearGradientBrush $area, $navy, $deep, 90
        $g.FillRectangle($fill, $area)
        $picture = [System.Drawing.Image]::FromFile($iconPicture)
        $side = [int]($width * 0.7)
        $g.DrawImage($picture, [int](($width - $side) / 2), [int]($height * 0.1), $side, $side)
        $centre = New-Object System.Drawing.StringFormat
        $centre.Alignment = [System.Drawing.StringAlignment]::Center
        $big = New-Object System.Drawing.Font 'Segoe UI Semibold', ([single]($width * 0.115)), ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
        $small = New-Object System.Drawing.Font 'Segoe UI', ([single]($width * 0.085)), ([System.Drawing.FontStyle]::Regular), ([System.Drawing.GraphicsUnit]::Pixel)
        $white = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::White)
        $indigo = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml('#9CA5FF'))
        $top = [int]($height * 0.1) + $side + [int]($height * 0.04)
        $g.DrawString('OpenStage', $big, $white, (New-Object System.Drawing.RectangleF 0, $top, $width, ($width * 0.2)), $centre)
        $g.DrawString('Capture Express', $small, $indigo, (New-Object System.Drawing.RectangleF 0, ($top + $width * 0.15), $width, ($width * 0.2)), $centre)
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bitmap.Dispose(); $picture.Dispose()
    }
    function New-SmallPicture([int]$side, [string]$iconPicture, [string]$path) {
        $bitmap = New-Object System.Drawing.Bitmap $side, $side, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g = [System.Drawing.Graphics]::FromImage($bitmap)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $picture = [System.Drawing.Image]::FromFile($iconPicture)
        $g.DrawImage($picture, 0, 0, $side, $side)
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bitmap.Dispose(); $picture.Dispose()
    }
    $source = Join-Path $iconPngs 'OpenCapture_Express_512.png'
    foreach ($size in @(@(164, 314), @(246, 471), @(328, 628))) { New-TallPicture $size[0] $size[1] $source (Join-Path $wizard "tall-$($size[0]).png") }
    foreach ($side in 55, 83, 110) { New-SmallPicture $side $source (Join-Path $wizard "small-$side.png") }

    $compile = @("/DAppVersion=$version", "/DStageDir=$stage", "/DWizardDir=$wizard", "/O$OutDir", "/F$name")
    if ($Sign) {
        # Inno Setup puts the quoted file name in place of $f and a quote mark in place of $q.
        $compile += '/DSignTool=artifact'
        $compile += ('/Sartifact=$q' + $python + '$q $q' + $signer + '$q --description $qOpenStage Capture Express$q $f')
    }
    & $iscc @compile $script
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup's compiler stopped with error $LASTEXITCODE." }
    if (-not (Test-Path -LiteralPath $setup)) { throw "The compiler finished but $setup was not made." }
    $hash = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash
    Set-Content -LiteralPath "$setup.sha256.txt" -Value "$hash *$name.exe" -Encoding Ascii
    $size = [math]::Round((Get-Item -LiteralPath $setup).Length / 1MB, 1)
    Write-Output "Made the installer $setup ($size MB)"
    Write-Output "SHA-256 $hash"
}
finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}

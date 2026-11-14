# SPDX-License-Identifier: Apache-2.0
# Stops a release when the program names this computer's user folder or the folder it was built in, as text or as UTF-16, with
# either kind of slash, in any case (the security review of 2026-11-13). Used by make-installer.ps1 and pack.ps1 on the program they
# pack. A build made by MAKE EXPRESS INSTALLER.bat or PACK EXPRESS.bat maps those folders out (tools\release-flags.ps1).
param([Parameter(Mandatory = $true)][string]$Exe, [Parameter(Mandatory = $true)][string]$Root)
$ErrorActionPreference = 'Stop'
$bytes = [IO.File]::ReadAllBytes($Exe)
$latin = [Text.Encoding]::GetEncoding(28591).GetString($bytes).ToLowerInvariant()
$wide = [Text.Encoding]::Unicode.GetString($bytes).ToLowerInvariant()
$wideOdd = if ($bytes.Length -gt 1) { [Text.Encoding]::Unicode.GetString($bytes, 1, $bytes.Length - 1).ToLowerInvariant() } else { '' }
$folders = @([Environment]::GetFolderPath('UserProfile'), (Resolve-Path -LiteralPath $Root).Path) | Where-Object { $_ }
$found = @()
foreach ($folder in $folders) {
    foreach ($form in @($folder, $folder.Replace('\', '/'))) {
        $needle = $form.TrimEnd('\', '/').ToLowerInvariant()
        if ($latin.Contains($needle) -or $wide.Contains($needle) -or $wideOdd.Contains($needle)) { $found += $form }
    }
}
if ($found.Count -gt 0) {
    throw ("The program names this computer's folders (" + (($found | Select-Object -Unique) -join '; ') + "). Build it with MAKE EXPRESS INSTALLER.bat or PACK EXPRESS.bat, which map them out, and try again.")
}
Write-Output 'The program names neither this computer''s user folder nor the folder it was built in.'

# SPDX-License-Identifier: Apache-2.0
# Puts the product icon (assets\icon\OpenCapture_Express.ico) into a program file, so that Explorer, the taskbar and the Start menu
# show it. It uses Windows' own resource calls (BeginUpdateResource and UpdateResource), so nothing has to be downloaded.
# Used by make-installer.ps1 and pack.ps1 on the copy they put in the folder to be shipped; the built program in target\ is not touched.
# Do it before signing a program: changing a signed file breaks its signature.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Icon
)
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class ExpressResources {
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    public static extern IntPtr BeginUpdateResource(string fileName, bool deleteExistingResources);
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool UpdateResource(IntPtr update, IntPtr type, IntPtr name, ushort language, byte[] data, uint size);
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool EndUpdateResource(IntPtr update, bool discard);
}
"@

$RT_ICON = [IntPtr]3
$RT_GROUP_ICON = [IntPtr]14

$bytes = [IO.File]::ReadAllBytes($Icon)
if ($bytes.Length -lt 6 -or $bytes[0] -ne 0 -or $bytes[1] -ne 0 -or $bytes[2] -ne 1 -or $bytes[3] -ne 0) {
    throw "$Icon is not a Windows icon file."
}
$count = [BitConverter]::ToUInt16($bytes, 4)
if ($count -lt 1) { throw "$Icon holds no pictures." }

# The group lists each picture (without where it sits in the file); each picture is stored on its own as a numbered icon.
$group = New-Object System.Collections.Generic.List[byte]
$group.AddRange([byte[]](0, 0, 1, 0))
$group.AddRange([BitConverter]::GetBytes([uint16]$count))
$images = @()
for ($i = 0; $i -lt $count; $i++) {
    $entry = 6 + 16 * $i
    $size = [BitConverter]::ToUInt32($bytes, $entry + 8)
    $offset = [BitConverter]::ToUInt32($bytes, $entry + 12)
    if ($offset + $size -gt $bytes.Length) { throw "$Icon is damaged (picture $($i + 1) runs past the end of the file)." }
    $images += , @{ Id = $i + 1; Data = $bytes[$offset..($offset + $size - 1)] }
    $group.AddRange([byte[]]$bytes[$entry..($entry + 11)])
    $group.AddRange([BitConverter]::GetBytes([uint16]($i + 1)))
}

$handle = [ExpressResources]::BeginUpdateResource($Exe, $false)
if ($handle -eq [IntPtr]::Zero) { throw "The program file $Exe could not be opened to add the icon (is it running?)." }
$ok = $false
try {
    foreach ($image in $images) {
        $data = [byte[]]$image.Data
        if (-not [ExpressResources]::UpdateResource($handle, $RT_ICON, [IntPtr]$image.Id, 0, $data, [uint32]$data.Length)) {
            throw "Adding icon picture $($image.Id) failed (Windows error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))."
        }
    }
    $groupData = $group.ToArray()
    if (-not [ExpressResources]::UpdateResource($handle, $RT_GROUP_ICON, [IntPtr]1, 0, $groupData, [uint32]$groupData.Length)) {
        throw "Adding the icon group failed (Windows error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))."
    }
    $ok = $true
}
finally {
    if (-not [ExpressResources]::EndUpdateResource($handle, -not $ok) -and $ok) {
        throw "Saving the icon into $Exe failed (Windows error $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))."
    }
}
Write-Output "Put the icon ($count pictures) into $Exe"

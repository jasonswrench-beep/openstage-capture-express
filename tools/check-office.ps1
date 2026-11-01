# SPDX-License-Identifier: Apache-2.0
# Opens the Word and PowerPoint files that EXPORT DEMO.bat made, in the real Word and the real PowerPoint, and checks what they
# find: the headings and the contents list in Word; the slide titles, the pictures' alt text and the speaker notes in PowerPoint.
# It needs Microsoft Word and PowerPoint on this computer. Run it after EXPORT DEMO.bat, from the Express folder:
#
#     powershell -NoProfile -ExecutionPolicy Bypass -File tools\check-office.ps1
#
# Each program is run in a child process with a time limit, hidden where it allows it, and the files are opened read only. If
# Word or PowerPoint was already running, the script leaves it alone. It only ever stops a process that it started itself.
# Exit code: 0 when every check passed, 1 when one failed, 2 when nothing could be checked.
param(
    [string]$Folder = '',
    [ValidateSet('', 'Word', 'PowerPoint')][string]$Child = '',
    [string]$File = ''
)

$ErrorActionPreference = 'Stop'

function Check([bool]$ok, [string]$what) {
    if ($ok) { Write-Output "PASS  $what" } else { Write-Output "FAIL  $what" }
}

if ($Child -eq 'Word') {
    $before = @(Get-Process WINWORD -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $word = New-Object -ComObject Word.Application
    $word.Visible = $false
    $word.DisplayAlerts = 0
    try {
        $doc = $word.Documents.Open([ref]$File, [ref]$false, [ref]$true, [ref]$false)
        Check $true 'Word opened the file without asking to repair it'
        Check ($doc.CompatibilityMode -ge 15) 'it is not in compatibility mode'
        $styles = @{}
        foreach ($p in $doc.Paragraphs) { $styles[$p.Style.NameLocal] = 1 + [int]$styles[$p.Style.NameLocal] }
        Check ($styles['Title'] -eq 1) 'one Title paragraph'
        Check ($styles['Heading 1'] -ge 1) 'at least one Heading 1 (a step)'
        $levels = @($doc.Paragraphs | Where-Object { $_.Style.NameLocal -like 'Heading*' } | ForEach-Object { $_.OutlineLevel })
        Check ($levels.Count -ge 1 -and ($levels | Where-Object { $_ -lt 1 -or $_ -gt 3 }).Count -eq 0) 'headings have outline levels 1 to 3'
        Check ($doc.TablesOfContents.Count -eq 1) 'Word sees one table of contents'
        $pictures = @($doc.InlineShapes)
        Check ($pictures.Count -ge 1) ('pictures: ' + $pictures.Count)
        Check (@($pictures | Where-Object { [string]::IsNullOrWhiteSpace($_.AlternativeText) }).Count -eq 0) 'every picture has alt text'
        $a = (@($doc.TablesOfContents.Item(1).Range.Text -split "`r") | Where-Object { $_.Trim() }) -join '|'
        $doc.Close([ref]0)
        $doc = $word.Documents.Open([ref]$File, [ref]$false, [ref]$false, [ref]$false)
        $doc.TablesOfContents.Item(1).Update()
        $b = (@($doc.TablesOfContents.Item(1).Range.Text -split "`r") | Where-Object { $_.Trim() }) -join '|'
        Check ($a -eq $b) 'Word updating the contents list itself gives the same entries'
        $doc.Close([ref]0)
    } finally {
        if ($before.Count -eq 0) { $word.Quit([ref]0) }
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($word)
    }
    return
}

if ($Child -eq 'PowerPoint') {
    $before = @(Get-Process POWERPNT -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $app = New-Object -ComObject PowerPoint.Application
    try {
        $pres = $app.Presentations.Open($File, -1, 0, 0)
        Check $true 'PowerPoint opened the file without asking to repair it'
        Check ($pres.Slides.Count -ge 1) ('slides: ' + $pres.Slides.Count)
        Check ([math]::Abs($pres.PageSetup.SlideWidth / $pres.PageSetup.SlideHeight - 16 / 9) -lt 0.01) 'the slides are 16 to 9'
        $titles = @(); $noTitle = 0; $noAlt = 0; $notes = 0
        foreach ($s in $pres.Slides) {
            if ($s.Shapes.HasTitle -ne 0 -and $s.Shapes.Title.TextFrame.TextRange.Text.Trim()) { $titles += $s.Shapes.Title.TextFrame.TextRange.Text.Trim() } else { $noTitle++ }
            foreach ($sh in $s.Shapes) { if ($sh.Type -eq 13 -and [string]::IsNullOrWhiteSpace($sh.AlternativeText)) { $noAlt++ } }
            foreach ($sh in $s.NotesPage.Shapes) {
                try { if ($sh.PlaceholderFormat.Type -eq 2 -and $sh.TextFrame.TextRange.Text.Trim()) { $notes++ } } catch { }
            }
        }
        Check ($noTitle -eq 0) 'every slide has a title'
        Check (($titles | Select-Object -Unique).Count -eq $titles.Count) 'no two slides share a title'
        Check ($noAlt -eq 0) 'every picture has alt text'
        Check ($notes -ge 1) ('slides with speaker notes: ' + $notes)
        $pres.Close()
    } finally {
        if ($before.Count -eq 0) { $app.Quit() }
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($app)
    }
    return
}

# The parent: find the files, run each child with a time limit, and add the results up.
$root = Split-Path -Parent $PSScriptRoot
if (-not $Folder) {
    $latest = Get-ChildItem (Join-Path $root 'export-demo-output') -Directory -ErrorAction SilentlyContinue | Sort-Object Name | Select-Object -Last 1
    if (-not $latest) { Write-Output 'There is no export-demo-output folder yet. Run EXPORT DEMO.bat first.'; exit 2 }
    $Folder = $latest.FullName
}
$jobs = @(
    @{ Name = 'Word'; Process = 'WINWORD'; Pattern = 'word\*.docx' },
    @{ Name = 'PowerPoint'; Process = 'POWERPNT'; Pattern = 'powerpoint\*.pptx' }
)
$failed = 0; $checked = 0
foreach ($job in $jobs) {
    $target = Get-ChildItem (Join-Path $Folder $job.Pattern) -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $target) { Write-Output ("SKIP  no " + $job.Name + " file in " + $Folder); continue }
    if (@(Get-Process $job.Process -ErrorAction SilentlyContinue).Count -gt 0) {
        Write-Output ("SKIP  " + $job.Name + " is already open on this computer. Close it and run this again, so this check cannot disturb your work.")
        continue
    }
    $registered = [bool](Get-ItemProperty ("Registry::HKEY_CLASSES_ROOT\" + $job.Name + ".Application") -ErrorAction SilentlyContinue)
    if (-not $registered) { Write-Output ("SKIP  " + $job.Name + " is not installed on this computer"); continue }
    Write-Output ("--- " + $job.Name + ": " + $target.Name)
    $before = @(Get-Process $job.Process -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $out = Join-Path $env:TEMP ("express-check-" + $job.Name + ".txt")
    $err = Join-Path $env:TEMP ("express-check-" + $job.Name + "-err.txt")
    $argList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ('"{0}"' -f $PSCommandPath), '-Child', $job.Name, '-File', ('"{0}"' -f $target.FullName))
    $p = Start-Process powershell -ArgumentList $argList -PassThru -WindowStyle Hidden -WorkingDirectory $env:TEMP -RedirectStandardOutput $out -RedirectStandardError $err
    if (-not $p.WaitForExit(150000)) { Write-Output ("FAIL  " + $job.Name + " did not answer in 150 seconds"); $failed++; Stop-Process -Id $p.Id -Force }
    Start-Sleep -Seconds 6
    Get-Process $job.Process -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | ForEach-Object { Stop-Process -Id $_.Id -Force; Write-Output ("      stopped a " + $job.Process + " process that this script had started (pid " + $_.Id + ")") }
    $lines = @(Get-Content $out -ErrorAction SilentlyContinue)
    $lines | ForEach-Object { Write-Output $_ }
    $errors = @(Get-Content $err -ErrorAction SilentlyContinue)
    if ($errors.Count -gt 0) { Write-Output ("FAIL  " + $job.Name + " reported an error: " + ($errors | Select-Object -First 2) -join ' '); $failed++ }
    $failed += @($lines | Where-Object { $_ -like 'FAIL*' }).Count
    $checked += @($lines | Where-Object { $_ -like 'PASS*' -or $_ -like 'FAIL*' }).Count
}
Write-Output ''
if ($checked -eq 0) { Write-Output 'Nothing could be checked.'; exit 2 }
if ($failed -eq 0) { Write-Output "All $checked Office checks passed."; exit 0 }
Write-Output "$failed Office check(s) failed."
exit 1

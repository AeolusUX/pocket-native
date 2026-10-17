# update.ps1 [-Out folder] [-Check]
#
# update.sh for Windows: brings this kit up to the latest release of Pocket
# Native, then builds every game already in the output folder, build\ by
# default, again with it. The kit's old files go in .old. -Check only says
# whether a new version is out.
param(
    [string]$Out = "",
    [switch]$Check
)

$repo = Split-Path -Parent $PSScriptRoot
$releases = if ($env:POCKET_NATIVE_RELEASES) { $env:POCKET_NATIVE_RELEASES } else { "https://api.github.com/repos/whovian64/pocket-native/releases/latest" }
$headers = @{ "User-Agent" = "pocket-native" }
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$current = [regex]::Match((Get-Content -Raw -LiteralPath (Join-Path $repo "Cargo.toml")), '(?m)^version = "(.+)"').Groups[1].Value
try {
    $release = Invoke-RestMethod -Uri $releases -Headers $headers -TimeoutSec 10
    $latest = "$($release.tag_name)".TrimStart("v")
    $newer = [version]$latest -gt [version]$current
} catch {
    if ($Check) { exit 0 }
    Write-Host "could not reach GitHub to look for a new version: $($_.Exception.Message)"
    exit 1
}

if ($Check) {
    if ($newer) { Write-Host "== Pocket Native $latest is out, you have $current. Update Pocket Native.bat updates this kit and builds your games again." }
    exit 0
}
if (-not $newer) {
    Write-Host "this kit is up to date, Pocket Native $current"
    exit 0
}

if (Test-Path -LiteralPath (Join-Path $repo ".git")) {
    Write-Host "== updating this git clone to Pocket Native $latest"
    git -C $repo pull --ff-only
    if ($LASTEXITCODE -ne 0) { exit 1 }
} else {
    Write-Host "== downloading Pocket Native $latest"
    $work = Join-Path $repo ".update\kit"
    Remove-Item -Recurse -Force -LiteralPath $work -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    $zip = Join-Path $work "kit.zip"
    $ProgressPreference = "SilentlyContinue"
    Invoke-WebRequest -Uri $release.zipball_url -Headers $headers -OutFile $zip -UseBasicParsing
    Expand-Archive -LiteralPath $zip -DestinationPath $work
    $new = Get-ChildItem -LiteralPath $work -Directory | Select-Object -First 1
    if (-not $new -or -not (Test-Path -LiteralPath (Join-Path $new.FullName "Cargo.toml"))) {
        Write-Host "the download is not a Pocket Native kit"
        exit 1
    }
    Write-Host "== replacing this kit's files, the old ones go in .old"
    $old = Join-Path $repo ".old"
    Remove-Item -Recurse -Force -LiteralPath $old -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $old | Out-Null
    foreach ($item in Get-ChildItem -Force -LiteralPath $new.FullName) {
        $here = Join-Path $repo $item.Name
        if (Test-Path -LiteralPath $here) { Move-Item -LiteralPath $here -Destination $old }
        Move-Item -LiteralPath $item.FullName -Destination $repo
    }
    Remove-Item -Recurse -Force -LiteralPath $work
}

# each game builds from the copy the last build left, moved out of the way
# while the build copies it back, and put back if it did not, also when an
# update stopped halfway left it there
if (-not $Out) { $Out = Join-Path $repo "build" }
$games = Join-Path $Out "Pocket Native\pocket-native\games"
$staging = Join-Path $repo ".update\games"
function Restore {
    foreach ($copy in @(Get-ChildItem -LiteralPath $staging -Filter *.3ds -ErrorAction SilentlyContinue)) {
        $game = Join-Path $games "$($copy.BaseName)\game.3ds"
        $built = Get-Item -LiteralPath $game -ErrorAction SilentlyContinue
        if ($built -and $built.Length -eq $copy.Length) {
            Remove-Item -LiteralPath $copy.FullName
        } else {
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $game) | Out-Null
            Move-Item -Force -LiteralPath $copy.FullName -Destination $game
        }
    }
}
Restore
$built = @(Get-ChildItem -Path (Join-Path $games "*\game.3ds") -ErrorAction SilentlyContinue)
if ($built.Count -eq 0) {
    Write-Host "== no games in $Out to build again, build them as before"
    exit 0
}
New-Item -ItemType Directory -Force -Path $staging | Out-Null
foreach ($game in $built) {
    Move-Item -LiteralPath $game.FullName -Destination (Join-Path $staging "$($game.Directory.Name).3ds")
}
Write-Host "== building $($built.Count) games again"
$status = 1
try {
    $roms = @(Get-ChildItem -LiteralPath $staging -Filter *.3ds | ForEach-Object { $_.FullName })
    & (Join-Path $repo "scripts\build.ps1") @roms -Out $Out
    $status = $LASTEXITCODE
} finally {
    Restore
    if (-not (Get-ChildItem -LiteralPath $staging)) { Remove-Item -Recurse -Force -LiteralPath (Join-Path $repo ".update") }
}
if ($status -eq 0) { Write-Host "== done: copy `"Pocket Native`" onto the card again, over the old one" }
exit $status

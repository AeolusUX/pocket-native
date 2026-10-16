# build.ps1 <rom> [-Keys aes_keys.txt] [-Out folder]
#
# build.sh for Windows: builds Pocket Native and a game for the handheld from
# your own ROM, a decrypted .3ds/.cci or a .cia with your console's
# aes_keys.txt, inside a container (Docker Desktop or Podman). Leaves
# "Pocket Native" in the output folder, build\ by default, to copy into
# roms\ports on the card.
param(
    [Parameter(Mandatory = $true, Position = 0)][string]$Rom,
    [string]$Keys = "",
    [string]$Out = ""
)

$repo = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path -LiteralPath $Rom -PathType Leaf)) {
    Write-Host "usage: scripts\build.ps1 <rom> [-Keys aes_keys.txt] [-Out folder]"
    exit 2
}
$Rom = (Resolve-Path -LiteralPath $Rom).Path
if ($Keys) { $Keys = (Resolve-Path -LiteralPath $Keys -ErrorAction Stop).Path }
if (-not $Out) { $Out = Join-Path $repo "build" }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path -LiteralPath $Out).Path

$engine = @("docker", "podman") | Where-Object { Get-Command $_ -ErrorAction SilentlyContinue } | Select-Object -First 1
if (-not $engine) {
    Write-Host "the build needs Docker Desktop or Podman"
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) { exit 1 }
    $answer = Read-Host "install Docker Desktop with winget now? (y/n)"
    if ($answer -ne "y") { exit 1 }
    winget install -e --id Docker.DockerDesktop
    Write-Host "start Docker Desktop, wait until it is running, then run this script again"
    exit 1
}
& $engine info 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "$engine is not running, start it and run this script again"
    exit 1
}
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $repo "scripts\container\Containerfile")).Hash.ToLower().Substring(0, 12)
$image = "pocket-native-build:$hash"
& $engine image inspect $image 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "== making the build container, once"
    & $engine build -f (Join-Path $repo "scripts\container\Containerfile") -t $image (Join-Path $repo "scripts\container")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# the scratch space stays in a volume on the container's own disk, compiling
# into a Windows folder is many times slower
$mounts = @("-v", "${repo}:/src:ro", "-v", "pocket-native-work:/work", "-v", "${Out}:/out", "-v", "$(Split-Path -Parent $Rom):/rom:ro")
$arguments = @("/rom/$(Split-Path -Leaf $Rom)")
if ($Keys) {
    $mounts += @("-v", "$(Split-Path -Parent $Keys):/keys:ro")
    $arguments += "/keys/$(Split-Path -Leaf $Keys)"
}
& $engine run --rm @mounts -e CARGO_HOME=/work/cargo -e CARGO_TARGET_DIR=/work/target -e HOME=/work `
    $image bash /src/scripts/container/inside.sh @arguments
exit $LASTEXITCODE

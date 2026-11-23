# Pictures of Varrow Bridgeworks (each base, each lane, an overview) and a Steam Deck preset timing
# run with 8 players, from a copy of the game (so a running game is never touched) and a copy of
# the save folder (so test matches never change your progress).
#   powershell -File tools\capture_varrow.ps1 [-Out shots\varrow] [-Bench] [-Only overview,quay]
# Run from app/ after `cargo build --release`.
param([string]$Out = "shots\varrow", [switch]$Bench, [string[]]$Only = @())

$Only = @($Only | ForEach-Object { $_ -split "," } | Where-Object { $_ })
$dir = Join-Path $env:TEMP "varrow_check"
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item "target\release\duel_arena.exe" (Join-Path $dir "duel_arena.exe") -Force
$exe = Join-Path $dir "duel_arena.exe"
$data = Join-Path $dir "data"
if (Test-Path $data) { Remove-Item $data -Recurse -Force }
New-Item -ItemType Directory -Force $data | Out-Null
$real = Join-Path $env:APPDATA "DuelRush"
foreach ($f in "settings.txt", "career.txt") {
    if (Test-Path (Join-Path $real $f)) { Copy-Item (Join-Path $real $f) $data }
}
New-Item -ItemType Directory -Force $Out | Out-Null
$outAbs = (Resolve-Path $Out).Path
$keys = "DUEL_MAP", "DUEL_MODE", "DUEL_AUTOPLAY", "DUEL_BOTS", "DUEL_VIEW", "DUEL_SHOT_FRAME", "DUEL_QUALITY", "DUEL_WINDOW", "DUEL_BENCH", "DUEL_DATA_DIR"

function Clear-Switches { foreach ($k in $keys) { Remove-Item "Env:$k" -ErrorAction SilentlyContinue } }

function Invoke-Shot([string]$name, [string]$view, [string]$mode = "5") {
    if ($Only.Count -gt 0 -and -not ($Only -contains $name)) { return }
    Clear-Switches
    $env:DUEL_DATA_DIR = $data
    $env:DUEL_MAP = "3"; $env:DUEL_MODE = $mode; $env:DUEL_AUTOPLAY = "1"; $env:DUEL_BOTS = "7"
    $env:DUEL_SHOT_FRAME = "420"; $env:DUEL_VIEW = $view
    $path = Join-Path $outAbs "$name.png"
    if (Test-Path $path) { Remove-Item $path }
    $p = Start-Process -FilePath $exe -ArgumentList "--screenshot", "`"$path`"" -PassThru -WindowStyle Minimized
    if (-not $p.WaitForExit(120000)) { $p.Kill() }
    Clear-Switches
}

# camera: x, y, z, yaw (0 looks north, to blue's base), pitch, field of view
Invoke-Shot "overview" "0,64,-44,0,-0.98,72"
Invoke-Shot "overview_blue" "0,64,44,3.1416,-0.98,72"
Invoke-Shot "base_red" "-2,7.5,-12,3.1416,-0.32,78"
Invoke-Shot "base_blue" "2,7.5,12,0,-0.32,78"
Invoke-Shot "lane_riverside_quay" "-23.5,1.7,-16,0,-0.03,72"
Invoke-Shot "lane_bridge_plaza" "4,4.5,-15.5,-0.2,-0.12,78"
Invoke-Shot "lane_workshops" "21.5,1.7,-13,0.1,-0.02,80"
Invoke-Shot "level3_rooftops" "-14.5,9.5,-9,0.6,-0.25,80"
Invoke-Shot "river" "-14,-1.2,0,1.5708,0.0,80"
Invoke-Shot "flag_room_red" "4,1.7,-23.5,3.6,-0.05,82"

if ($Bench) {
    Clear-Switches
    $env:DUEL_DATA_DIR = $data
    $env:DUEL_MAP = "3"; $env:DUEL_MODE = "1"; $env:DUEL_AUTOPLAY = "1"; $env:DUEL_BOTS = "7"; $env:DUEL_QUALITY = "4"; $env:DUEL_WINDOW = "1280x800"
    $txt = Join-Path $outAbs "bench_steamdeck.txt"
    $env:DUEL_BENCH = "60,$txt"
    $p = Start-Process -FilePath $exe -PassThru -WindowStyle Minimized
    if (-not $p.WaitForExit(150000)) { $p.Kill() }
    Clear-Switches
    Get-Content $txt
}
Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue

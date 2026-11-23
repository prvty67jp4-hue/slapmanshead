# Before / after pictures of every map's lanes and of every weapon in play, from a copy of the game
# (a running game is never touched) and a copy of the save folder (nothing changes your progress).
# Each picture's game closes itself once it's taken; any copy still open at the end is closed.
#   powershell -File tools\capture_compare.ps1 -Phase before|after [-Only arena,weapons] [-Bench]
# Run from app/ after `cargo build --release`.
param([string]$Phase = "after", [string[]]$Only = @(), [switch]$Bench)

$Only = @($Only | ForEach-Object { $_ -split "," } | Where-Object { $_ })
$dir = Join-Path $env:TEMP "compare_check"
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item "target\release\duel_arena.exe" (Join-Path $dir "duel_arena.exe") -Force
$exe = Join-Path $dir "duel_arena.exe"
$data = Join-Path $dir "data"
New-Item -ItemType Directory -Force $data | Out-Null
Get-ChildItem $data | Remove-Item -Recurse -Force
$real = Join-Path $env:APPDATA "DuelRush"
foreach ($f in "settings.txt", "career.txt") {
    if (Test-Path (Join-Path $real $f)) { Copy-Item (Join-Path $real $f) $data }
}
$out = Join-Path "shots\compare" $Phase
New-Item -ItemType Directory -Force $out | Out-Null
$outAbs = (Resolve-Path $out).Path
$keys = "DUEL_MAP", "DUEL_MODE", "DUEL_AUTOPLAY", "DUEL_BOTS", "DUEL_VIEW", "DUEL_SHOT_FRAME", "DUEL_HOLD", "DUEL_QUALITY", "DUEL_WINDOW", "DUEL_BENCH", "DUEL_DATA_DIR", "DUEL_HUD_SHOT"
function Clear-Switches { foreach ($k in $keys) { Remove-Item "Env:$k" -ErrorAction SilentlyContinue } }
$started = @()

function Invoke-Shot([string]$group, [string]$name, [hashtable]$set) {
    if ($Only.Count -gt 0 -and -not ($Only -contains $group)) { return }
    Clear-Switches
    $env:DUEL_DATA_DIR = $data
    $env:DUEL_AUTOPLAY = "1"
    foreach ($k in $set.Keys) { Set-Item "Env:$k" $set[$k] }
    $path = Join-Path $outAbs "$name.png"
    if (Test-Path $path) { Remove-Item $path }
    $p = Start-Process -FilePath $exe -ArgumentList "--screenshot", "`"$path`"" -PassThru -WindowStyle Minimized
    $script:started += $p
    if (-not $p.WaitForExit(120000)) { $p.Kill() }
    Clear-Switches
}

# map lanes: camera x, y, z, yaw (0 looks north), pitch, field of view
$maps = [ordered]@{
    "arena"         = @(0, @{ overview = "0,40,-30,0,-0.95,70"; lane_west = "-10.5,1.7,-14.5,0.1,-0.03,75"; lane_mid = "0,1.7,-16,0,-0.05,75"; lane_east = "12,1.7,-15,-0.15,-0.03,75" })
    "courtyard"     = @(1, @{ overview = "0,50,-38,0,-0.95,70"; lane_west = "-14,1.7,-20,0.1,-0.03,75"; lane_mid = "-8,1.7,-20,0.15,-0.03,75"; lane_east = "14,1.7,-20,-0.1,-0.03,75" })
    "closequarters" = @(2, @{ overview = "0,34,-24,0,-0.98,70"; lane_west = "-9,1.7,12,2.99,-0.03,78"; lane_mid = "0,1.7,-12.5,0,-0.05,78"; lane_east = "9,1.7,-12,-0.15,-0.03,78" })
    "bridgeworks"   = @(3, @{ overview = "0,64,-44,0,-0.98,72"; base_red = "-2,7.5,-12,3.1416,-0.32,78"; base_blue = "2,7.5,12,0,-0.32,78"; lane_riverside = "-23.5,1.7,-16,0,-0.03,72"; lane_bridge = "4,4.5,-15.5,-0.2,-0.12,78"; lane_workshops = "21.5,1.7,-13,0.1,-0.02,80"; river = "-14,-1.2,0,1.5708,0.0,80" })
}
foreach ($m in $maps.Keys) {
    $id = $maps[$m][0]
    $views = $maps[$m][1]
    foreach ($v in $views.Keys) {
        $mode = if ($id -eq 3) { "1" } else { "0" }
        Invoke-Shot $m "map_${m}_$v" @{ DUEL_MAP = "$id"; DUEL_MODE = $mode; DUEL_BOTS = "3"; DUEL_SHOT_FRAME = "240"; DUEL_VIEW = $views[$v] }
    }
}

# every weapon in play: a duel on the Arena, everyone holding it, the picture well into the fight
$weapons = @("dagger", "katana", "greatsword", "scattergun", "revolver", "smg", "rifle", "rocket", "hand_cannon", "sniper", "shuriken")
for ($w = 0; $w -lt $weapons.Count; $w++) {
    Invoke-Shot "weapons" ("weapon_{0:D2}_{1}" -f $w, $weapons[$w]) @{ DUEL_MAP = "0"; DUEL_MODE = "0"; DUEL_BOTS = "1"; DUEL_HOLD = "$w"; DUEL_SHOT_FRAME = "520" }
}

if ($Bench) {
    # Steam Deck preset, 8 fighters, team rounds on the team map
    Clear-Switches
    $env:DUEL_DATA_DIR = $data
    $env:DUEL_AUTOPLAY = "1"; $env:DUEL_MAP = "3"; $env:DUEL_MODE = "1"; $env:DUEL_BOTS = "7"; $env:DUEL_QUALITY = "4"; $env:DUEL_WINDOW = "1280x800"
    $txt = Join-Path $outAbs "bench_steamdeck.txt"
    $env:DUEL_BENCH = "40,$txt"
    $p = Start-Process -FilePath $exe -PassThru -WindowStyle Minimized
    $started += $p
    if (-not $p.WaitForExit(150000)) { $p.Kill() }
    Clear-Switches
    Get-Content $txt
}
# close any copy of the game this script opened that is still running (never your own game)
foreach ($p in $started) { if (-not $p.HasExited) { $p.Kill() } }
Get-ChildItem $data | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

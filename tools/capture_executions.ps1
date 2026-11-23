# Pictures of every execution (two key frames each: the wind-up and the blow) and a Steam Deck
# preset timing with executions playing on screen in a full lobby, from a copy of the game (so a
# running game is never touched) and a copy of the save folder (so nothing changes your progress).
#   powershell -File tools\capture_executions.ps1 [-Out shots\executions] [-Bench] [-Only 0,4,12] [-Violence 1] [-Spot wall|side]
# (-Spot: stage them against a wall behind the attacker, or a wall at their right; default open ground)
# Run from app/ after `cargo build --release`.
param([string]$Out = "shots\executions", [switch]$Bench, [string[]]$Only = @(), [int]$Violence = -1, [string]$Spot = "")

$Only = @($Only | ForEach-Object { $_ -split "," } | Where-Object { $_ } | ForEach-Object { [int]$_ })
$dir = Join-Path $env:TEMP "exec_check"
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item "target\release\duel_arena.exe" (Join-Path $dir "duel_arena.exe") -Force
$exe = Join-Path $dir "duel_arena.exe"
$data = Join-Path $dir "data"
if (Test-Path $data) { Get-ChildItem $data | Remove-Item -Recurse -Force }
New-Item -ItemType Directory -Force $data | Out-Null
$real = Join-Path $env:APPDATA "DuelRush"
foreach ($f in "settings.txt", "career.txt") {
    if (Test-Path (Join-Path $real $f)) { Copy-Item (Join-Path $real $f) $data }
}
if ($Violence -ge 0) {
    $s = Join-Path $data "settings.txt"
    $lines = @(); if (Test-Path $s) { $lines = Get-Content $s | Where-Object { $_ -notmatch "^violence=" } }
    ($lines + "violence=$Violence") | Set-Content -Encoding utf8 $s
}
New-Item -ItemType Directory -Force $Out | Out-Null
$outAbs = (Resolve-Path $Out).Path
$keys = "DUEL_EXEC_SPOT", "DUEL_EXEC", "DUEL_EXEC_LOOP", "DUEL_BOTS", "DUEL_SHOT_FRAME", "DUEL_QUALITY", "DUEL_WINDOW", "DUEL_BENCH", "DUEL_DATA_DIR"
function Clear-Switches { foreach ($k in $keys) { Remove-Item "Env:$k" -ErrorAction SilentlyContinue } }

# the moments: a little before each one's blow, and the blow itself (shares of the execution)
$names = @("wardens_verdict", "border_ledger", "lantern_watch", "ridge_fall", "the_collection", "bounty_closed", "frozen_lake", "hook_and_line", "hammer_falls", "quench", "foundry_bell", "anvil_drop", "lights_out", "rooftop_run", "lower_ward_lightning", "skylight")
$hits = @(0.64, 0.72, 0.32, 0.58, 0.6, 0.62, 0.66, 0.6, 0.62, 0.7, 0.74, 0.62, 0.7, 0.74, 0.72, 0.62)
for ($k = 0; $k -lt 16; $k++) {
    if ($Only.Count -gt 0 -and -not ($Only -contains $k)) { continue }
    foreach ($frame in @(@("a_windup", [Math]::Max(0.05, $hits[$k] - 0.2)), @("b_blow", $hits[$k] + 0.012))) {
        Clear-Switches
        $env:DUEL_DATA_DIR = $data
        $env:DUEL_EXEC = "$k," + $frame[1].ToString([Globalization.CultureInfo]::InvariantCulture)
        if ($Spot) { $env:DUEL_EXEC_SPOT = $Spot }
        $env:DUEL_SHOT_FRAME = "3000"
        $path = Join-Path $outAbs ("{0:D2}_{1}_{2}.png" -f $k, $names[$k], $frame[0])
        if (Test-Path $path) { Remove-Item $path }
        $p = Start-Process -FilePath $exe -ArgumentList "--screenshot", "`"$path`"" -PassThru -WindowStyle Minimized
        if (-not $p.WaitForExit(90000)) { $p.Kill() }
        Clear-Switches
    }
}

if ($Bench) {
    # Steam Deck preset, 8 fighters: an execution after another in front of the camera, six
    # bots fighting round it
    Clear-Switches
    $env:DUEL_DATA_DIR = $data
    $env:DUEL_EXEC = "8"; $env:DUEL_EXEC_LOOP = "1"; $env:DUEL_BOTS = "6"; $env:DUEL_QUALITY = "4"; $env:DUEL_WINDOW = "1280x800"
    $txt = Join-Path $outAbs "bench_steamdeck.txt"
    $env:DUEL_BENCH = "40,$txt"
    $p = Start-Process -FilePath $exe -PassThru -WindowStyle Minimized
    if (-not $p.WaitForExit(150000)) { $p.Kill() }
    Clear-Switches
    Get-Content $txt
}
Get-ChildItem $data | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

# Pictures of the wardrobe's outfits in the game, on all four fighters (one of each size), held
# in a pose: standing (front and back), running, wall-running and emoting. From a copy of the game
# (a running game is never touched) and a copy of the save folder.
#   powershell -File tools\capture_fits.ps1 [-Sets herald,smuggler,pilgrim,rig] [-Poses idle,back,run,wall,emote] [-Out shots\outfits]
# Run from app/ after `cargo build --release`. Every game it starts closes itself (any left are closed).
param([string[]]$Sets = @("herald", "smuggler", "pilgrim"), [string[]]$Poses = @("idle", "back", "run", "wall", "emote"), [string]$Out = "shots\outfits")

$Sets = @($Sets | ForEach-Object { $_ -split "," } | Where-Object { $_ })
$Poses = @($Poses | ForEach-Object { $_ -split "," } | Where-Object { $_ })
$dir = Join-Path $env:TEMP "fit_check"
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item "target\release\duel_arena.exe" (Join-Path $dir "duel_arena.exe") -Force
New-Item -ItemType Directory -Force (Join-Path $dir "assets") | Out-Null
Copy-Item "assets\lore.txt" (Join-Path $dir "assets\lore.txt") -Force
$exe = Join-Path $dir "duel_arena.exe"
$data = Join-Path $dir "data"
New-Item -ItemType Directory -Force $data | Out-Null
Get-ChildItem $data | Remove-Item -Recurse -Force
$real = Join-Path $env:APPDATA "DuelRush"
foreach ($f in "settings.txt", "career.txt") {
    if (Test-Path (Join-Path $real $f)) { Copy-Item (Join-Path $real $f) $data }
}
New-Item -ItemType Directory -Force $Out | Out-Null
$outAbs = (Resolve-Path $Out).Path
$keys = "DUEL_HUB", "DUEL_SHOT_FRAME", "DUEL_DATA_DIR", "DUEL_VIEW", "DUEL_GALLERY"
function Clear-Switches { foreach ($k in $keys) { Remove-Item "Env:$k" -ErrorAction SilentlyContinue } }
$started = @()
foreach ($set in $Sets) {
    foreach ($pose in $Poses) {
        Clear-Switches
        $env:DUEL_DATA_DIR = $data
        $env:DUEL_HUB = "fit"
        $env:DUEL_GALLERY = "fit:${set}:${pose}"
        $env:DUEL_SHOT_FRAME = "150"
        # the camera on the plaza, looking at the row (a little higher for the wall-run)
        $env:DUEL_VIEW = if ($pose -eq "wall") { "0,1.9,-7.2,3.14159,-0.06,62" } elseif ($pose -eq "close") { "-0.95,1.3,-11.5,3.14159,-0.03,36" } else { "0,1.2,-7.2,3.14159,-0.02,60" }
        $path = Join-Path $outAbs "${set}_${pose}.png"
        if (Test-Path $path) { Remove-Item $path }
        $p = Start-Process -FilePath $exe -ArgumentList "--screenshot", "`"$path`"" -PassThru -WindowStyle Minimized
        $script:started += $p
        if (-not $p.WaitForExit(120000)) { $p.Kill() }
        Write-Output "${set}_${pose} : $(Test-Path $path)"
    }
}
Clear-Switches
foreach ($p in $started) { if (-not $p.HasExited) { $p.Kill() } }
Get-Process duel_arena -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Stop-Process -Force

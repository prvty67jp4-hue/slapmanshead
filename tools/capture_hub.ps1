# Pictures of the Varrow Hub, from a copy of the game (a running game is never touched) and a copy
# of the save folder: the square from above, a fit in photo mode, the wardrobe and its mirror, an
# aim room, the Rooftop Run with its ghost, a chess game with people watching.
#   powershell -File tools\capture_hub.ps1 [-Only overview,photo,wardrobe,aim,run,chess]
# Run from app/ after `cargo build --release`. Every game it starts closes itself (any left are closed).
param([string[]]$Only = @())

$Only = @($Only | ForEach-Object { $_ -split "," } | Where-Object { $_ })
$dir = Join-Path $env:TEMP "hub_check"
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
$out = "shots\hub"
New-Item -ItemType Directory -Force $out | Out-Null
$outAbs = (Resolve-Path $out).Path
$keys = "DUEL_HUB", "DUEL_SHOT_FRAME", "DUEL_DATA_DIR", "DUEL_VIEW", "DUEL_BODY"
function Clear-Switches { foreach ($k in $keys) { Remove-Item "Env:$k" -ErrorAction SilentlyContinue } }
$started = @()

function Invoke-Shot([string]$group, [string]$name, [hashtable]$set) {
    if ($Only.Count -gt 0 -and -not ($Only -contains $group)) { return }
    Clear-Switches
    $env:DUEL_DATA_DIR = $data
    foreach ($k in $set.Keys) { Set-Item "Env:$k" $set[$k] }
    $path = Join-Path $outAbs "$name.png"
    if (Test-Path $path) { Remove-Item $path }
    $p = Start-Process -FilePath $exe -ArgumentList "--screenshot", "`"$path`"" -PassThru -WindowStyle Minimized
    $script:started += $p
    if (-not $p.WaitForExit(180000)) { $p.Kill() }
    Clear-Switches
    Write-Output "$name : $(Test-Path $path)"
}

# the square from above the tailor's: the fountain, the pits beyond, the pavilion and the Gallery
Invoke-Shot "overview" "hub_overview" @{ DUEL_HUB = "overview"; DUEL_SHOT_FRAME = "900"; DUEL_VIEW = "14,21,-36,-0.36,-0.45,72" }
# photo mode: a River Smuggler with a herald's cape on the podium, saluting
Invoke-Shot "photo" "hub_photo_mode" @{ DUEL_HUB = "photo"; DUEL_SHOT_FRAME = "260" }
# the wardrobe open, the mirror showing you in the Court Herald's set
Invoke-Shot "wardrobe" "hub_wardrobe" @{ DUEL_HUB = "wardrobe"; DUEL_SHOT_FRAME = "200" }
# an aim room: static flicks, a target up
Invoke-Shot "aim" "hub_aim_room" @{ DUEL_HUB = "aim"; DUEL_SHOT_FRAME = "900" }
# the Rooftop Run: on the pavilion roof, your best run's ghost ahead
Invoke-Shot "run" "hub_rooftop_run" @{ DUEL_HUB = "run"; DUEL_SHOT_FRAME = "640" }
# chess at the pavilion: two regulars playing, four watching, you by the table
Invoke-Shot "chess" "hub_chess" @{ DUEL_HUB = "chess"; DUEL_SHOT_FRAME = "300" }

foreach ($p in $started) { if (-not $p.HasExited) { $p.Kill() } }
Get-Process duel_arena -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Stop-Process -Force

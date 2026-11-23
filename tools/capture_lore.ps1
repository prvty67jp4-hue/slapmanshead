# Pictures of the story features, from a copy of the game (a running game is never touched) and a
# copy of the save folder: the Lore page, the character select, a duel's intro card and a voice line.
#   powershell -File tools\capture_lore.ps1 [-Only lore,select,intro,voice] [-Frame <n>]
# Run from app/ after `cargo build --release`. Every game it starts closes itself (any left are closed).
param([string[]]$Only = @(), [int]$Frame = 0)

$Only = @($Only | ForEach-Object { $_ -split "," } | Where-Object { $_ })
$dir = Join-Path $env:TEMP "lore_check"
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
$out = "shots\lore"
New-Item -ItemType Directory -Force $out | Out-Null
$outAbs = (Resolve-Path $out).Path
$keys = "DUEL_MAP", "DUEL_MODE", "DUEL_AUTOPLAY", "DUEL_BOTS", "DUEL_SHOT_FRAME", "DUEL_DATA_DIR", "DUEL_PAGE", "DUEL_BODY", "DUEL_BOT_BODY", "DUEL_HUMAN", "DUEL_HUMAN_READY", "DUEL_WINDOW", "DUEL_KILLS"
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
    if (-not $p.WaitForExit(120000)) { $p.Kill() }
    Clear-Switches
    Write-Output "$name : $(Test-Path $path)"
}

Invoke-Shot "lore" "lore_world" @{ DUEL_PAGE = "lore:0" }
Invoke-Shot "lore" "lore_fighter_corvin" @{ DUEL_PAGE = "lore:1" }
Invoke-Shot "lore" "lore_places" @{ DUEL_PAGE = "lore:5" }
# a new player: only the first entry of each fighter is open
$fresh = Join-Path $dir "fresh"; New-Item -ItemType Directory -Force $fresh | Out-Null; Get-ChildItem $fresh | Remove-Item -Recurse -Force
Invoke-Shot "lore" "lore_fighter_sable_new_player" @{ DUEL_PAGE = "lore:2"; DUEL_DATA_DIR = $fresh }
Invoke-Shot "select" "character_select_sable" @{ DUEL_PAGE = "loadout"; DUEL_BODY = "1" }
Invoke-Shot "select" "character_select_anvil" @{ DUEL_PAGE = "loadout"; DUEL_BODY = "2" }
$f = if ($Frame -gt 0) { "$Frame" } else { "200" }
# a duel between rivals: Corvin Vale (you, ready at once) against Sable Rook, in the countdown
Invoke-Shot "intro" "intro_card_rivals" @{ DUEL_AUTOPLAY = "1"; DUEL_HUMAN = "1"; DUEL_HUMAN_READY = "1"; DUEL_MODE = "2"; DUEL_BOTS = "1"; DUEL_BODY = "0"; DUEL_BOT_BODY = "1"; DUEL_SHOT_FRAME = $f }
$v = if ($Frame -gt 0) { "$($Frame + 260)" } else { "740" }
# just after the bell: the announcer and the fighters' first words
Invoke-Shot "voice" "voice_line_subtitle" @{ DUEL_AUTOPLAY = "1"; DUEL_HUMAN = "1"; DUEL_HUMAN_READY = "1"; DUEL_MODE = "2"; DUEL_BOTS = "1"; DUEL_BODY = "0"; DUEL_BOT_BODY = "1"; DUEL_SHOT_FRAME = $v }
Invoke-Shot "loading" "prematch_loadout_marble_court" @{ DUEL_AUTOPLAY = "1"; DUEL_HUMAN = "1"; DUEL_MODE = "0"; DUEL_BOTS = "1"; DUEL_MAP = "1"; DUEL_SHOT_FRAME = "120" }
$v2 = if ($Frame -gt 0) { "$($Frame + 420)" } else { "900" }
# a moment later: your first words and the announcer still up
Invoke-Shot "voice" "voice_line_subtitle_2" @{ DUEL_AUTOPLAY = "1"; DUEL_HUMAN = "1"; DUEL_HUMAN_READY = "1"; DUEL_MODE = "2"; DUEL_BOTS = "1"; DUEL_BODY = "0"; DUEL_BOT_BODY = "1"; DUEL_SHOT_FRAME = $v2 }
Invoke-Shot "loading" "map_card_undercroft" @{ DUEL_AUTOPLAY = "1"; DUEL_HUMAN = "1"; DUEL_HUMAN_READY = "1"; DUEL_MODE = "0"; DUEL_BOTS = "3"; DUEL_MAP = "2"; DUEL_SHOT_FRAME = $f }

foreach ($p in $started) { if (-not $p.HasExited) { $p.Kill() } }
Get-Process duel_arena -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Stop-Process -Force
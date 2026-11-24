# Pictures of Ledger Duels, from a copy of the game (a running game is never touched) and a copy of
# the save folder: the gear screen, the Court's terms (rules), a gear switch and a special winding up.
#   powershell -File tools\capture_ledger.ps1 [-Only gear,rules,swap,spec,rig,haul] [-Fight <frame the fight is on>]
# Run from app/ after `cargo build --release`. Every game it starts closes itself (any left are closed).
param([string[]]$Only = @(), [int]$Fight = 720)

$Only = @($Only | ForEach-Object { $_ -split "," } | Where-Object { $_ })
$dir = Join-Path $env:TEMP "ledger_check"
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
$out = "shots\ledger"
New-Item -ItemType Directory -Force $out | Out-Null
$outAbs = (Resolve-Path $out).Path
$keys = "DUEL_MAP", "DUEL_MODE", "DUEL_AUTOPLAY", "DUEL_BOTS", "DUEL_SHOT_FRAME", "DUEL_DATA_DIR", "DUEL_PAGE", "DUEL_BODY", "DUEL_HUMAN", "DUEL_HUMAN_READY", "DUEL_LEDGER_RULES", "DUEL_LEDGER_KIT", "DUEL_LEDGER_FORCE", "DUEL_SHOT_AFTER"
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

Invoke-Shot "gear" "gear_screen" @{ DUEL_PAGE = "gear"; DUEL_LEDGER_KIT = "0" }
# the Court's terms before the duel: no food, no sets, best of 5
Invoke-Shot "rules" "rules_screen" @{ DUEL_AUTOPLAY = "1"; DUEL_HUMAN = "1"; DUEL_MODE = "8"; DUEL_BOTS = "1"; DUEL_LEDGER_RULES = "68,5"; DUEL_SHOT_FRAME = "90" }
# mid-duel: a swap to the pack's first item (the Ashfall Cleaver), part way through
Invoke-Shot "swap" "gear_switch" @{ DUEL_AUTOPLAY = "1"; DUEL_MODE = "8"; DUEL_BOTS = "1"; DUEL_MAP = "0"; DUEL_LEDGER_FORCE = "swap@$Fight"; DUEL_SHOT_FRAME = "$($Fight + 24)" }
# mid-duel: both sides winding up a special (the tell over the opponent's head, yours on the HUD)
# the Bridgewright's Rig on your fighter (its own colours: oiled brown, brass, teal), front and back
Invoke-Shot "rig" "rig_front" @{ DUEL_PAGE = "gear_preview"; DUEL_LEDGER_KIT = "8"; DUEL_BODY = "0" }
Invoke-Shot "rig" "rig_back" @{ DUEL_PAGE = "gear_preview_back"; DUEL_LEDGER_KIT = "8"; DUEL_BODY = "0" }
Invoke-Shot "rig" "rig_front_sable" @{ DUEL_PAGE = "gear_preview"; DUEL_LEDGER_KIT = "8"; DUEL_BODY = "1" }
Invoke-Shot "rig" "rig_back_ilya" @{ DUEL_PAGE = "gear_preview_back"; DUEL_LEDGER_KIT = "8"; DUEL_BODY = "3" }
# Haul: the chain rattles and the hook glows teal (the wind-up), then the hook flies
Invoke-Shot "haul" "haul_windup" @{ DUEL_AUTOPLAY = "1"; DUEL_MODE = "8"; DUEL_BOTS = "1"; DUEL_MAP = "0"; DUEL_LEDGER_KIT = "8"; DUEL_LEDGER_FORCE = "haul@$Fight"; DUEL_SHOT_AFTER = "45" }
Invoke-Shot "haul" "haul_thrown" @{ DUEL_AUTOPLAY = "1"; DUEL_MODE = "8"; DUEL_BOTS = "1"; DUEL_MAP = "0"; DUEL_LEDGER_KIT = "8"; DUEL_LEDGER_FORCE = "haul@$Fight"; DUEL_SHOT_AFTER = "74" }
Invoke-Shot "spec" "spec_windup" @{ DUEL_AUTOPLAY = "1"; DUEL_MODE = "8"; DUEL_BOTS = "1"; DUEL_MAP = "0"; DUEL_LEDGER_FORCE = "spec@$Fight"; DUEL_SHOT_FRAME = "$($Fight + 22)" }

foreach ($p in $started) { if (-not $p.HasExited) { $p.Kill() } }
Get-Process duel_arena -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Stop-Process -Force
# Pictures of the four fighters for before / after comparisons, and a Steam Deck preset timing run.
#   powershell -File tools\capture_fighters.ps1 -Out shots\character-art\after [-Exe path\to\duel_arena.exe] [-Bench]
# Run from app/ after `cargo build --release`.
#
# The runs use a copy of your save folder (settings and progress), so the matches they play
# never touch your real one.
param([string]$Out = "shots\character-art\after", [string]$Exe = "target\release\duel_arena.exe", [switch]$Bench, [switch]$NoShots)

New-Item -ItemType Directory -Force $Out | Out-Null
$keys = "DUEL_MAP", "DUEL_PAGE", "DUEL_BODY", "DUEL_AUTOPLAY", "DUEL_BOTS", "DUEL_GALLERY", "DUEL_GALLERY_Z", "DUEL_VIEW", "DUEL_SHOT_FRAME", "DUEL_QUALITY", "DUEL_WINDOW", "DUEL_BENCH", "DUEL_DATA_DIR"

$data = Join-Path $env:TEMP "duel_capture_data"
if (Test-Path $data) { Remove-Item $data -Recurse -Force }
New-Item -ItemType Directory -Force $data | Out-Null
$real = Join-Path $env:APPDATA "DuelRush"
foreach ($f in "settings.txt", "career.txt") {
    if (Test-Path (Join-Path $real $f)) { Copy-Item (Join-Path $real $f) $data }
}

function Clear-Switches { foreach ($k in $keys) { Remove-Item "Env:$k" -ErrorAction SilentlyContinue } }

function Invoke-Shot([hashtable]$vars, [string]$file) {
    Clear-Switches
    $env:DUEL_DATA_DIR = $data
    foreach ($k in $vars.Keys) { Set-Item "Env:$k" $vars[$k] }
    $path = Join-Path $Out $file
    if (Test-Path $path) { Remove-Item $path }
    $p = Start-Process -FilePath $Exe -ArgumentList "--screenshot", $path -PassThru -WindowStyle Minimized
    if (-not $p.WaitForExit(90000)) { $p.Kill() }
    Clear-Switches
}

if (-not $NoShots) {
    $names = "ranger", "huntress", "brawler", "striker"
    for ($b = 0; $b -lt 4; $b++) {
        Invoke-Shot @{ DUEL_PAGE = "main"; DUEL_BODY = "$b" } "menu_$($names[$b]).png"
        Invoke-Shot @{ DUEL_PAGE = "loadout"; DUEL_BODY = "$b" } "loadout_$($names[$b]).png"
    }
    # in a match: the four side by side on the Courtyard, running with a katana (the gallery's
    # carry page), seen by a camera at a person's eye height from 5 m (straight on) and 15 m
    # (from the side the four are turned to: the clear 15 m line of sight there)
    $views = @{ 5 = "0,1.7,4.5,0,-0.03,70"; 15 = "-10.6,1.7,-1.1,0.785,-0.03,70" }
    foreach ($d in 5, 15) {
        $common = @{ DUEL_MAP = "1"; DUEL_AUTOPLAY = "1"; DUEL_BOTS = "3"; DUEL_GALLERY = "carry:1:0"; DUEL_GALLERY_Z = "9.5"; DUEL_VIEW = $views[$d]; DUEL_SHOT_FRAME = "150" }
        Invoke-Shot $common "match_$($d)m.png"
        $deck = $common.Clone(); $deck.DUEL_QUALITY = "4"; $deck.DUEL_WINDOW = "1280x800"
        Invoke-Shot $deck "match_$($d)m_steamdeck.png"
    }
}

if ($Bench) {
    # an 8-fighter Free-for-All on the Steam Deck preset at the Deck's resolution, frame rate unlocked
    Clear-Switches
    $env:DUEL_DATA_DIR = $data
    $env:DUEL_AUTOPLAY = "1"; $env:DUEL_BOTS = "7"; $env:DUEL_QUALITY = "4"; $env:DUEL_WINDOW = "1280x800"
    $txt = Join-Path (Resolve-Path $Out) "bench_steamdeck.txt"
    $env:DUEL_BENCH = "30,$txt"
    $p = Start-Process -FilePath $Exe -PassThru -WindowStyle Minimized
    if (-not $p.WaitForExit(120000)) { $p.Kill() }
    Clear-Switches
    Get-Content $txt
}
Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue

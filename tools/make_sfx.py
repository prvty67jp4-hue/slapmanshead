"""Cut single shots / swishes out of the downloaded CC0 recordings into 44.1 kHz mono 16-bit WAVs,
and copy the chosen Kenney (CC0) clips and music into app/assets.

Usage: python -I make_sfx.py <downloads dir> <app/assets dir>
"""
import math
import os
import shutil
import struct
import sys
import wave

DL, ASSETS = sys.argv[1], sys.argv[2]
SFX = os.path.join(ASSETS, "sfx")
MUS = os.path.join(ASSETS, "music")
os.makedirs(SFX, exist_ok=True)
os.makedirs(MUS, exist_ok=True)
OUT_SR = 44100


def read_mono(path):
    w = wave.open(path, "rb")
    ch, sw, sr, n = w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()
    raw = w.readframes(n)
    w.close()
    out = []
    step = sw * ch
    for i in range(0, len(raw) - step + 1, step):
        acc = 0.0
        for c in range(ch):
            o = i + c * sw
            if sw == 2:
                v = struct.unpack_from("<h", raw, o)[0] / 32768.0
            elif sw == 3:
                v = int.from_bytes(raw[o:o + 3], "little", signed=True) / 8388608.0
            else:
                v = (raw[o] - 128) / 128.0
            acc += v
        out.append(acc / ch)
    return out, sr


def lowpass(s, sr, cutoff):
    """Windowed-sinc low-pass so resampling down doesn't alias."""
    taps = 31
    fc = cutoff / sr
    h = []
    for k in range(taps):
        m = k - (taps - 1) / 2
        x = 2 * fc if m == 0 else math.sin(2 * math.pi * fc * m) / (math.pi * m)
        h.append(x * (0.42 - 0.5 * math.cos(2 * math.pi * k / (taps - 1)) + 0.08 * math.cos(4 * math.pi * k / (taps - 1))))
    g = sum(h)
    h = [x / g for x in h]
    half = taps // 2
    n = len(s)
    out = [0.0] * n
    for i in range(n):
        acc = 0.0
        for k in range(taps):
            j = i + k - half
            if 0 <= j < n:
                acc += s[j] * h[k]
        out[i] = acc
    return out


def resample(s, sr):
    if sr == OUT_SR:
        return s
    if sr > OUT_SR:
        s = lowpass(s, sr, 19000)
    ratio = sr / OUT_SR
    n = int(len(s) / ratio)
    out = []
    for i in range(n):
        x = i * ratio
        j = int(x)
        f = x - j
        a = s[j]
        b = s[j + 1] if j + 1 < len(s) else a
        out.append(a + (b - a) * f)
    return out


def write(name, s, peak=0.9):
    m = max(1e-6, max(abs(x) for x in s))
    k = peak / m
    w = wave.open(os.path.join(SFX, name), "wb")
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(OUT_SR)
    w.writeframes(b"".join(struct.pack("<h", max(-32767, min(32767, int(x * k * 32767)))) for x in s))
    w.close()
    print(f"{name}: {len(s)/OUT_SR:.2f}s")


def cut(src, start, length, name, peak=0.9):
    """One clip from `start` seconds, `length` long, with a short fade in and a smooth fade out."""
    s, sr = read_mono(os.path.join(DL, src))
    a = max(0, int((start - 0.008) * sr))
    b = min(len(s), a + int(length * sr))
    seg = resample(s[a:b], sr)
    n = len(seg)
    fin = int(0.002 * OUT_SR)
    for i in range(min(fin, n)):
        seg[i] *= i / fin
    fstart = int(n * 0.55)
    for i in range(fstart, n):
        t = (i - fstart) / max(1, n - fstart)
        seg[i] *= (1 - t) ** 2
    write(name, seg, peak)


oga = "oga"
# gunshots (kurt, CC0): two separate shots of each, for variety
cut(f"{oga}/Black Powder.wav", 0.14, 0.66, "shotgun_1.wav")
cut(f"{oga}/Black Powder.wav", 0.84, 0.66, "shotgun_2.wav")
cut(f"{oga}/22 Magnum.wav", 0.23, 0.62, "revolver_1.wav")
cut(f"{oga}/22 Magnum.wav", 0.93, 0.62, "revolver_2.wav")
cut(f"{oga}/22 Pistol.wav", 0.14, 0.30, "smg_1.wav", 0.8)
cut(f"{oga}/22 Pistol.wav", 0.59, 0.30, "smg_2.wav", 0.8)
cut(f"{oga}/Unkown.wav", 0.26, 1.10, "rifle_1.wav")
cut(f"{oga}/Unkown.wav", 2.31, 1.10, "rifle_2.wav")
# swishes (artisticdude, CC0): sword swings, dash, throws
sw = f"{oga}/swishes/swishes"
for i, n in enumerate([3, 4, 6, 8]):
    s, sr = read_mono(os.path.join(DL, f"{sw}/swish-{n}.wav"))
    write(f"swing_{i + 1}.wav", resample(s, sr), 0.85)
for i, n in enumerate([9, 7]):
    s, sr = read_mono(os.path.join(DL, f"{sw}/swish-{n}.wav"))
    write(f"dash_{i + 1}.wav", resample(s, sr), 0.7)
s, sr = read_mono(os.path.join(DL, f"{sw}/swish-12.wav"))
write("throw.wav", resample(s, sr), 0.6)

# Kenney (CC0) clips, copied as they are
K = "kenney"
picks = {
    "clang_1.ogg": "impact-sounds/Audio/impactMetal_heavy_000.ogg",
    "clang_2.ogg": "impact-sounds/Audio/impactMetal_heavy_001.ogg",
    "clang_3.ogg": "impact-sounds/Audio/impactPlate_heavy_002.ogg",
    "flesh_1.ogg": "impact-sounds/Audio/impactPunch_heavy_000.ogg",
    "flesh_2.ogg": "impact-sounds/Audio/impactPunch_heavy_001.ogg",
    "flesh_3.ogg": "impact-sounds/Audio/impactPunch_heavy_002.ogg",
    "bullet_flesh_1.ogg": "impact-sounds/Audio/impactPunch_medium_000.ogg",
    "bullet_flesh_2.ogg": "impact-sounds/Audio/impactPunch_medium_001.ogg",
    "impact_stone_1.ogg": "impact-sounds/Audio/impactMining_000.ogg",
    "impact_stone_2.ogg": "impact-sounds/Audio/impactMining_001.ogg",
    "impact_stone_3.ogg": "impact-sounds/Audio/impactMining_002.ogg",
    "hurt_1.ogg": "impact-sounds/Audio/impactPunch_heavy_003.ogg",
    "hurt_2.ogg": "impact-sounds/Audio/impactPunch_heavy_004.ogg",
    "land_1.ogg": "impact-sounds/Audio/impactSoft_heavy_000.ogg",
    "land_2.ogg": "impact-sounds/Audio/impactSoft_heavy_001.ogg",
    "headshot.ogg": "impact-sounds/Audio/impactBell_heavy_000.ogg",
    "jump_1.ogg": "rpg-audio/Audio/cloth1.ogg",
    "jump_2.ogg": "rpg-audio/Audio/cloth2.ogg",
    "wall_1.ogg": "rpg-audio/Audio/cloth3.ogg",
    "wall_2.ogg": "rpg-audio/Audio/cloth4.ogg",
    "reload.ogg": "rpg-audio/Audio/metalLatch.ogg",
    "reload_end.ogg": "rpg-audio/Audio/metalClick.ogg",
    "switch_blade.ogg": "rpg-audio/Audio/drawKnife1.ogg",
    "switch_gun.ogg": "rpg-audio/Audio/beltHandle1.ogg",
    "heal.ogg": "rpg-audio/Audio/handleSmallLeather.ogg",
    "armor.ogg": "rpg-audio/Audio/metalPot1.ogg",
    "boom_1.ogg": "sci-fi-sounds/Audio/lowFrequency_explosion_000.ogg",
    "boom_2.ogg": "sci-fi-sounds/Audio/lowFrequency_explosion_001.ogg",
    "boom_crunch.ogg": "sci-fi-sounds/Audio/explosionCrunch_000.ogg",
    "rocket.ogg": "sci-fi-sounds/Audio/thrusterFire_000.ogg",
    "ui_click.ogg": "interface-sounds/Audio/click_002.ogg",
    "ui_confirm.ogg": "interface-sounds/Audio/confirmation_001.ogg",
    "ui_back.ogg": "interface-sounds/Audio/back_001.ogg",
    "ui_tick.ogg": "interface-sounds/Audio/tick_002.ogg",
    "hitmark.ogg": "interface-sounds/Audio/tick_001.ogg",
    "kill.ogg": "interface-sounds/Audio/confirmation_002.ogg",
    "match_start.ogg": "music-jingles/Audio/jingles_STEEL00.ogg",
    "match_over.ogg": "music-jingles/Audio/jingles_HIT04.ogg",
}
for i in range(5):
    picks[f"step_stone_{i + 1}.ogg"] = f"impact-sounds/Audio/footstep_concrete_00{i}.ogg"
    picks[f"step_wood_{i + 1}.ogg"] = f"impact-sounds/Audio/footstep_wood_00{i}.ogg"
for dst, src in picks.items():
    p = os.path.join(DL, K, src)
    if not os.path.exists(p):
        # the jingles pack keeps its files one folder up
        alt = os.path.join(DL, K, src.replace("music-jingles/Audio/", "music-jingles/Audio/"))
        print("MISSING", src)
        continue
    shutil.copyfile(p, os.path.join(SFX, dst))

# music (CC0)
for dst, src in {"menu.ogg": "music/ambientmain_0.ogg", "fight.ogg": "music/fight.ogg", "battle.mp3": "music/battleThemeA.mp3"}.items():
    shutil.copyfile(os.path.join(DL, src), os.path.join(MUS, dst))
print("done")

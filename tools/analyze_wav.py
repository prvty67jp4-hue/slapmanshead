"""Print duration, peak level and loud onsets of WAV files (for picking single gunshots)."""
import struct
import sys
import wave


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
                b = raw[o:o + 3]
                v = int.from_bytes(b, "little", signed=True) / 8388608.0
            else:
                v = (raw[o] - 128) / 128.0
            acc += v
        out.append(acc / ch)
    return out, sr


for p in sys.argv[1:]:
    s, sr = read_mono(p)
    win = sr // 100  # 10 ms
    env = [max(abs(x) for x in s[i:i + win]) for i in range(0, len(s), win)]
    peak = max(env)
    onsets = []
    prev = 0.0
    for k, e in enumerate(env):
        if e > peak * 0.35 and prev < peak * 0.12:
            onsets.append(round(k * 0.01, 2))
        prev = max(prev * 0.93, e)
    print(f"{p.split(chr(92))[-1]}: {len(s)/sr:.2f}s sr={sr} peak={peak:.2f} onsets={onsets[:12]}")

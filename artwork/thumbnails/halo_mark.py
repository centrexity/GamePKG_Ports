"""Original Halo catalog mark. No Halo artwork, logo, or ringed planet.

Players do not run this. It writes ports/halo/thumbnail.png.
"""
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "halo" / "thumbnail.png"
W = H = 512


def png(pixels: bytes) -> bytes:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    raw = b"".join(b"\x00" + pixels[y * W * 4 : (y + 1) * W * 4] for y in range(H))
    ihdr = struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def put(buf: bytearray, x: int, y: int, color: tuple[int, int, int, int]) -> None:
    if 0 <= x < W and 0 <= y < H:
        i = (y * W + x) * 4
        buf[i : i + 4] = bytes(color)


def fill_round_rect(
    buf: bytearray,
    x0: int,
    y0: int,
    x1: int,
    y1: int,
    radius: int,
    color: tuple[int, int, int, int],
) -> None:
    for y in range(y0, y1):
        for x in range(x0, x1):
            dx = dy = 0
            if x < x0 + radius and y < y0 + radius:
                dx, dy = x0 + radius - x, y0 + radius - y
            elif x >= x1 - radius and y < y0 + radius:
                dx, dy = x - (x1 - radius - 1), y0 + radius - y
            elif x < x0 + radius and y >= y1 - radius:
                dx, dy = x0 + radius - x, y - (y1 - radius - 1)
            elif x >= x1 - radius and y >= y1 - radius:
                dx, dy = x - (x1 - radius - 1), y - (y1 - radius - 1)
            if dx * dx + dy * dy <= radius * radius:
                put(buf, x, y, color)


def main() -> None:
    buf = bytearray(W * H * 4)
    for y in range(H):
        t = y / (H - 1)
        row = bytes(
            (
                int(16 + 10 * t),
                int(22 + 12 * t),
                int(30 + 16 * t),
                255,
            )
        )
        for x in range(W):
            i = (y * W + x) * 4
            buf[i : i + 4] = row
    panel = (36, 52, 68, 255)
    fill_round_rect(buf, 84, 72, 428, 440, 36, panel)
    bars = (
        (128, 168, 384, 210, (230, 194, 122, 255)),
        (128, 230, 352, 272, (143, 180, 201, 255)),
        (128, 292, 312, 334, (196, 208, 216, 255)),
    )
    for x0, y0, x1, y1, color in bars:
        fill_round_rect(buf, x0, y0, x1, y1, 8, color)
    OUT.write_bytes(png(bytes(buf)))
    print(OUT, OUT.stat().st_size)


if __name__ == "__main__":
    main()

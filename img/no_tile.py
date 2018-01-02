#!/usr/bin/python3

import cairo
import math

img = cairo.ImageSurface(cairo.FORMAT_ARGB32, 256, 256)
cx = cairo.Context(img)
cx.set_source_rgb(0.125, 0.125, 0.125)
cx.paint()

cx.set_source_rgb(0.25, 0.25, 0.25)

for i in range(1, 8):
    num = 2 ** i
    for j in range(0, num + 1):
        x = float(j) * 256.0 / num
        w = 8.0 / num
        cx.rectangle(x - w * 0.5, 0, w, 256)
        cx.rectangle(0, x - w * 0.5, 256, w)

cx.fill()

img.write_to_png('no_tile.png')

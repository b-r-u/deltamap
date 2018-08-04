#version 100
precision highp float;

attribute vec2 position;

uniform vec2 scale;

varying vec2 v_pos;

void main() {
    gl_Position = vec4(position * scale, 0.0, 1.0);
    v_pos = position;
}

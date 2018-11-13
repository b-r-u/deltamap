#version 100
precision highp float;

attribute vec2 position;
attribute vec2 extrusion;

uniform vec2 scale;
uniform float half_width;

varying vec2 v_extrusion;

void main() {
    gl_Position = vec4(position + extrusion * scale * half_width, 0.0, 1.0);
    v_extrusion = extrusion;
}

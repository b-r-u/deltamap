#version 100
precision highp float;

uniform float half_width;
uniform vec3 color;

varying vec2 v_extrusion;

void main() {
    float len = length(v_extrusion);
    gl_FragColor = vec4(color, min(half_width - half_width * len, 1.0));
}

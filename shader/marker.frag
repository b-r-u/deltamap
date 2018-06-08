#version 100
precision mediump float;

varying vec2 v_tex;
uniform sampler2D tex;

void main() {
    gl_FragColor = texture2D(tex, v_tex.xy).rgba;
}

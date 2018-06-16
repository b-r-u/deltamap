#version 100
precision mediump float;

varying vec2 v_tex;
uniform sampler2D tex_map;

void main() {
    gl_FragColor = vec4(texture2D(tex_map, v_tex.xy).rgb, 1.0);
}

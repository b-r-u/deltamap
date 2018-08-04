#version 100
precision highp float;

varying vec2 v_tex;
varying vec4 v_tex_minmax;
uniform sampler2D tex_map;

void main() {
    gl_FragColor = vec4(texture2D(tex_map, clamp(v_tex.xy, v_tex_minmax.xy, v_tex_minmax.zw)).rgb, 1.0);
}

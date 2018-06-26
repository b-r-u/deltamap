#version 100
precision mediump float;

varying vec2 v_tex;
varying vec4 v_tex_minmax;
uniform sampler2D tex_map;

void main() {
    vec2 mid = 0.5 * (v_tex_minmax.zw + v_tex_minmax.xy);
    vec2 scale = 1.0 / (v_tex_minmax.zw - v_tex_minmax.xy);
    vec2 dist = abs((v_tex - mid) * scale);
    float shade = 1.0 - step(0.49, max(dist.x, dist.y)) * 0.5;
    float add = step(0.4975, max(dist.x, dist.y)) * 0.5;
    gl_FragColor = vec4(texture2D(tex_map, clamp(v_tex.xy, v_tex_minmax.xy, v_tex_minmax.zw)).rgb * shade + vec3(add), 1.0);
}

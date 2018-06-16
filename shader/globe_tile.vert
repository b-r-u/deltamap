#version 100
precision mediump float;

attribute vec3 position;
attribute vec2 tex_coord;

varying vec2 v_tex;

void main() {
    gl_Position = vec4(position.xy, 0.0, 1.0);
    v_tex = tex_coord;
}

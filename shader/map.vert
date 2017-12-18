#version 100
precision mediump float;

attribute vec2 position;
attribute vec2 tex_coord;
attribute vec4 tex_minmax;

varying vec2 v_tex;
varying vec4 v_tex_minmax;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    v_tex = tex_coord;
    v_tex_minmax = tex_minmax;
}

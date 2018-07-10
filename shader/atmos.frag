#version 100
precision mediump float;

varying vec2 v_pos;

void main() {
    //float len = abs(1.0 - length(v_pos));
    //float val = (1.0 - sqrt(sqrt(len*4.0))) * (1.0 - len*5.0);
    float len = length(v_pos);
    //float val = max(0.0, sqrt(1.1*1.1 - len*len) * 1.5) * step(0.99, len);
    float val = exp(abs(1.0 - len)*(-16.0 - step(1.0, len)*16.0));
    gl_FragColor = vec4(sin(val*1.0) * 0.8 + 0.2, sin(val*1.5) * 0.8 + 0.2 , 1.0, val * 1.0);
}

#version 100

precision highp float;

varying vec2 v_coords;

uniform sampler2D tex;
uniform vec2 half_pixel;
uniform float offset;

// 1.0 for samples inside the captured region, 0.0 outside (see blur_down.frag).
float tap_weight(vec2 coord) {
    if (coord.x < 0.0 || coord.x > 1.0 || coord.y < 0.0 || coord.y > 1.0)
        return 0.0;
    return 1.0;
}

void main() {
    vec2 o = half_pixel * offset;

    vec4 sum = vec4(0.0);
    float weight_sum = 0.0;

    // Four edge centers
    vec2 sample_coord = v_coords + vec2(-o.x * 2.0, 0.0);
    float weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(o.x * 2.0, 0.0);
    weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(0.0, -o.y * 2.0);
    weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(0.0, o.y * 2.0);
    weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    // Four diagonal corners (double-weighted)
    sample_coord = v_coords + vec2(-o.x, o.y);
    weight = tap_weight(sample_coord) * 2.0;
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(o.x, o.y);
    weight = tap_weight(sample_coord) * 2.0;
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(-o.x, -o.y);
    weight = tap_weight(sample_coord) * 2.0;
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(o.x, -o.y);
    weight = tap_weight(sample_coord) * 2.0;
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    // For a degenerate 1-pixel source every tap falls outside the region; fall back to
    // the plain value instead of dividing by zero.
    if (weight_sum == 0.0)
        gl_FragColor = texture2D(tex, v_coords);
    else
        gl_FragColor = sum / weight_sum;
}
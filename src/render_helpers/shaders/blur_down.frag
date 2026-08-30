#version 100

precision highp float;

varying vec2 v_coords;

uniform sampler2D tex;
uniform vec2 half_pixel;
uniform float offset;

// 1.0 for samples inside the captured region, 0.0 outside. This prevents the blur
// from sampling past the valid data with the symmetric CLAMP_TO_EDGE extension,
// which would smear border pixels inward and produce the edge halo.
float tap_weight(vec2 coord) {
    if (coord.x < 0.0 || coord.x > 1.0 || coord.y < 0.0 || coord.y > 1.0)
        return 0.0;
    return 1.0;
}

void main() {
    vec2 o = half_pixel * offset;

    vec4 sum = vec4(0.0);
    float weight_sum = 0.0;

    vec2 sample_coord = v_coords;
    float weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(-o.x, -o.y);
    weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(o.x, -o.y);
    weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(-o.x, o.y);
    weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    sample_coord = v_coords + vec2(o.x, o.y);
    weight = tap_weight(sample_coord);
    sum += texture2D(tex, sample_coord) * weight;
    weight_sum += weight;

    // The center tap always lands inside the region, so weight_sum can't be zero.
    gl_FragColor = sum / weight_sum;
}
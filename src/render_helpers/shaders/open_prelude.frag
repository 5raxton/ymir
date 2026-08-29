precision highp float;

#if defined(DEBUG_FLAGS)
uniform float ymir_tint;
#endif

varying vec2 ymir_v_coords;
uniform vec2 ymir_size;

uniform mat3 ymir_input_to_geo;
uniform vec2 ymir_geo_size;

uniform sampler2D ymir_tex;
uniform mat3 ymir_geo_to_tex;

uniform float ymir_progress;
uniform float ymir_clamped_progress;
uniform float ymir_random_seed;

uniform float ymir_alpha;
uniform float ymir_scale;


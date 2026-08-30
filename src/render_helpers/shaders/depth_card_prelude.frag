precision highp float;

#if defined(DEBUG_FLAGS)
uniform float ymir_tint;
#endif

varying vec2 ymir_v_coords;
uniform vec2 ymir_size;

uniform sampler2D ymir_tex;

uniform float ymir_alpha;
uniform float ymir_scale;

uniform float ymir_depth_bottom;
uniform float ymir_depth_tilt_pow;
uniform float ymir_min_opacity;
uniform vec4 ymir_corner_radius;

float ymir_rounding_alpha(vec2 coords, vec2 size, vec4 corner_radius);
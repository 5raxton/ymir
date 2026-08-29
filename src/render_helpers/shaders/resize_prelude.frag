precision highp float;

#if defined(DEBUG_FLAGS)
uniform float ymir_tint;
#endif

varying vec2 ymir_v_coords;
uniform vec2 ymir_size;

uniform mat3 ymir_input_to_curr_geo;
uniform mat3 ymir_curr_geo_to_prev_geo;
uniform mat3 ymir_curr_geo_to_next_geo;
uniform vec2 ymir_curr_geo_size;

uniform sampler2D ymir_tex_prev;
uniform mat3 ymir_geo_to_tex_prev;

uniform sampler2D ymir_tex_next;
uniform mat3 ymir_geo_to_tex_next;

uniform float ymir_progress;
uniform float ymir_clamped_progress;

uniform vec4 ymir_corner_radius;
uniform float ymir_clip_to_geometry;

uniform float ymir_alpha;
uniform float ymir_scale;

float ymir_rounding_alpha(vec2 coords, vec2 size, vec4 corner_radius);

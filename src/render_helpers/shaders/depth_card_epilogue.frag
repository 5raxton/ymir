void main() {
    vec2 coords_card = ymir_v_coords * ymir_size;

    vec4 color = depth_card_color() * ymir_rounding_alpha(coords_card, ymir_size, ymir_corner_radius);

    color = color * ymir_alpha;

#if defined(DEBUG_FLAGS)
    if (ymir_tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif

    gl_FragColor = color;
}
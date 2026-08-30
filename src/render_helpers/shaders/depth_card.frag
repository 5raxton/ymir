vec4 depth_card_color() {
    // Distance from the near (apex-facing) edge of the card, 0 = near, 1 = far.
    // Bottom deck cards have their near edge at the top, top deck cards at the
    // bottom.
    float v = mix(ymir_v_coords.y, 1.0 - ymir_v_coords.y, ymir_depth_bottom);

    // Fake the depth of the fan: the surface near the apex shows as if seen from
    // up close (wide lens, stretched), while it compresses towards the far edge
    // of the card.
    float tex_v = mix(pow(v, ymir_depth_tilt_pow), 1.0 - pow(v, ymir_depth_tilt_pow), ymir_depth_bottom);

    vec4 color = texture2D(ymir_tex, vec2(ymir_v_coords.x, tex_v));

    // Fade towards the far edge of the card.
    color = color * mix(1.0, ymir_min_opacity, v);

    return color;
}
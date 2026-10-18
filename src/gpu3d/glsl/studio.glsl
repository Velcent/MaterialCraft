// The built-in studio environment (EN-09): soft sky and floor, a key softbox, a side strip, a top panel and a horizon
// line on a dark floor. The softboxes soften and dim with roughness, so a smooth surface mirrors clear shapes and a
// rough one only shows a broad sheen. Everything but the key softbox is baked once into the environment levels in
// the studio's own frame (key towards +Z, so the side strip lies towards -X; the frame turns with the key's
// azimuth); the key softbox follows the key light exactly, so it is evaluated where it is seen.

// Smooth gradient only: the studio's diffuse light.
vec3 studioSoft(vec3 d) {
    vec3 sky = vec3(0.30, 0.34, 0.42);
    vec3 hor = vec3(0.52, 0.52, 0.53);
    vec3 ground = vec3(0.20, 0.19, 0.18);
    return d.y > 0.0 ? mix(hor, sky, pow(d.y, 0.6)) : mix(hor, ground, pow(-d.y, 0.5));
}

// The studio without its key softbox at roughness r, in the studio's frame.
vec3 studioRest(vec3 d, float r) {
    float soft = r * r;
    vec3 col = mix(studioSoft(d), vec3(0.30), soft * 0.5);
    float w = 0.015 + soft * 0.9;
    // tall side strip opposite the key
    vec3 side = vec3(-1.0, 0.0, 0.0);
    float sdot = dot(normalize(vec3(d.x, 0.0, d.z) + 1e-5), side);
    float strip = smoothstep(0.93 - w, 0.97, sdot) * smoothstep(0.02 - w, 0.12, d.y) * (1.0 - smoothstep(0.55, 0.75 + w, d.y));
    col += vec3(0.85, 0.92, 1.0) * strip * 1.5 / (1.0 + soft * 10.0);
    // top panel
    float top = smoothstep(0.90 - w, 0.97, d.y);
    col += vec3(1.0) * top * 1.0 / (1.0 + soft * 10.0);
    // bright horizon line
    float hl = exp(-pow(d.y / (0.035 + soft * 0.4), 2.0));
    col += vec3(0.9) * hl * 0.25 / (1.0 + soft * 8.0);
    return col;
}

// The key softbox seen in direction d at roughness r, for the key light direction key.
vec3 studioKey(vec3 d, float r, vec3 key) {
    float soft = r * r;
    float w = 0.015 + soft * 0.9;
    float keyI = 4.5 / (1.0 + soft * 14.0);
    return vec3(1.0, 0.96, 0.90) * smoothstep(0.93 - w, 0.93 + 0.015, dot(d, key)) * keyI;
}

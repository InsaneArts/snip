#version 440

// A drawn frame (RGBA, `size` px) as the encoder takes it: I420, BT.709 at
// video range. The bytes go out in order, four to a texel of an RGBA target
// `width` texels wide, rows counted from the target's first row in memory on
// any backend (render.cpp reads it back as it is); what's past the end is
// padding. Chroma is the average of its 2 x 2 pixels.
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    ivec2 size;
    int width;
};
layout(binding = 1) uniform sampler2D frame;

const vec3 luma = vec3(0.2126, 0.7152, 0.0722);

float byteAt(int at)
{
    int w = size.x, h = size.y, quarter = (w / 2) * (h / 2);
    if (at < w * h) {
        vec3 c = texelFetch(frame, ivec2(at % w, at / w), 0).rgb;
        return (16.0 + 219.0 * dot(c, luma)) / 255.0;
    }
    if (at >= w * h + 2 * quarter)
        return 0.0;
    int k = (at - w * h) % quarter;
    ivec2 p = 2 * ivec2(k % (w / 2), k / (w / 2));
    vec3 c = (texelFetch(frame, p, 0).rgb + texelFetch(frame, p + ivec2(1, 0), 0).rgb
              + texelFetch(frame, p + ivec2(0, 1), 0).rgb + texelFetch(frame, p + ivec2(1, 1), 0).rgb) / 4.0;
    float y = dot(c, luma);
    float v = at < w * h + quarter ? (c.b - y) / 1.8556 : (c.r - y) / 1.5748;
    return (128.0 + 224.0 * v) / 255.0;
}

void main()
{
    int at = 4 * (int(gl_FragCoord.y) * width + int(gl_FragCoord.x));
    fragColor = vec4(byteAt(at), byteAt(at + 1), byteAt(at + 2), byteAt(at + 3));
}

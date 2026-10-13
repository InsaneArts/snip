#version 440

// A zoom's motion blur (the design's §6.6): each pixel of the card sampled
// back along where that point of the card was a frame before (times the
// motion blur's level, StudioFrame.qml), so a pan blurs along its way and a
// zoom toward or away from its middle. The background stays sharp: it isn't
// in the layer.
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    vec4 now;     // the card now: x, y, w, h, shares of the layer
    vec4 before;  // where its points were, a frame (times the level) before
};

layout(binding = 1) uniform sampler2D source;

void main()
{
    vec2 c = qt_TexCoord0;
    vec2 p = before.xy + (c - now.xy) / now.zw * before.zw;
    vec4 sum = vec4(0.0);
    for (int i = 0; i < 16; i++)
        sum += texture(source, mix(c, p, float(i) / 15.0));
    fragColor = sum / 16.0 * qt_Opacity;
}

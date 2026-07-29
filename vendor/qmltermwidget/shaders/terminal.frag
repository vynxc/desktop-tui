#version 440

layout(location = 0) in vec2 glyphTexCoord;
layout(location = 1) in vec4 glyphColor;

layout(location = 0) out vec4 fragColor;
layout(binding = 1) uniform sampler2D glyphAtlas;

void main()
{
    float alpha = texture(glyphAtlas, glyphTexCoord).a * glyphColor.a;
    fragColor = vec4(glyphColor.rgb * alpha, alpha);
}

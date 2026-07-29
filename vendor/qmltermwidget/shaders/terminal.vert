#version 440

layout(location = 0) in vec2 vertex;
layout(location = 1) in vec2 texCoord;
layout(location = 2) in vec4 color;

layout(location = 0) out vec2 glyphTexCoord;
layout(location = 1) out vec4 glyphColor;

layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
} ubuf;

void main()
{
    glyphTexCoord = texCoord;
    glyphColor = color * ubuf.qt_Opacity;
    gl_Position = ubuf.qt_Matrix * vec4(vertex, 0.0, 1.0);
}

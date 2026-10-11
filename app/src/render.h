#pragma once

#include <QString>

// snipd --render-video SPEC (Studio Mode, D55; the design's §8.2): draw a
// recording's export frame by frame with the scene the video editor shows,
// off screen. Returns the process's exit code.
int renderVideo(const QString& specPath);

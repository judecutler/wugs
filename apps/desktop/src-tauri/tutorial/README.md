# Tutorial recording

`counting-to-six.wav`: an English speaker counting from one to six, 5.5 s, 16 kHz mono PCM16, converted from the original phone recording with ffmpeg (`-ac 1 -ar 16000 -c:a pcm_s16le`).

Recorded by the wugs maintainer (judecutler on GitHub) in his own voice and published by him under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). It is the only real speech in this repository; everything else is synthetic or openly licensed (CLAUDE.md).

`src/sample.rs` embeds it in the app and builds the tutorial project around it: the word times there were measured from this file.

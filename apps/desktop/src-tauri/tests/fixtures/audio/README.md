Synthetic test tones (a 440 Hz sine, made with ffmpeg's `sine` source), used by
`tests/convert.rs`. No recorded speech. Rebuild with:

    ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=16000:duration=0.5" -ac 1 [-c:a aac|libvorbis] tone.<ext>
    ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=22050:duration=0.5" -ac 2 -c:a aac -b:a 48k stereo.m4a

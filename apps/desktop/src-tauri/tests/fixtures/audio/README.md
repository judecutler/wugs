Synthetic test tones (a 440 Hz sine, made with ffmpeg's `sine` source), used by
`tests/convert.rs`. No recorded speech. Rebuild with:

    ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=16000:duration=0.5" -ac 1 [-c:a aac|libvorbis] tone.<ext>
    ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=22050:duration=0.5" -ac 2 -c:a aac -b:a 48k stereo.m4a
    ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=16000:duration=0.5" -ac 1 -c:a alac tone-alac.m4a
    ffmpeg -f lavfi -i "sine=frequency=440:sample_rate=16000:duration=0.5" -ac 1 -c:a pcm_mulaw tone-mulaw.wav
    ffmpeg -f lavfi -i "aevalsrc=0.3*sin(2*PI*220*t)*(between(t\,0.5\,1.3)+between(t\,1.9\,2.9))+0.002*(random(0)-0.5):s=16000:d=3.4" -ac 1 -c:a aac -b:a 48k two-bursts.m4a

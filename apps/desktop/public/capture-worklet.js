// Runs on the audio thread. Batches input into ~100 ms blocks and posts
// them to the page as one Float32Array per channel.
class CaptureProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.channels = options.processorOptions.channels;
    this.blockFrames = Math.round(sampleRate / 10);
    this.reset();
    this.stopped = false;
    this.port.onmessage = (e) => {
      if (e.data === "flush") {
        this.post();
        this.stopped = true;
        this.port.postMessage("flushed");
      }
    };
  }

  reset() {
    this.block = Array.from({ length: this.channels }, () => new Float32Array(this.blockFrames));
    this.filled = 0;
  }

  post() {
    if (this.filled === 0) return;
    const channels = this.block.map((c) => c.slice(0, this.filled));
    this.port.postMessage(channels, channels.map((c) => c.buffer));
    this.reset();
  }

  process(inputs) {
    if (this.stopped) return false;
    const input = inputs[0];
    const frames = input[0] ? input[0].length : 128;
    for (let i = 0; i < frames; i++) {
      for (let c = 0; c < this.channels; c++) this.block[c][this.filled] = input[c] ? input[c][i] : 0;
      this.filled += 1;
      if (this.filled === this.blockFrames) this.post();
    }
    return true;
  }
}

registerProcessor("wugs-capture", CaptureProcessor);

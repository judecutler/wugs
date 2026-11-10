import { captureBegin, captureChunk, captureDiscard, captureFinish, captureKeep, errorText, type SourceRow } from "../api";
import { floatToPcm16, levelOf, micErrorMessage, type Level } from "./pcm.ts";

/** One recording in progress. Nothing here deletes audio except `discard`,
 * which the caller invokes only after the user confirmed. */
export type Recorder = {
  elapsedMs(): number;
  /** Ends the input and drains the upload. Resolves to why the take ended
   * early (input lost, upload failed, size cap), or null for a normal stop.
   * The audio written so far is on disk either way. */
  stop(): Promise<string | null>;
  /** Safe to call again after a failed import: the file stays until one succeeds. */
  finish(label: string): Promise<SourceRow>;
  discard(): Promise<void>;
  /** Stops and leaves the file in `recordings-pending/` for a Recovered recording. */
  keep(): Promise<void>;
};

export type StartOptions = {
  deviceId: string | null;
  onLevel(level: Level): void;
  /** The take ended by itself and is already stopped: the input was lost or an upload failed. */
  onInterrupted(reason: string): void;
};

export const canRecord = (): boolean => navigator.mediaDevices?.getUserMedia !== undefined && typeof AudioWorkletNode !== "undefined";

const FLUSH_TIMEOUT_MS = 2000;

const closeContext = async (ctx: AudioContext) => {
  if (ctx.state !== "closed") await ctx.close().catch(() => {});
};

export async function startRecording(options: StartOptions): Promise<Recorder> {
  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        deviceId: options.deviceId === null ? undefined : { exact: options.deviceId },
        echoCancellation: false,
        noiseSuppression: false,
        autoGainControl: false,
        channelCount: { ideal: 2 },
      },
    });
  } catch (e) {
    throw new Error(micErrorMessage(e instanceof DOMException ? e.name : "UnknownError"));
  }
  const closeStream = () => stream.getTracks().forEach((t) => t.stop());

  let ctx: AudioContext | null = null;
  try {
    const track = stream.getAudioTracks()[0];
    const channels = Math.min(2, track?.getSettings().channelCount ?? 1);
    ctx = new AudioContext();
    const live = ctx;
    await live.audioWorklet.addModule("/capture-worklet.js");
    await live.resume();
    // A context that is not running delivers no audio; recording it would save silence.
    if (live.state !== "running") throw new Error(`The audio system did not start (${live.state}), so nothing was recorded. Try again.`);

    const node = new AudioWorkletNode(live, "wugs-capture", {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      channelCount: channels,
      channelCountMode: "explicit",
      processorOptions: { channels },
    });
    // Silent sink: some engines only run a node that reaches the destination.
    const sink = live.createGain();
    sink.gain.value = 0;
    const source = live.createMediaStreamSource(stream);
    // The file is created only once nothing else in the setup can throw.
    const id = await captureBegin({ sampleRate: live.sampleRate, channels });

    // Uploads run one at a time so chunks reach the file in order.
    let uploads: Promise<void> = Promise.resolve();
    let failed = false;
    let reason: string | null = null;
    let frames = 0;
    const rate = live.sampleRate;
    let flushed!: () => void;
    const allFlushed = new Promise<void>((r) => (flushed = r));
    let stopped: Promise<string | null> | null = null;

    const stop = (): Promise<string | null> =>
      (stopped ??= (async () => {
        closeStream();
        if (!failed) {
          node.port.postMessage("flush");
          await Promise.race([allFlushed, new Promise((r) => setTimeout(r, FLUSH_TIMEOUT_MS))]);
        }
        await closeContext(live);
        await uploads;
        return reason;
      })());

    // The first reason wins. Before Stop it ends the take; during Stop it only explains it.
    const interrupt = (why: string) => {
      reason ??= why;
      if (stopped === null) void stop().then(() => options.onInterrupted(why));
    };
    const lost = (why: string) => {
      if (stopped === null) interrupt(why);
    };
    track?.addEventListener("ended", () => lost("The microphone stopped (unplugged or taken by another app). What was recorded so far is kept."));
    live.addEventListener("statechange", () => {
      if (live.state !== "running") lost("The audio system stopped. What was recorded so far is kept.");
    });

    node.port.onmessage = (e: MessageEvent<Float32Array[] | "flushed">) => {
      const block = e.data;
      if (block === "flushed") return flushed();
      frames += block[0]?.length ?? 0;
      options.onLevel(levelOf(block));
      const pcm = floatToPcm16(block);
      uploads = uploads.then(async () => {
        if (failed) return;
        try {
          await captureChunk(id, pcm);
        } catch (err) {
          failed = true;
          interrupt(`Recording stopped: ${errorText(err)}. What was recorded so far is kept.`);
        }
      });
    };
    source.connect(node);
    node.connect(sink).connect(live.destination);

    return {
      elapsedMs: () => (frames * 1000) / rate,
      stop,
      finish: async (label) => {
        await stop();
        return captureFinish({ captureId: id, label });
      },
      discard: async () => {
        await stop();
        await captureDiscard({ captureId: id });
      },
      keep: async () => {
        await stop();
        await captureKeep({ captureId: id });
      },
    };
  } catch (e) {
    closeStream();
    if (ctx !== null) void closeContext(ctx);
    throw new Error(e instanceof Error ? errorText(e) : String(e));
  }
}

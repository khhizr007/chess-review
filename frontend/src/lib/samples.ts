// Recorded review sounds. A sound pack is a folder `public/sounds/` with one file per sound, named
// after it (`move.mp3`, `result_good.mp3`, ...). No pack is committed (docs/sound.md, THIRD_PARTY.md).
// Every file is fetched and decoded once, at start-up, and kept as an AudioBuffer, so a sound starts
// at once however fast the board is stepped. A sound without a file is synthesised (synth.ts).
import { audioContext, playSynth, type SoundName } from "./synth";

/** Where the sound pack is served from. */
export const SOUNDS_URL = "/sounds/";

const NAMES: Record<SoundName, true> = {
  move: true,
  capture: true,
  castle: true,
  check: true,
  promote: true,
  brilliant: true,
  correct: true,
  incorrect: true,
  result_good: true,
  result_ok: true,
  result_bad: true,
};

const samples = new Map<SoundName, AudioBuffer>();
let loading: Promise<void> | null = null;

async function load(name: SoundName, decoder: BaseAudioContext): Promise<void> {
  const res = await fetch(`${SOUNDS_URL}${name}.mp3`);
  // A dev server answers a missing file with the app's page, so only audio counts.
  if (!res.ok || !res.headers.get("content-type")?.startsWith("audio/")) return;
  samples.set(name, await decoder.decodeAudioData(await res.arrayBuffer()));
}

/** Fetches and decodes the sound pack, once. Missing or unreadable files are left to the synth. */
export function loadSamples(): Promise<void> {
  if (loading) return loading;
  if (typeof window === "undefined" || !window.OfflineAudioContext) return (loading = Promise.resolve());
  // Decoding needs no running context, so the page's own waits for the first sound (a user gesture).
  const decoder = new OfflineAudioContext(1, 1, 44100);
  const names = Object.keys(NAMES) as SoundName[];
  loading = Promise.all(names.map((name) => load(name, decoder).catch(() => undefined))).then(() => undefined);
  return loading;
}

/** Plays one sound now: its recording if the pack has one, else the synthesised sound. */
export function playSample(name: SoundName, volume = 1): void {
  const buffer = samples.get(name);
  if (!buffer) return playSynth(name, volume);
  const ac = audioContext();
  if (!ac) return;
  const gain = ac.createGain();
  gain.gain.value = volume;
  gain.connect(ac.destination);
  const src = ac.createBufferSource();
  src.buffer = buffer;
  src.connect(gain);
  src.start();
}

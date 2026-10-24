// Board and review sounds, synthesised with Web Audio. Nothing is sampled: every sound is built here
// from noise bursts and tones, so the project ships no audio files (see THIRD_PARTY.md). The set and
// the rough shape of each sound follow the reference client's (docs/sound.md). A recorded sound pack
// in `public/sounds/` replaces these where it is present (samples.ts).

export type SoundName =
  | "move"
  | "capture"
  | "castle"
  | "check"
  | "promote"
  | "brilliant"
  | "correct"
  | "incorrect"
  | "result_good"
  | "result_ok"
  | "result_bad";

let ctx: AudioContext | null = null;
let noise: AudioBuffer | null = null;

/**
 * The page's one AudioContext, made on first use and resumed if suspended (browsers start it
 * suspended until the page has had a user gesture). Null where Web Audio is unavailable.
 */
export function audioContext(): AudioContext | null {
  if (!ctx) {
    const Ctor = typeof window === "undefined" ? undefined : window.AudioContext;
    if (!Ctor) return null;
    ctx = new Ctor();
  }
  if (ctx.state === "suspended") void ctx.resume();
  return ctx;
}

function noiseBuffer(ac: AudioContext): AudioBuffer {
  if (noise) return noise;
  const len = Math.round(ac.sampleRate * 0.25);
  noise = ac.createBuffer(1, len, ac.sampleRate);
  const data = noise.getChannelData(0);
  for (let i = 0; i < len; i++) data[i] = Math.random() * 2 - 1;
  return noise;
}

/** Gain that rises in 2 ms to `peak` and decays exponentially over `decay` seconds. */
function envelope(ac: AudioContext, out: AudioNode, t: number, peak: number, decay: number): GainNode {
  const g = ac.createGain();
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(peak, t + 0.002);
  g.gain.exponentialRampToValueAtTime(0.0001, t + decay);
  g.connect(out);
  return g;
}

/** A piece set down on a wooden board: a filtered noise tick over a short low knock. */
function knock(ac: AudioContext, out: AudioNode, t: number, o: { tick: number; body: number; gain: number }) {
  const src = ac.createBufferSource();
  src.buffer = noiseBuffer(ac);
  const band = ac.createBiquadFilter();
  band.type = "bandpass";
  band.frequency.value = o.tick;
  band.Q.value = 1.4;
  src.connect(band).connect(envelope(ac, out, t, o.gain, 0.05));
  src.start(t);
  src.stop(t + 0.06);

  const body = ac.createOscillator();
  body.type = "sine";
  body.frequency.setValueAtTime(o.body, t);
  body.frequency.exponentialRampToValueAtTime(o.body * 0.6, t + 0.08);
  body.connect(envelope(ac, out, t, o.gain * 0.9, 0.09));
  body.start(t);
  body.stop(t + 0.1);
}

/** A bell-like note: a tone with a quieter octave partial. */
function chime(ac: AudioContext, out: AudioNode, t: number, freq: number, gain: number, decay: number, type: OscillatorType = "sine") {
  for (const [mult, level] of [[1, 1], [2, 0.3]] as const) {
    const osc = ac.createOscillator();
    osc.type = type;
    osc.frequency.value = freq * mult;
    osc.connect(envelope(ac, out, t, gain * level, decay));
    osc.start(t);
    osc.stop(t + decay + 0.02);
  }
}

/** Notes one after another, `gap` seconds apart. */
function phrase(ac: AudioContext, out: AudioNode, t: number, notes: number[], gap: number, gain: number, type: OscillatorType = "sine") {
  notes.forEach((f, i) => chime(ac, out, t + i * gap, f, gain, i === notes.length - 1 ? 0.7 : 0.3, type));
}

const NOTE = (semitones: number) => 440 * 2 ** (semitones / 12);

const SOUNDS: Record<SoundName, (ac: AudioContext, out: AudioNode, t: number) => void> = {
  move: (ac, out, t) => knock(ac, out, t, { tick: 1900, body: 230, gain: 0.55 }),
  capture: (ac, out, t) => {
    knock(ac, out, t, { tick: 2800, body: 170, gain: 0.75 });
    knock(ac, out, t + 0.03, { tick: 1500, body: 140, gain: 0.45 });
  },
  castle: (ac, out, t) => {
    knock(ac, out, t, { tick: 1900, body: 230, gain: 0.5 });
    knock(ac, out, t + 0.09, { tick: 1700, body: 210, gain: 0.5 });
  },
  check: (ac, out, t) => knock(ac, out, t, { tick: 3400, body: 260, gain: 0.75 }),
  // A knock with a lower tonal tail.
  promote: (ac, out, t) => {
    knock(ac, out, t, { tick: 1900, body: 230, gain: 0.5 });
    chime(ac, out, t + 0.04, NOTE(-9), 0.12, 0.22, "triangle");
  },
  // A bright shimmer: a fast run of high notes.
  brilliant: (ac, out, t) => {
    [NOTE(19), NOTE(22), NOTE(26), NOTE(29), NOTE(31)].forEach((f, i) => chime(ac, out, t + i * 0.045, f, 0.09, 0.45 + i * 0.08));
  },
  // A quick two-part rising blip.
  correct: (ac, out, t) => phrase(ac, out, t, [NOTE(10), NOTE(17)], 0.08, 0.14),
  // Two notes, slightly falling.
  incorrect: (ac, out, t) => phrase(ac, out, t, [NOTE(3), NOTE(1)], 0.13, 0.12, "triangle"),
  result_good: (ac, out, t) => phrase(ac, out, t, [NOTE(-5), NOTE(-1), NOTE(2)], 0.12, 0.14),
  result_ok: (ac, out, t) => phrase(ac, out, t, [NOTE(-5), NOTE(-1), NOTE(-5)], 0.12, 0.13),
  result_bad: (ac, out, t) => phrase(ac, out, t, [NOTE(-5), NOTE(-6), NOTE(-10)], 0.14, 0.13, "triangle"),
};

/** Plays one sound now. Does nothing where Web Audio is unavailable. */
export function playSynth(name: SoundName, volume = 1): void {
  const ac = audioContext();
  if (!ac) return;
  const master = ac.createGain();
  master.gain.value = volume;
  master.connect(ac.destination);
  SOUNDS[name](ac, master, ac.currentTime + 0.005);
}

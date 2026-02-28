type SoundKey =
  | "hover"
  | "click"
  | "switch"
  | "panel-open"
  | "panel-close"
  | "launch-start"
  | "launch-stop";

let audioContext: AudioContext | null = null;
let masterGain: GainNode | null = null;
const lastPlayAt: Partial<Record<SoundKey, number>> = {};

function getAudioContext(): AudioContext {
  if (!audioContext) {
    const AudioCtor = window.AudioContext || (window as typeof window & {
      webkitAudioContext?: typeof AudioContext;
    }).webkitAudioContext;

    if (!AudioCtor) {
      throw new Error("Web Audio API is not supported in this environment.");
    }

    audioContext = new AudioCtor();
  }

  return audioContext;
}

function getMasterGain(): GainNode {
  const context = getAudioContext();

  if (!masterGain) {
    masterGain = context.createGain();
    masterGain.gain.value = 3;
    masterGain.connect(context.destination);
  }

  return masterGain;
}

async function ensureAudioReady(): Promise<AudioContext | null> {
  try {
    const context = getAudioContext();

    if (context.state !== "running") {
      await context.resume();
    }

    getMasterGain();
    return context;
  } catch {
    return null;
  }
}

function shouldPlay(key: SoundKey, minGapMs: number): boolean {
  const now = performance.now();
  const last = lastPlayAt[key] ?? 0;

  if (now - last < minGapMs) {
    return false;
  }

  lastPlayAt[key] = now;
  return true;
}

function createNoiseLayer(
  context: AudioContext,
  options: {
    duration: number;
    volume: number;
    filterType?: BiquadFilterType;
    fromFrequency: number;
    toFrequency?: number;
    q?: number;
    startTime?: number;
    attack?: number;
    release?: number;
  },
): void {
  const startTime = options.startTime ?? context.currentTime;
  const duration = options.duration;
  const attack = options.attack ?? duration * 0.12;
  const release = options.release ?? duration;
  const frameCount = Math.max(1, Math.floor(context.sampleRate * duration));
  const buffer = context.createBuffer(1, frameCount, context.sampleRate);
  const channel = buffer.getChannelData(0);

  for (let i = 0; i < frameCount; i += 1) {
    const progress = i / frameCount;
    const fade = Math.max(0, 1 - progress);
    channel[i] = (Math.random() * 2 - 1) * fade;
  }

  const source = context.createBufferSource();
  const filter = context.createBiquadFilter();
  const gain = context.createGain();

  source.buffer = buffer;
  filter.type = options.filterType ?? "lowpass";
  filter.Q.value = options.q ?? 0.8;
  filter.frequency.setValueAtTime(Math.max(options.fromFrequency, 30), startTime);

  if (options.toFrequency && options.toFrequency !== options.fromFrequency) {
    filter.frequency.exponentialRampToValueAtTime(
      Math.max(options.toFrequency, 30),
      startTime + duration,
    );
  }

  gain.gain.setValueAtTime(0.0001, startTime);
  gain.gain.exponentialRampToValueAtTime(
    Math.max(options.volume, 0.0001),
    startTime + Math.max(attack, 0.005),
  );
  gain.gain.exponentialRampToValueAtTime(0.0001, startTime + Math.max(release, 0.01));

  source.connect(filter);
  filter.connect(gain);
  gain.connect(getMasterGain());

  source.start(startTime);
  source.stop(startTime + duration);
}

function createMutedBody(
  context: AudioContext,
  options: {
    startFrequency: number;
    endFrequency: number;
    duration: number;
    volume: number;
    startTime?: number;
  },
): void {
  const startTime = options.startTime ?? context.currentTime;
  const oscillator = context.createOscillator();
  const filter = context.createBiquadFilter();
  const gain = context.createGain();

  oscillator.type = "sine";
  oscillator.frequency.setValueAtTime(Math.max(options.startFrequency, 20), startTime);
  oscillator.frequency.exponentialRampToValueAtTime(
    Math.max(options.endFrequency, 20),
    startTime + options.duration,
  );

  filter.type = "lowpass";
  filter.frequency.setValueAtTime(420, startTime);
  filter.Q.value = 0.6;

  gain.gain.setValueAtTime(0.0001, startTime);
  gain.gain.exponentialRampToValueAtTime(
    Math.max(options.volume, 0.0001),
    startTime + options.duration * 0.16,
  );
  gain.gain.exponentialRampToValueAtTime(0.0001, startTime + options.duration);

  oscillator.connect(filter);
  filter.connect(gain);
  gain.connect(getMasterGain());

  oscillator.start(startTime);
  oscillator.stop(startTime + options.duration);
}

function createSoftTick(
  context: AudioContext,
  options: {
    duration: number;
    volume: number;
    color: number;
    startTime?: number;
  },
): void {
  createNoiseLayer(context, {
    duration: options.duration,
    volume: options.volume,
    filterType: "bandpass",
    fromFrequency: options.color,
    toFrequency: options.color * 0.78,
    q: 1.2,
    startTime: options.startTime,
    attack: options.duration * 0.08,
    release: options.duration,
  });
}

function createSoftWhoosh(
  context: AudioContext,
  options: {
    duration: number;
    volume: number;
    fromFrequency: number;
    toFrequency: number;
    startTime?: number;
  },
): void {
  createNoiseLayer(context, {
    duration: options.duration,
    volume: options.volume,
    filterType: "bandpass",
    fromFrequency: options.fromFrequency,
    toFrequency: options.toFrequency,
    q: 0.7,
    startTime: options.startTime,
    attack: options.duration * 0.14,
    release: options.duration,
  });
}

export async function unlockUiSound(): Promise<void> {
  await ensureAudioReady();
}

export async function playHoverSound(): Promise<void> {
  if (!shouldPlay("hover", 70)) {
    return;
  }

  const context = await ensureAudioReady();
  if (!context) {
    return;
  }

  createSoftTick(context, {
    duration: 0.028,
    volume: 0.0036,
    color: 1450,
  });
}

export async function playClickSound(): Promise<void> {
  if (!shouldPlay("click", 45)) {
    return;
  }

  const context = await ensureAudioReady();
  if (!context) {
    return;
  }

  createSoftTick(context, {
    duration: 0.045,
    volume: 0.0062,
    color: 980,
  });
  createMutedBody(context, {
    startFrequency: 170,
    endFrequency: 118,
    duration: 0.06,
    volume: 0.0044,
    startTime: context.currentTime + 0.003,
  });
}

export async function playSwitchSound(): Promise<void> {
  if (!shouldPlay("switch", 85)) {
    return;
  }

  const context = await ensureAudioReady();
  if (!context) {
    return;
  }

  createSoftWhoosh(context, {
    duration: 0.16,
    volume: 0.0075,
    fromFrequency: 780,
    toFrequency: 1480,
  });
  createNoiseLayer(context, {
    duration: 0.09,
    volume: 0.0038,
    filterType: "lowpass",
    fromFrequency: 520,
    toFrequency: 340,
    startTime: context.currentTime + 0.018,
    attack: 0.01,
    release: 0.09,
  });
}

export async function playPanelOpenSound(): Promise<void> {
  if (!shouldPlay("panel-open", 95)) {
    return;
  }

  const context = await ensureAudioReady();
  if (!context) {
    return;
  }

  createSoftWhoosh(context, {
    duration: 0.13,
    volume: 0.006,
    fromFrequency: 620,
    toFrequency: 1120,
  });
  createMutedBody(context, {
    startFrequency: 156,
    endFrequency: 208,
    duration: 0.08,
    volume: 0.0038,
    startTime: context.currentTime + 0.01,
  });
}

export async function playPanelCloseSound(): Promise<void> {
  if (!shouldPlay("panel-close", 95)) {
    return;
  }

  const context = await ensureAudioReady();
  if (!context) {
    return;
  }

  createSoftWhoosh(context, {
    duration: 0.11,
    volume: 0.0054,
    fromFrequency: 980,
    toFrequency: 520,
  });
  createMutedBody(context, {
    startFrequency: 182,
    endFrequency: 124,
    duration: 0.075,
    volume: 0.0034,
    startTime: context.currentTime + 0.004,
  });
}

export async function playLaunchSound(starting: boolean): Promise<void> {
  const key: SoundKey = starting ? "launch-start" : "launch-stop";
  if (!shouldPlay(key, 95)) {
    return;
  }

  const context = await ensureAudioReady();
  if (!context) {
    return;
  }

  if (starting) {
    createSoftWhoosh(context, {
      duration: 0.18,
      volume: 0.0086,
      fromFrequency: 480,
      toFrequency: 1260,
    });
    createMutedBody(context, {
      startFrequency: 132,
      endFrequency: 188,
      duration: 0.1,
      volume: 0.0054,
      startTime: context.currentTime + 0.012,
    });
    createSoftTick(context, {
      duration: 0.032,
      volume: 0.0035,
      color: 840,
      startTime: context.currentTime + 0.052,
    });
    return;
  }

  createNoiseLayer(context, {
    duration: 0.12,
    volume: 0.0052,
    filterType: "lowpass",
    fromFrequency: 640,
    toFrequency: 280,
    attack: 0.008,
    release: 0.12,
  });
  createMutedBody(context, {
    startFrequency: 164,
    endFrequency: 92,
    duration: 0.095,
    volume: 0.0048,
    startTime: context.currentTime + 0.004,
  });
}

export function attachHoverSounds(root: HTMLElement): () => void {
  let lastHoveredElement: Element | null = null;
  const selector = [
    "button",
    "[role='button']",
    "input",
    "select",
    "textarea",
  ].join(", ");

  const handlePointerDown = (): void => {
    void unlockUiSound();
  };

  const handlePointerOver = (event: PointerEvent): void => {
    const target = event.target as Element | null;
    if (!target) {
      return;
    }

    const interactive = target.closest(selector);
    if (!interactive || !root.contains(interactive)) {
      return;
    }

    if (interactive === lastHoveredElement) {
      return;
    }

    const relatedTarget = event.relatedTarget as Node | null;
    if (relatedTarget && interactive.contains(relatedTarget)) {
      return;
    }

    lastHoveredElement = interactive;
    void playHoverSound();
  };

  const handlePointerOut = (event: PointerEvent): void => {
    const target = event.target as Element | null;
    if (!target) {
      return;
    }

    const interactive = target.closest(selector);
    if (!interactive || interactive !== lastHoveredElement) {
      return;
    }

    const relatedTarget = event.relatedTarget as Node | null;
    if (!relatedTarget || !interactive.contains(relatedTarget)) {
      lastHoveredElement = null;
    }
  };

  root.addEventListener("pointerdown", handlePointerDown, true);
  root.addEventListener("pointerover", handlePointerOver);
  root.addEventListener("pointerout", handlePointerOut);

  return () => {
    root.removeEventListener("pointerdown", handlePointerDown, true);
    root.removeEventListener("pointerover", handlePointerOver);
    root.removeEventListener("pointerout", handlePointerOut);
  };
}

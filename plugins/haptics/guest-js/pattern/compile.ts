// Pattern compiler: steps a portable pattern down the five-tier ladder for one device
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	Capabilities,
	CompiledStep,
	EffectRequest,
	HapticsUsage,
	PrimitiveId,
	Tier,
} from '../types';
import type { CompiledSegment } from './playback';
import { placeSteps, playbackEnd } from './playback';
import {
	AMPLITUDE_CEILING,
	MAX_STEPS,
	pickPrimitive,
	primitiveMs,
	resolvePrimitive,
} from './tables';
import type { ContinuousEvent, CurvePoint, Pattern, PatternEvent } from './types';

export type { CompiledSegment };

export type CompileReport = {
	id?: string;
	tier: Tier; // the tier the pattern compiled to
	estimatedMs: number;
	mixed: boolean; // tier 3 with one or more events stepped down to tier 2
	notes: string[]; // human-readable, shown in the lab verbatim
	steps: CompiledStep[];
	request: EffectRequest | null; // the single native request, when one suffices
	segments: CompiledSegment[];
};

export type CompileOptions = {
	tier?: Tier; // compile at most this tier
	maxTier?: Tier | null; // an app-wide cap (setMaxTier); null means no cap
	scale?: number; // effective scale (master × trigger), 0..1; default 1
	usage?: HapticsUsage; // overrides the pattern's usage
	respectSystemSettings?: boolean;
};

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));
const round = (v: number) => Math.round(v);

// Tier 2 and 1 tuning, from the plugin spec.
const SAMPLE_MS = 10;
const SOFT_SAMPLE_MS = 12;
const SOFT_STRENGTH = 0.9;
const MERGE_WITHIN = 8; // amplitude steps (of 255) that are merged into one segment
const MIN_SEGMENT_MS = 6;
const ON_OFF_FLOOR = 40; // quieter than this is dropped at tier 1
const DUTY_PERIOD_MS = 20;
const MIN_ON_MS = 6;

// ── curves ────────────────────────────────────────────────────────────────────────────────────

function curveAt(curve: CurvePoint[], x: number): number {
	const t = clamp01(x);
	for (let k = 1; k < curve.length; k++) {
		if (t <= curve[k].t) {
			const a = curve[k - 1];
			const b = curve[k];
			return a.v + (b.v - a.v) * ((t - a.t) / (b.t - a.t || 1));
		}
	}
	return curve[curve.length - 1].v;
}

function levelAt(level: number | CurvePoint[], x: number): number {
	return typeof level === 'number' ? level : curveAt(level, x);
}

function peak(level: number | CurvePoint[]): number {
	return typeof level === 'number' ? level : Math.max(...level.map((p) => p.v));
}

function mean(level: number | CurvePoint[]): number {
	if (typeof level === 'number') return level;
	let sum = 0;
	for (let i = 0; i <= 10; i++) sum += curveAt(level, i / 10);
	return sum / 11;
}

// ── helpers ───────────────────────────────────────────────────────────────────────────────────

type Segment = { at: number; dur: number; amp: number }; // amp 0..255

function sortEvents(events: PatternEvent[]): PatternEvent[] {
	return events
		.map((ev, i) => ({ ev, i }))
		.sort((a, b) => a.ev.at - b.ev.at || a.i - b.i)
		.map((x) => x.ev);
}

/** Tier-2 segments for one event. */
function amplitudeSegments(
	ev: PatternEvent,
	caps: Capabilities,
	scale: number,
	maxAmp: number
): Segment[] {
	if (ev.type === 'transient') {
		const p = pickPrimitive(ev);
		const amp = Math.min(maxAmp, round(ev.intensity * scale * AMPLITUDE_CEILING[p]));
		if (amp <= 0) return [];
		return [{ at: ev.at, dur: Math.max(MIN_SEGMENT_MS, primitiveMs(caps, p)), amp }];
	}
	const soft = mean(ev.sharpness) < 0.4;
	const step = soft ? SOFT_SAMPLE_MS : SAMPLE_MS;
	const out: Segment[] = [];
	for (let x = 0; x < ev.duration; x += step) {
		const dur = Math.min(step, ev.duration - x);
		const level = levelAt(ev.intensity, (x + dur / 2) / ev.duration);
		const amp = Math.min(maxAmp, round(clamp01(level * scale * (soft ? SOFT_STRENGTH : 1)) * 255));
		const last = out[out.length - 1];
		if (last && Math.abs(last.amp - amp) <= MERGE_WITHIN && last.at + last.dur === ev.at + x) {
			last.dur += dur;
		} else {
			out.push({ at: ev.at + x, dur, amp });
		}
	}
	return out.filter((s) => s.amp > 0 && s.dur >= MIN_SEGMENT_MS);
}

/** Cuts segments to the cap and clamps the amplitude. Returns how many were removed or shortened. */
function applyCaps(
	segs: Segment[],
	maxMs: number,
	maxAmp: number
): { segs: Segment[]; cut: boolean } {
	let cut = false;
	const out: Segment[] = [];
	for (const s of segs) {
		if (s.at >= maxMs) {
			cut = true;
			continue;
		}
		const dur = Math.min(s.dur, maxMs - s.at);
		if (dur < s.dur) cut = true;
		out.push({ at: s.at, dur, amp: Math.min(maxAmp, s.amp) });
	}
	return { segs: out, cut };
}

/**
 * Waveform timings that alternate off/on from `origin`; `amplitudes` is omitted for on/off. Native
 * takes whole milliseconds, so every timing is rounded, and the result is clipped to `maxMs` after
 * overlapping segments have been moved later. `end` is when the waveform stops, on the pattern clock.
 */
function waveformRequest(
	segs: Segment[],
	origin: number,
	withAmplitudes: boolean,
	base: RequestBase,
	maxMs: number
): { request: EffectRequest; end: number; cut: boolean } {
	const timingsMs: number[] = [];
	const amplitudes: number[] = [];
	let cut = false;
	let cursor = Math.round(origin);
	for (const s of segs) {
		const start = Math.max(round(s.at), cursor);
		if (start >= maxMs) {
			cut = true;
			break;
		}
		const wanted = Math.max(1, round(s.dur));
		const dur = Math.min(wanted, maxMs - start);
		if (dur < wanted) cut = true;
		timingsMs.push(start - cursor, dur);
		amplitudes.push(0, s.amp);
		cursor = start + dur;
	}
	const request: EffectRequest = {
		...base,
		effect: withAmplitudes
			? { type: 'waveform', timingsMs, amplitudes, repeat: -1 }
			: { type: 'waveform', timingsMs, repeat: -1 },
	};
	return { request, end: cursor, cut };
}

type RequestBase = Pick<EffectRequest, 'id' | 'usage' | 'respectSystemSettings'>;

// ── tiers ─────────────────────────────────────────────────────────────────────────────────────

/** What a tier compiler emits; the report's segments and duration are derived from the steps. */
type Emitted = Omit<CompileReport, 'id' | 'segments' | 'estimatedMs'>;

type Context = {
	pattern: Pattern;
	caps: Capabilities;
	events: PatternEvent[];
	scale: number;
	maxMs: number;
	maxAmp: number;
	base: RequestBase;
};

function compileEnvelope(cx: Context): Emitted | { fallback: string } {
	const info = cx.caps.envelopeInfo;
	if (!cx.caps.envelopeSupported || !info) {
		return { fallback: 'No envelope support reported; compiled at tier 3' };
	}
	const { minControlPointDurationMs: minPt, maxControlPointDurationMs: maxPt } = info;
	const profile = info.frequencyProfile;
	const centre = cx.caps.resonantHz ?? 150;
	const lo = profile ? profile.minHz : Math.max(1, centre - 40);
	const hi = profile ? profile.maxHz : centre + 40;
	const freqFor = (sharpness: number) => lo + (hi - lo) * clamp01(sharpness);

	const points: { amplitude: number; frequencyHz: number; durationMs: number }[] = [];
	const notes: string[] = [];
	let t = 0;
	let lastAmp = 0;
	let lastFreq = freqFor(cx.events.length ? sharpnessAt(cx.events[0], 0) : 0.5);
	let raised = false;
	let serialised = false;

	const push = (amplitude: number, frequencyHz: number, wanted: number) => {
		// Native takes whole milliseconds.
		const durationMs = Math.max(1, Math.round(wanted));
		let remaining = durationMs;
		const from = lastAmp;
		const total = durationMs;
		while (remaining > 0) {
			let chunk = Math.min(remaining, maxPt);
			if (chunk < minPt) {
				chunk = minPt;
				raised = true;
			}
			remaining -= chunk;
			const a = remaining > 0 ? from + (amplitude - from) * (1 - remaining / total) : amplitude;
			points.push({ amplitude: clamp01(a), frequencyHz, durationMs: chunk });
			t += chunk;
		}
		lastAmp = amplitude;
		lastFreq = frequencyHz;
	};

	const initialFrequencyHz = lastFreq;
	for (const ev of cx.events) {
		const at = round(ev.at);
		if (at > t) push(0, lastFreq, at - t);
		else if (at < t) serialised = true;

		if (ev.type === 'transient') {
			const f = freqFor(ev.sharpness);
			push(clamp01(ev.intensity * cx.scale), f, minPt);
			push(0, f, minPt);
		} else {
			const marks = new Set<number>([0, 1]);
			for (const lvl of [ev.intensity, ev.sharpness]) {
				if (typeof lvl !== 'number') lvl.forEach((p) => marks.add(p.t));
			}
			const ts = [...marks].sort((a, b) => a - b);
			const at = (x: number) => ({
				a: clamp01(levelAt(ev.intensity, x) * cx.scale),
				f: freqFor(levelAt(ev.sharpness, x)),
			});
			// The lead point to the first curve value comes out of the first segment, so the event
			// keeps its own length and the next event is not pushed later.
			const first = at(0);
			push(first.a, first.f, minPt);
			for (let k = 1; k < ts.length; k++) {
				const p = at(ts[k]);
				let wanted = (ts[k] - ts[k - 1]) * ev.duration;
				if (k === 1) wanted = Math.max(minPt, wanted - minPt);
				push(p.a, p.f, wanted);
			}
		}
	}
	if (lastAmp > 0) push(0, lastFreq, minPt);

	// An envelope with no strength anywhere plays nothing, so there is nothing to emit.
	if (points.every((p) => p.amplitude <= 0)) {
		return { tier: 4, mixed: false, notes: [], steps: [], request: null };
	}

	const limit = Math.min(info.maxDurationMs, cx.maxMs);
	if (points.length > info.maxSize) {
		return {
			fallback: `${points.length} control points is over the device limit of ${info.maxSize}; compiled at tier 3`,
		};
	}
	if (t > limit) {
		return { fallback: `${t} ms is over the envelope limit of ${limit} ms; compiled at tier 3` };
	}

	notes.push(`${points.length} control points of ${info.maxSize} allowed`);
	notes.push(`Sharpness maps to ${round(lo)}–${round(hi)} Hz on this actuator`);
	if (raised) notes.push(`Short segments were raised to the ${minPt} ms minimum`);
	if (serialised) notes.push('Overlapping events were played one after another');

	const request: EffectRequest = {
		...cx.base,
		effect: { type: 'envelopeWaveform', initialFrequencyHz, controlPoints: points },
	};
	return {
		tier: 4,
		mixed: false,
		notes,
		steps: [{ atMs: 0, request }],
		request,
	};
}

function sharpnessAt(ev: PatternEvent, x: number): number {
	return ev.type === 'transient' ? ev.sharpness : levelAt(ev.sharpness, x);
}

function compilePrimitives(cx: Context): Emitted {
	const notes: string[] = [];
	type Item =
		| { kind: 'primitive'; ev: PatternEvent; id: PrimitiveId; scale: number }
		| { kind: 'amplitude'; ev: PatternEvent; segs: Segment[] };
	const items: Item[] = [];
	let cut = false;
	// Mirrors the native composition builder, which drops a primitive that would end past the limit.
	let primitiveEnd = 0;
	let primitiveTotal = 0;

	for (const ev of cx.events) {
		if (ev.at >= cx.maxMs) {
			cut = true;
			continue;
		}
		const wanted = pickPrimitive(ev);
		const resolved = resolvePrimitive(cx.caps, wanted);
		if (resolved) {
			const level =
				ev.type === 'transient' ? ev.intensity : peak((ev as ContinuousEvent).intensity);
			const scale = clamp01(level * cx.scale);
			// A primitive with no strength plays nothing, so it is left out instead of sent.
			if (scale === 0) continue;
			const dur = primitiveMs(cx.caps, resolved.id);
			const delayMs = Math.max(0, round(ev.at - primitiveEnd));
			if (primitiveTotal + delayMs + dur > cx.maxMs) {
				cut = true;
				continue;
			}
			primitiveTotal += delayMs + dur;
			primitiveEnd = Math.max(primitiveEnd, ev.at) + dur;
			items.push({ kind: 'primitive', ev, id: resolved.id, scale });
			if (resolved.note) notes.push(resolved.note);
		} else {
			const capped = applyCaps(
				amplitudeSegments(ev, cx.caps, cx.scale, cx.maxAmp),
				cx.maxMs,
				cx.maxAmp
			);
			cut = cut || capped.cut;
			items.push({ kind: 'amplitude', ev, segs: capped.segs });
			notes.push(
				`No ${wanted} or neighbour; that event drops to tier ${cx.caps.hasAmplitudeControl ? 2 : 1}`
			);
		}
	}

	const mixed = items.some((i) => i.kind === 'amplitude');
	const primitiveItems = items.filter((i) => i.kind === 'primitive');

	if (!primitiveItems.length) {
		// Nothing needed a primitive the motor has, so the whole pattern is tier 2.
		const segs = items.flatMap((i) => (i.kind === 'amplitude' ? i.segs : []));
		// Without amplitude control the fallback is on/off, which is what the device plays.
		const fallback = cx.caps.hasAmplitudeControl
			? compileAmplitude({ ...cx, events: [] }, segs)
			: compileOnOff({ ...cx, events: [] }, segs);
		if (cut || fallback.cut) notes.push(`Truncated to ${cx.maxMs} ms`);
		return { ...withoutCut(fallback), notes };
	}

	const steps: CompiledStep[] = [];
	let request: EffectRequest | null = null;
	let stepsCut = false;
	if (!mixed) {
		let end = 0;
		const comp = primitiveItems.map((item) => {
			const dur = primitiveMs(cx.caps, item.id);
			const delayMs = Math.max(0, round(item.ev.at - end));
			end = Math.max(end, item.ev.at) + dur;
			return { kind: 'primitive' as const, primitive: item.id, scale: item.scale, delayMs };
		});
		request = { ...cx.base, effect: { type: 'composition', steps: comp } };
		steps.push({ atMs: 0, request });
	} else {
		// Each step cancels the one before it, so steps start after the previous one ends, and one
		// that would run past the cap once placed is dropped.
		let end = 0;
		for (const item of items) {
			if (steps.length >= MAX_STEPS) {
				stepsCut = true;
				break;
			}
			if (item.kind === 'primitive') {
				const start = Math.max(round(item.ev.at), end);
				const dur = primitiveMs(cx.caps, item.id);
				if (start + dur > cx.maxMs) {
					cut = true;
					continue;
				}
				steps.push({
					atMs: start,
					request: {
						...cx.base,
						effect: {
							type: 'composition',
							steps: [{ kind: 'primitive', primitive: item.id, scale: item.scale, delayMs: 0 }],
						},
					},
				});
				end = start + dur;
			} else if (item.segs.length) {
				// Without amplitude control the device plays these at one strength, so emit on/off pulses.
				const plan = cx.caps.hasAmplitudeControl ? item.segs : dutyCycle(item.segs).on;
				if (!plan.length) continue;
				const origin = Math.max(round(plan[0].at), end);
				const built = waveformRequest(plan, origin, cx.caps.hasAmplitudeControl, cx.base, cx.maxMs);
				cut = cut || built.cut;
				if (built.end <= origin) continue;
				steps.push({ atMs: origin, request: built.request });
				end = built.end;
			}
		}
	}

	// Dropping steps can leave only one kind, so the tier and the mixed flag follow what is emitted.
	const kinds = new Set(steps.map((s) => s.request.effect.type));
	const stillMixed = kinds.has('composition') && kinds.has('waveform');
	const tier: Tier = kinds.has('composition') ? 3 : cx.caps.hasAmplitudeControl ? 2 : 1;

	if (cut) notes.push(`Truncated to ${cx.maxMs} ms`);
	if (stillMixed) notes.push('Mixed: runs as a scheduled step list');
	if (stepsCut) notes.push(`Truncated to ${MAX_STEPS} steps`);

	return { tier, mixed: stillMixed, notes, steps, request };
}

function compileAmplitude(cx: Context, given?: Segment[]): Emitted & { cut: boolean } {
	const notes: string[] = [];
	const raw =
		given ?? cx.events.flatMap((ev) => amplitudeSegments(ev, cx.caps, cx.scale, cx.maxAmp));
	const capped = applyCaps(
		raw.sort((a, b) => a.at - b.at),
		cx.maxMs,
		cx.maxAmp
	);
	const { segs } = capped;

	const built = segs.length ? waveformRequest(segs, 0, true, cx.base, cx.maxMs) : null;
	const cut = capped.cut || (built?.cut ?? false);
	if (cut) notes.push(`Truncated to ${cx.maxMs} ms`);
	notes.push(`${segs.length} one-shot segments, neighbours within ${MERGE_WITHIN} merged`);

	const request = built?.request ?? null;
	return {
		tier: 2,
		mixed: false,
		notes,
		steps: request ? [{ atMs: 0, request }] : [],
		request,
		cut,
	};
}

/** Turns amplitude segments into full-strength pulses, dropping the ones too quiet to feel. */
function dutyCycle(source: Segment[]): { on: Segment[]; dropped: number } {
	const on: Segment[] = [];
	let dropped = 0;
	for (const s of source) {
		if (s.amp < ON_OFF_FLOOR) {
			dropped++;
			continue;
		}
		const onTime = Math.max(MIN_ON_MS, (s.amp / 255) * DUTY_PERIOD_MS);
		for (let x = 0; x < s.dur; x += DUTY_PERIOD_MS) {
			on.push({ at: s.at + x, dur: Math.min(s.dur - x, Math.max(MIN_ON_MS, onTime)), amp: 255 });
		}
	}
	return { on, dropped };
}

function compileOnOff(cx: Context, given?: Segment[]): Emitted & { cut: boolean } {
	const notes: string[] = [];
	const quiet = compileAmplitude(cx, given);
	// The amplitude plan as it plays, so the duty cycle follows the serialised segments.
	const { on, dropped } = dutyCycle(
		placeSteps(quiet.steps, cx.caps).map((s) => ({
			at: s.atMs,
			dur: s.durationMs,
			amp: round(s.amplitude * 255),
		}))
	);
	notes.push(`Duty-cycled on a ${DUTY_PERIOD_MS} ms period, minimum on-time ${MIN_ON_MS} ms`);
	if (dropped) {
		notes.push(
			`${dropped} quiet segment${dropped > 1 ? 's' : ''} under amplitude ${ON_OFF_FLOOR} dropped`
		);
	}

	const built = on.length ? waveformRequest(on, 0, false, cx.base, cx.maxMs) : null;
	const cut = quiet.cut || (built?.cut ?? false);
	if (cut) notes.push(`Truncated to ${cx.maxMs} ms`);

	const request = built?.request ?? null;
	return {
		tier: 1,
		mixed: false,
		notes,
		steps: request ? [{ atMs: 0, request }] : [],
		request,
		cut,
	};
}

// ── entry point ───────────────────────────────────────────────────────────────────────────────

/**
 * Compiles `pattern` for the device described by `caps`. Pure and deterministic: the same inputs
 * always give the same report, so the lab previews exactly what the runtime plays. The target tier
 * is the lowest of the device's top tier, `opts.tier` and `opts.maxTier`.
 */
export function compilePattern(
	pattern: Pattern,
	caps: Capabilities,
	opts: CompileOptions = {}
): CompileReport {
	const target = Math.min(caps.topTier, opts.tier ?? 4, opts.maxTier ?? 4) as Tier;

	if (target === 0) {
		const why = caps.topTier === 0 ? 'No vibrator on this device. ' : '';
		return {
			id: pattern.id,
			tier: 0,
			estimatedMs: 0,
			mixed: false,
			notes: [`${why}Nothing plays. Resolves ok at tier 0.`],
			steps: [],
			request: null,
			segments: [],
		};
	}

	const base: RequestBase = {
		...(pattern.id !== undefined ? { id: pattern.id } : {}),
		usage: opts.usage ?? pattern.usage ?? 'media',
		...(opts.respectSystemSettings !== undefined
			? { respectSystemSettings: opts.respectSystemSettings }
			: {}),
	};
	const cx: Context = {
		pattern,
		caps,
		events: sortEvents(pattern.events),
		scale: clamp01(Number.isFinite(opts.scale) ? (opts.scale as number) : 1),
		maxMs: caps.limits.maxDurationMs,
		maxAmp: Math.min(255, caps.limits.maxAmplitude),
		base,
	};

	const carried: string[] = [];
	let result: Emitted | null = null;
	if (target === 4) {
		const env = compileEnvelope(cx);
		if ('fallback' in env) carried.push(env.fallback);
		else result = env;
	}
	if (!result) {
		if (target >= 3) result = compilePrimitives(cx);
		else if (target === 2 && caps.hasAmplitudeControl) result = withoutCut(compileAmplitude(cx));
		else result = withoutCut(compileOnOff(cx));
	}

	// The report is read off the steps, so it can only say what the steps play.
	const segments = placeSteps(result.steps, caps);
	return {
		id: pattern.id,
		...result,
		notes: [...carried, ...result.notes],
		estimatedMs: playbackEnd(segments),
		segments,
	};
}

function withoutCut({ cut: _cut, ...emitted }: Emitted & { cut: boolean }): Emitted {
	return emitted;
}

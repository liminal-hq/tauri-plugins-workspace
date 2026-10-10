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
import { AMPLITUDE_CEILING, pickPrimitive, primitiveMs, resolvePrimitive } from './tables';
import type { ContinuousEvent, CurvePoint, Pattern, PatternEvent } from './types';

/** One bar of the compiled pattern, for previews: when, how long and how strong (0..1). */
export type CompiledSegment = {
	atMs: number;
	durationMs: number;
	amplitude: number;
	tier: Tier; // the tier this segment was compiled at (2 inside a mixed tier-3 pattern)
	label?: string; // the primitive, at tier 3
};

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
): { request: EffectRequest; end: number; placed: Segment[] } {
	const timingsMs: number[] = [];
	const amplitudes: number[] = [];
	const placed: Segment[] = [];
	let cursor = Math.round(origin);
	for (const s of segs) {
		const start = Math.max(round(s.at), cursor);
		if (start >= maxMs) break;
		const dur = Math.min(Math.max(1, round(s.dur)), maxMs - start);
		timingsMs.push(start - cursor, dur);
		amplitudes.push(0, s.amp);
		placed.push({ at: start, dur, amp: s.amp });
		cursor = start + dur;
	}
	const request: EffectRequest = {
		...base,
		effect: withAmplitudes
			? { type: 'waveform', timingsMs, amplitudes, repeat: -1 }
			: { type: 'waveform', timingsMs, repeat: -1 },
	};
	return { request, end: cursor, placed };
}

type RequestBase = Pick<EffectRequest, 'id' | 'usage' | 'respectSystemSettings'>;

function toSegmentReport(segs: Segment[], tier: Tier): CompiledSegment[] {
	return segs.map((s) => ({
		atMs: s.at,
		durationMs: s.dur,
		amplitude: s.amp / 255,
		tier,
	}));
}

function endMs(segments: CompiledSegment[]): number {
	return round(segments.reduce((m, s) => Math.max(m, s.atMs + s.durationMs), 0));
}

// ── tiers ─────────────────────────────────────────────────────────────────────────────────────

type Attempt = Omit<CompileReport, 'id'>;

type Context = {
	pattern: Pattern;
	caps: Capabilities;
	events: PatternEvent[];
	scale: number;
	maxMs: number;
	maxAmp: number;
	base: RequestBase;
};

function compileEnvelope(cx: Context): Attempt | { fallback: string } {
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
	const bars: CompiledSegment[] = [];
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
			bars.push({ atMs: t, durationMs: chunk, amplitude: clamp01(a), tier: 4 });
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
		estimatedMs: round(t),
		mixed: false,
		notes,
		steps: [{ atMs: 0, request }],
		request,
		segments: bars,
	};
}

function sharpnessAt(ev: PatternEvent, x: number): number {
	return ev.type === 'transient' ? ev.sharpness : levelAt(ev.sharpness, x);
}

function compilePrimitives(cx: Context): Attempt {
	const notes: string[] = [];
	const segments: CompiledSegment[] = [];
	type Item =
		| { kind: 'primitive'; ev: PatternEvent; id: PrimitiveId; scale: number; dropped?: boolean }
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
			const dur = primitiveMs(cx.caps, resolved.id);
			const delayMs = Math.max(0, round(ev.at - primitiveEnd));
			if (primitiveTotal + delayMs + dur > cx.maxMs) {
				cut = true;
				continue;
			}
			primitiveTotal += delayMs + dur;
			primitiveEnd = Math.max(primitiveEnd, ev.at) + dur;
			const level =
				ev.type === 'transient' ? ev.intensity : peak((ev as ContinuousEvent).intensity);
			items.push({ kind: 'primitive', ev, id: resolved.id, scale: clamp01(level * cx.scale) });
			if (resolved.note) notes.push(resolved.note);
		} else {
			const capped = applyCaps(
				amplitudeSegments(ev, cx.caps, cx.scale, cx.maxAmp),
				cx.maxMs,
				cx.maxAmp
			);
			cut = cut || capped.cut;
			items.push({ kind: 'amplitude', ev, segs: capped.segs });
			notes.push(`No ${wanted} or neighbour; that event drops to tier 2`);
		}
	}

	const mixed = items.some((i) => i.kind === 'amplitude');
	const primitiveItems = items.filter((i) => i.kind === 'primitive');

	// Bars: primitives on the beat, plus any tier-2 segments. A mixed pattern plays one step after
	// another, so a step that would run past the cap once serialised is dropped here.
	let cursor = 0;
	for (const item of items) {
		if (item.kind === 'primitive') {
			const start = Math.max(mixed ? round(item.ev.at) : item.ev.at, cursor);
			const dur = primitiveMs(cx.caps, item.id);
			if (mixed && start + dur > cx.maxMs) {
				item.dropped = true;
				cut = true;
				continue;
			}
			segments.push({
				atMs: start,
				durationMs: dur,
				amplitude: item.scale,
				tier: 3,
				label: item.id,
			});
			cursor = start + dur;
		} else if (item.segs.length) {
			// Playback starts a segment list only once the step before it has ended, and clips it to the cap.
			const shift = Math.max(round(item.segs[0].at), cursor) - item.segs[0].at;
			const placed: Segment[] = [];
			for (const seg of item.segs) {
				const at = seg.at + shift;
				if (at >= cx.maxMs) {
					cut = true;
					break;
				}
				const dur = Math.min(seg.dur, cx.maxMs - at);
				if (dur < seg.dur) cut = true;
				placed.push({ ...seg, at, dur });
			}
			segments.push(...toSegmentReport(placed, 2));
			cursor = Math.max(cursor, ...placed.map((seg) => seg.at + seg.dur));
		}
	}
	if (cut) notes.push(`Truncated to ${cx.maxMs} ms`);

	if (!primitiveItems.length) {
		// Nothing needed a primitive the motor has, so the whole pattern is tier 2.
		const segs = items.flatMap((i) => (i.kind === 'amplitude' ? i.segs : []));
		return { ...compileAmplitude({ ...cx, events: [] }, segs), notes, tier: 2 };
	}

	const steps: CompiledStep[] = [];
	let request: EffectRequest | null = null;
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
		notes.push('Mixed: runs as a scheduled step list');
		// Each step cancels the one before it, so steps start after the previous one ends.
		let end = 0;
		for (const item of items) {
			if (item.kind === 'primitive') {
				if (item.dropped) continue;
				const start = Math.max(round(item.ev.at), end);
				if (start >= cx.maxMs) continue;
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
				end = start + primitiveMs(cx.caps, item.id);
			} else if (item.segs.length) {
				const origin = Math.max(round(item.segs[0].at), end);
				const built = waveformRequest(item.segs, origin, true, cx.base, cx.maxMs);
				if (built.end <= origin) continue;
				steps.push({ atMs: origin, request: built.request });
				end = built.end;
			}
		}
	}

	return {
		tier: 3,
		estimatedMs: endMs(segments),
		mixed,
		notes,
		steps,
		request,
		segments,
	};
}

function compileAmplitude(cx: Context, given?: Segment[]): Attempt {
	const notes: string[] = [];
	const raw =
		given ?? cx.events.flatMap((ev) => amplitudeSegments(ev, cx.caps, cx.scale, cx.maxAmp));
	const { segs, cut } = applyCaps(
		raw.sort((a, b) => a.at - b.at),
		cx.maxMs,
		cx.maxAmp
	);
	if (cut) notes.push(`Truncated to ${cx.maxMs} ms`);
	notes.push(`${segs.length} one-shot segments, neighbours within ${MERGE_WITHIN} merged`);

	// The report follows what plays: overlapping segments are moved behind the ones before them.
	const built = segs.length ? waveformRequest(segs, 0, true, cx.base, cx.maxMs) : null;
	const request = built?.request ?? null;
	const segments = toSegmentReport(built?.placed ?? [], 2);
	return {
		tier: 2,
		estimatedMs: endMs(segments),
		mixed: false,
		notes,
		steps: request ? [{ atMs: 0, request }] : [],
		request,
		segments,
	};
}

function compileOnOff(cx: Context): Attempt {
	const notes: string[] = [];
	const quiet = compileAmplitude(cx);
	const source = quiet.segments.map((s) => ({
		at: s.atMs,
		dur: s.durationMs,
		amp: round(s.amplitude * 255),
	}));

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
	notes.push(`Duty-cycled on a ${DUTY_PERIOD_MS} ms period, minimum on-time ${MIN_ON_MS} ms`);
	if (dropped) {
		notes.push(
			`${dropped} quiet segment${dropped > 1 ? 's' : ''} under amplitude ${ON_OFF_FLOOR} dropped`
		);
	}
	notes.push(...quiet.notes.filter((n) => n.startsWith('Truncated')));

	const request = on.length ? waveformRequest(on, 0, false, cx.base, cx.maxMs).request : null;
	const segments = toSegmentReport(on, 1);
	return {
		tier: 1,
		estimatedMs: endMs(segments),
		mixed: false,
		notes,
		steps: request ? [{ atMs: 0, request }] : [],
		request,
		segments,
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
	let result: Attempt | null = null;
	if (target === 4) {
		const env = compileEnvelope(cx);
		if ('fallback' in env) carried.push(env.fallback);
		else result = env;
	}
	if (!result) {
		if (target >= 3) result = compilePrimitives(cx);
		else if (target === 2) result = compileAmplitude(cx);
		else result = compileOnOff(cx);
	}

	return { id: pattern.id, ...result, notes: [...carried, ...result.notes] };
}

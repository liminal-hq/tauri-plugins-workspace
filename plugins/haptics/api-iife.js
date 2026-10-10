if ('__TAURI__' in window) {
var __TAURI_PLUGIN_HAPTICS__ = (function (exports, core) {
    'use strict';

    // Primitive tables shared by the compiler: durations, amplitude ceilings and neighbours
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PRIMITIVE_IDS = [
        'tick',
        'low_tick',
        'click',
        'thud',
        'spin',
        'quick_rise',
        'slow_rise',
    ];
    /** Built-in primitive durations in ms, used when the motor does not report its own. */
    const PRIMITIVE_MS = {
        tick: 10,
        low_tick: 12,
        click: 15,
        thud: 30,
        quick_rise: 60,
        slow_rise: 150,
        spin: 90,
    };
    /** The amplitude (0..255) a full-strength primitive stands in for at tier 2. */
    const AMPLITUDE_CEILING = {
        tick: 140,
        low_tick: 120,
        click: 200,
        thud: 255,
        quick_rise: 220,
        slow_rise: 220,
        spin: 200,
    };
    /** Nearest stand-ins, tried in order, when a motor lacks a primitive. */
    const NEIGHBOURS = {
        low_tick: ['tick', 'click'],
        tick: ['click'],
        thud: ['click'],
        spin: ['quick_rise'],
        slow_rise: ['quick_rise'],
        quick_rise: [],
        click: [],
    };
    /** The measured duration of a primitive on this motor, or the built-in value. */
    function primitiveMs(caps, id) {
        return caps.primitives[id]?.durationMs ?? PRIMITIVE_MS[id];
    }
    function isPrimitiveSupported(caps, id) {
        return caps.primitives[id]?.supported === true;
    }
    /**
     * The primitive a supported one stands in for: `id` itself, else its first supported neighbour.
     * Returns `null` when neither exists.
     */
    function resolvePrimitive(caps, id) {
        if (isPrimitiveSupported(caps, id))
            return { id };
        for (const next of NEIGHBOURS[id]) {
            if (isPrimitiveSupported(caps, next)) {
                return { id: next, note: `${id} missing on this motor → ${next}` };
            }
        }
        return null;
    }
    /** The first and last value of an event's intensity, whether it is flat or a curve. */
    function intensityEnds(ev) {
        if (typeof ev.intensity === 'number')
            return [ev.intensity, ev.intensity];
        return [ev.intensity[0].v, ev.intensity[ev.intensity.length - 1].v];
    }
    /** Which primitive an event would use, before any neighbour substitution. */
    function pickPrimitive(ev) {
        if (ev.type === 'transient') {
            if (ev.sharpness >= 0.6)
                return ev.intensity < 0.4 ? 'tick' : 'click';
            if (ev.sharpness < 0.4)
                return ev.intensity < 0.4 ? 'low_tick' : 'thud';
            return 'click';
        }
        const [first, last] = intensityEnds(ev);
        if (last > first + 0.05)
            return ev.duration < 150 ? 'quick_rise' : 'slow_rise';
        return 'spin';
    }

    // Pattern compiler: steps a portable pattern down the five-tier ladder for one device
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const clamp01$1 = (v) => Math.min(1, Math.max(0, v));
    const round = (v) => Math.round(v);
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
    function curveAt(curve, x) {
        const t = clamp01$1(x);
        for (let k = 1; k < curve.length; k++) {
            if (t <= curve[k].t) {
                const a = curve[k - 1];
                const b = curve[k];
                return a.v + (b.v - a.v) * ((t - a.t) / (b.t - a.t || 1));
            }
        }
        return curve[curve.length - 1].v;
    }
    function levelAt(level, x) {
        return typeof level === 'number' ? level : curveAt(level, x);
    }
    function peak(level) {
        return typeof level === 'number' ? level : Math.max(...level.map((p) => p.v));
    }
    function mean(level) {
        if (typeof level === 'number')
            return level;
        let sum = 0;
        for (let i = 0; i <= 10; i++)
            sum += curveAt(level, i / 10);
        return sum / 11;
    }
    function sortEvents(events) {
        return events
            .map((ev, i) => ({ ev, i }))
            .sort((a, b) => a.ev.at - b.ev.at || a.i - b.i)
            .map((x) => x.ev);
    }
    /** Tier-2 segments for one event. */
    function amplitudeSegments(ev, caps, scale, maxAmp) {
        if (ev.type === 'transient') {
            const p = pickPrimitive(ev);
            const amp = Math.min(maxAmp, round(ev.intensity * scale * AMPLITUDE_CEILING[p]));
            return [{ at: ev.at, dur: Math.max(MIN_SEGMENT_MS, primitiveMs(caps, p)), amp }];
        }
        const soft = mean(ev.sharpness) < 0.4;
        const step = soft ? SOFT_SAMPLE_MS : SAMPLE_MS;
        const out = [];
        for (let x = 0; x < ev.duration; x += step) {
            const dur = Math.min(step, ev.duration - x);
            const level = levelAt(ev.intensity, (x + dur / 2) / ev.duration);
            const amp = Math.min(maxAmp, round(clamp01$1(level * scale * (soft ? SOFT_STRENGTH : 1)) * 255));
            const last = out[out.length - 1];
            if (last && Math.abs(last.amp - amp) <= MERGE_WITHIN && last.at + last.dur === ev.at + x) {
                last.dur += dur;
            }
            else {
                out.push({ at: ev.at + x, dur, amp });
            }
        }
        return out.filter((s) => s.amp > 0 && s.dur >= MIN_SEGMENT_MS);
    }
    /** Cuts segments to the cap and clamps the amplitude. Returns how many were removed or shortened. */
    function applyCaps(segs, maxMs, maxAmp) {
        let cut = false;
        const out = [];
        for (const s of segs) {
            if (s.at >= maxMs) {
                cut = true;
                continue;
            }
            const dur = Math.min(s.dur, maxMs - s.at);
            if (dur < s.dur)
                cut = true;
            out.push({ at: s.at, dur, amp: Math.min(maxAmp, s.amp) });
        }
        return { segs: out, cut };
    }
    /**
     * Waveform timings that alternate off/on from `origin`; `amplitudes` is omitted for on/off. Native
     * takes whole milliseconds, so every timing is rounded, and the result is clipped to `maxMs` after
     * overlapping segments have been moved later. `end` is when the waveform stops, on the pattern clock.
     */
    function waveformRequest(segs, origin, withAmplitudes, base, maxMs) {
        const timingsMs = [];
        const amplitudes = [];
        let cursor = Math.round(origin);
        for (const s of segs) {
            const start = Math.max(round(s.at), cursor);
            if (start >= maxMs)
                break;
            const dur = Math.min(Math.max(1, round(s.dur)), maxMs - start);
            timingsMs.push(start - cursor, dur);
            amplitudes.push(0, s.amp);
            cursor = start + dur;
        }
        const request = {
            ...base,
            effect: withAmplitudes
                ? { type: 'waveform', timingsMs, amplitudes, repeat: -1 }
                : { type: 'waveform', timingsMs, repeat: -1 },
        };
        return { request, end: cursor };
    }
    function toSegmentReport(segs, tier) {
        return segs.map((s) => ({
            atMs: s.at,
            durationMs: s.dur,
            amplitude: s.amp / 255,
            tier,
        }));
    }
    function endMs(segments) {
        return round(segments.reduce((m, s) => Math.max(m, s.atMs + s.durationMs), 0));
    }
    function compileEnvelope(cx) {
        const info = cx.caps.envelopeInfo;
        if (!cx.caps.envelopeSupported || !info) {
            return { fallback: 'No envelope support reported; compiled at tier 3' };
        }
        const { minControlPointDurationMs: minPt, maxControlPointDurationMs: maxPt } = info;
        const profile = info.frequencyProfile;
        const centre = cx.caps.resonantHz ?? 150;
        const lo = profile ? profile.minHz : Math.max(1, centre - 40);
        const hi = profile ? profile.maxHz : centre + 40;
        const freqFor = (sharpness) => lo + (hi - lo) * clamp01$1(sharpness);
        const points = [];
        const bars = [];
        const notes = [];
        let t = 0;
        let lastAmp = 0;
        let lastFreq = freqFor(cx.events.length ? sharpnessAt(cx.events[0], 0) : 0.5);
        let raised = false;
        let serialised = false;
        const push = (amplitude, frequencyHz, wanted) => {
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
                points.push({ amplitude: clamp01$1(a), frequencyHz, durationMs: chunk });
                bars.push({ atMs: t, durationMs: chunk, amplitude: clamp01$1(a), tier: 4 });
                t += chunk;
            }
            lastAmp = amplitude;
            lastFreq = frequencyHz;
        };
        const initialFrequencyHz = lastFreq;
        for (const ev of cx.events) {
            const at = round(ev.at);
            if (at > t)
                push(0, lastFreq, at - t);
            else if (at < t)
                serialised = true;
            if (ev.type === 'transient') {
                const f = freqFor(ev.sharpness);
                push(clamp01$1(ev.intensity * cx.scale), f, minPt);
                push(0, f, minPt);
            }
            else {
                const marks = new Set([0, 1]);
                for (const lvl of [ev.intensity, ev.sharpness]) {
                    if (typeof lvl !== 'number')
                        lvl.forEach((p) => marks.add(p.t));
                }
                const ts = [...marks].sort((a, b) => a - b);
                const at = (x) => ({
                    a: clamp01$1(levelAt(ev.intensity, x) * cx.scale),
                    f: freqFor(levelAt(ev.sharpness, x)),
                });
                // The lead point to the first curve value comes out of the first segment, so the event
                // keeps its own length and the next event is not pushed later.
                const first = at(0);
                push(first.a, first.f, minPt);
                for (let k = 1; k < ts.length; k++) {
                    const p = at(ts[k]);
                    let wanted = (ts[k] - ts[k - 1]) * ev.duration;
                    if (k === 1)
                        wanted = Math.max(minPt, wanted - minPt);
                    push(p.a, p.f, wanted);
                }
            }
        }
        if (lastAmp > 0)
            push(0, lastFreq, minPt);
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
        if (raised)
            notes.push(`Short segments were raised to the ${minPt} ms minimum`);
        if (serialised)
            notes.push('Overlapping events were played one after another');
        const request = {
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
    function sharpnessAt(ev, x) {
        return ev.type === 'transient' ? ev.sharpness : levelAt(ev.sharpness, x);
    }
    function compilePrimitives(cx) {
        const notes = [];
        const segments = [];
        const items = [];
        let cut = false;
        for (const ev of cx.events) {
            if (ev.at >= cx.maxMs) {
                cut = true;
                continue;
            }
            const wanted = pickPrimitive(ev);
            const resolved = resolvePrimitive(cx.caps, wanted);
            if (resolved) {
                const level = ev.type === 'transient' ? ev.intensity : peak(ev.intensity);
                items.push({ kind: 'primitive', ev, id: resolved.id, scale: clamp01$1(level * cx.scale) });
                if (resolved.note)
                    notes.push(resolved.note);
            }
            else {
                const capped = applyCaps(amplitudeSegments(ev, cx.caps, cx.scale, cx.maxAmp), cx.maxMs, cx.maxAmp);
                cut = cut || capped.cut;
                items.push({ kind: 'amplitude', ev, segs: capped.segs });
                notes.push(`No ${wanted} or neighbour; that event drops to tier 2`);
            }
        }
        const mixed = items.some((i) => i.kind === 'amplitude');
        const primitiveItems = items.filter((i) => i.kind === 'primitive');
        if (cut)
            notes.push(`Truncated to ${cx.maxMs} ms`);
        // Bars: primitives on the beat, plus any tier-2 segments.
        let cursor = 0;
        for (const item of items) {
            if (item.kind === 'primitive') {
                const start = Math.max(item.ev.at, cursor);
                const dur = primitiveMs(cx.caps, item.id);
                segments.push({
                    atMs: start,
                    durationMs: dur,
                    amplitude: item.scale,
                    tier: 3,
                    label: item.id,
                });
                cursor = start + dur;
            }
            else {
                segments.push(...toSegmentReport(item.segs, 2));
            }
        }
        if (!primitiveItems.length) {
            // Nothing needed a primitive the motor has, so the whole pattern is tier 2.
            const segs = items.flatMap((i) => (i.kind === 'amplitude' ? i.segs : []));
            return { ...compileAmplitude({ ...cx, events: [] }, segs), notes, tier: 2 };
        }
        const steps = [];
        let request = null;
        if (!mixed) {
            let end = 0;
            const comp = primitiveItems.map((item) => {
                const dur = primitiveMs(cx.caps, item.id);
                const delayMs = Math.max(0, round(item.ev.at - end));
                end = Math.max(end, item.ev.at) + dur;
                return { kind: 'primitive', primitive: item.id, scale: item.scale, delayMs };
            });
            request = { ...cx.base, effect: { type: 'composition', steps: comp } };
            steps.push({ atMs: 0, request });
        }
        else {
            notes.push('Mixed: runs as a scheduled step list');
            // Each step cancels the one before it, so steps start after the previous one ends.
            let end = 0;
            for (const item of items) {
                if (item.kind === 'primitive') {
                    const start = Math.max(round(item.ev.at), end);
                    if (start >= cx.maxMs)
                        continue;
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
                }
                else if (item.segs.length) {
                    const origin = Math.max(round(item.segs[0].at), end);
                    const built = waveformRequest(item.segs, origin, true, cx.base, cx.maxMs);
                    if (built.end <= origin)
                        continue;
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
    function compileAmplitude(cx, given) {
        const notes = [];
        const raw = given ?? cx.events.flatMap((ev) => amplitudeSegments(ev, cx.caps, cx.scale, cx.maxAmp));
        const { segs, cut } = applyCaps(raw.sort((a, b) => a.at - b.at), cx.maxMs, cx.maxAmp);
        if (cut)
            notes.push(`Truncated to ${cx.maxMs} ms`);
        notes.push(`${segs.length} one-shot segments, neighbours within ${MERGE_WITHIN} merged`);
        const request = segs.length ? waveformRequest(segs, 0, true, cx.base, cx.maxMs).request : null;
        const segments = toSegmentReport(segs, 2);
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
    function compileOnOff(cx) {
        const notes = [];
        const quiet = compileAmplitude(cx);
        const source = quiet.segments.map((s) => ({
            at: s.atMs,
            dur: s.durationMs,
            amp: round(s.amplitude * 255),
        }));
        const on = [];
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
            notes.push(`${dropped} quiet segment${dropped > 1 ? 's' : ''} under amplitude ${ON_OFF_FLOOR} dropped`);
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
    function compilePattern(pattern, caps, opts = {}) {
        const target = Math.min(caps.topTier, opts.tier ?? 4, opts.maxTier ?? 4);
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
        const base = {
            ...(pattern.id !== undefined ? { id: pattern.id } : {}),
            usage: opts.usage ?? pattern.usage ?? 'media',
            ...(opts.respectSystemSettings !== undefined
                ? { respectSystemSettings: opts.respectSystemSettings }
                : {}),
        };
        const cx = {
            pattern,
            caps,
            events: sortEvents(pattern.events),
            scale: clamp01$1(Number.isFinite(opts.scale) ? opts.scale : 1),
            maxMs: caps.limits.maxDurationMs,
            maxAmp: Math.min(255, caps.limits.maxAmplitude),
            base,
        };
        const carried = [];
        let result = null;
        if (target === 4) {
            const env = compileEnvelope(cx);
            if ('fallback' in env)
                carried.push(env.fallback);
            else
                result = env;
        }
        if (!result) {
            if (target >= 3)
                result = compilePrimitives(cx);
            else if (target === 2)
                result = compileAmplitude(cx);
            else
                result = compileOnOff(cx);
        }
        return { id: pattern.id, ...result, notes: [...carried, ...result.notes] };
    }

    // Per-pattern scheduler for the interrupt, queue, drop-if-busy and coalesce policies
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const MAX_QUEUE = 4;
    const MAX_MERGES = 3;
    const MERGE_BOOST = 0.15;
    /**
     * Decides whether and when a compiled pattern runs. It only schedules: playing is the caller's
     * `run` function, so the same scheduler serves any backend. `stop()` clears every queue and timer.
     */
    class PatternScheduler {
        state = new Map();
        submit(job) {
            const st = this.get(job.key);
            const policy = job.policy;
            const now = Date.now();
            if (policy === 'interrupt') {
                this.clear(st);
                return this.play(st, job, job.scale, 'played');
            }
            if (policy === 'drop-if-busy') {
                if (now < st.busyUntil)
                    return Promise.resolve({ policy: 'dropped' });
                return this.play(st, job, job.scale, 'played');
            }
            if (policy === 'queue') {
                if (now >= st.busyUntil && st.queue.length === 0) {
                    return this.play(st, job, job.scale, 'played');
                }
                if (st.queue.length >= MAX_QUEUE)
                    return Promise.resolve({ policy: 'dropped' });
                return this.enqueue(st, job);
            }
            return this.coalesce(st, job, policy.coalesce);
        }
        /** True while the pattern with this key is still within its estimated run time. */
        isBusy(key) {
            const st = this.state.get(key);
            return st !== undefined && Date.now() < st.busyUntil;
        }
        /** Cancels one pattern's queued, merged and pending triggers and forgets its state. */
        cancel(key) {
            const st = this.state.get(key);
            if (!st)
                return;
            this.clear(st);
            this.state.delete(key);
        }
        /** Cancels every queued, merged and pending trigger and clears the busy state. */
        stop() {
            for (const st of this.state.values())
                this.clear(st);
            this.state.clear();
        }
        get(key) {
            let st = this.state.get(key);
            if (!st) {
                st = { busyUntil: 0, queue: [] };
                this.state.set(key, st);
            }
            return st;
        }
        clear(st) {
            for (const w of st.queue) {
                clearTimeout(w.timer);
                w.settle({ policy: 'dropped' });
            }
            st.queue = [];
            if (st.group) {
                clearTimeout(st.group.timer);
                st.group.settle({ policy: 'dropped' });
                st.group = undefined;
            }
            st.busyUntil = 0;
        }
        async play(st, job, scale, policy) {
            st.busyUntil = Date.now() + job.estimatedMs;
            try {
                const result = await job.run(Math.min(1, scale));
                return { policy, result };
            }
            catch (err) {
                // A play that failed never ran, so the pattern is not busy.
                st.busyUntil = 0;
                throw err;
            }
        }
        enqueue(st, job) {
            const start = Math.max(Date.now(), st.busyUntil);
            const wait = start - Date.now();
            // Reserve the slot now so a later trigger queues behind this one.
            st.busyUntil = start + job.estimatedMs;
            return new Promise((resolve, reject) => {
                const entry = {
                    settle: resolve,
                    timer: setTimeout(() => {
                        st.queue = st.queue.filter((w) => w !== entry);
                        // Start from a promise so a synchronous throw in `run` rejects instead of escaping.
                        Promise.resolve()
                            .then(() => job.run(job.scale))
                            .then((result) => resolve({ policy: 'queued', result }), reject);
                    }, wait),
                };
                st.queue.push(entry);
            });
        }
        coalesce(st, job, windowMs) {
            const group = st.group;
            if (group) {
                // Merge into the open group; once it holds three merges later triggers are absorbed
                // without a further boost.
                if (group.merges < MAX_MERGES)
                    group.merges++;
                group.scale = Math.max(group.scale, job.scale);
                return Promise.resolve({ policy: 'coalesced' });
            }
            return new Promise((resolve, reject) => {
                const created = {
                    job,
                    merges: 0,
                    scale: job.scale,
                    settle: resolve,
                    timer: setTimeout(() => {
                        st.group = undefined;
                        const boosted = created.scale + MERGE_BOOST * created.merges;
                        this.play(st, job, boosted, 'played').then(resolve, reject);
                    }, windowMs),
                };
                st.group = created;
            });
        }
    }

    // Portable haptic pattern format, shared by the validator, compiler and scheduler
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PATTERN_FORMAT = 'haptics-lab/pattern@1';
    /** Shortest continuous event, in ms. */
    const MIN_CONTINUOUS_MS = 20;

    // Pattern validation that reports every problem at once, each with a path and a fix
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const USAGES = ['touch', 'notification', 'alarm', 'media'];
    const POLICIES = ['interrupt', 'queue', 'drop-if-busy'];
    /** Longest coalesce window; far above this a timer would be a bug, not a window. */
    const MAX_COALESCE_MS = 1000;
    function isRecord(v) {
        return typeof v === 'object' && v !== null && !Array.isArray(v);
    }
    function isNumber(v) {
        return typeof v === 'number' && Number.isFinite(v);
    }
    function show(v) {
        return typeof v === 'number' || typeof v === 'string' ? String(v) : typeof v;
    }
    /** Checks a number that must sit in 0..1. */
    function checkUnit(value, path, issues) {
        if (!isNumber(value)) {
            issues.push({ path, message: `${show(value)} is not a number. Use 0..1.` });
        }
        else if (value > 1) {
            issues.push({ path, message: `${value} is above 1. Use 0..1.` });
        }
        else if (value < 0) {
            issues.push({ path, message: `${value} is below 0. Use 0..1.` });
        }
    }
    function checkCurve(points, path, issues) {
        if (points.length < 2) {
            issues.push({
                path,
                message: `${points.length} point${points.length === 1 ? '' : 's'}. A curve needs at least 2.`,
            });
            return;
        }
        let previous = -Infinity;
        points.forEach((point, i) => {
            const here = `${path}[${i}]`;
            if (!isRecord(point)) {
                issues.push({ path: here, message: 'Not a point. Use { t, v } with both in 0..1.' });
                return;
            }
            checkUnit(point.v, `${here}.v`, issues);
            if (!isNumber(point.t)) {
                issues.push({ path: `${here}.t`, message: `${show(point.t)} is not a number. Use 0..1.` });
                return;
            }
            if (point.t < 0 || point.t > 1) {
                issues.push({ path: `${here}.t`, message: `${point.t} is outside 0..1. Use 0..1.` });
            }
            if (point.t <= previous) {
                issues.push({
                    path: `${here}.t`,
                    message: `${point.t} does not come after ${previous}. Keep t ascending.`,
                });
            }
            previous = point.t;
        });
        const first = points[0];
        const last = points[points.length - 1];
        if (isRecord(first) && isNumber(first.t) && first.t !== 0) {
            issues.push({ path: `${path}[0].t`, message: `${first.t} is not 0. A curve starts at t = 0.` });
        }
        if (isRecord(last) && isNumber(last.t) && last.t !== 1) {
            issues.push({
                path: `${path}[${points.length - 1}].t`,
                message: `${last.t} is not 1. A curve ends at t = 1.`,
            });
        }
    }
    function checkLevel(value, path, issues) {
        if (Array.isArray(value))
            checkCurve(value, path, issues);
        else
            checkUnit(value, path, issues);
    }
    /**
     * Returns every problem with `input`, or an empty list when it is a valid pattern. Never throws.
     */
    function validatePattern(input, opts = {}) {
        const issues = [];
        if (!isRecord(input)) {
            return [{ path: '', message: 'Not a pattern. Use an object with a format and events.' }];
        }
        if (input.format !== PATTERN_FORMAT) {
            issues.push({
                path: 'format',
                message: `${show(input.format)} is not a known format. Use "${PATTERN_FORMAT}".`,
            });
        }
        if (input.id !== undefined && (typeof input.id !== 'string' || input.id.length === 0)) {
            issues.push({ path: 'id', message: 'The id must be a non-empty string.' });
        }
        if (input.usage !== undefined && !USAGES.includes(input.usage)) {
            issues.push({
                path: 'usage',
                message: `${show(input.usage)} is not a usage. Use ${USAGES.join(', ')}.`,
            });
        }
        checkPolicy(input.policy, issues);
        const events = input.events;
        if (!Array.isArray(events)) {
            issues.push({ path: 'events', message: 'Events must be a list.' });
            return issues;
        }
        if (events.length === 0) {
            issues.push({ path: 'events', message: 'No events. Add at least one.' });
            return issues;
        }
        let end = 0;
        events.forEach((event, i) => {
            const at = `events[${i}]`;
            if (!isRecord(event)) {
                issues.push({ path: at, message: 'Not an event. Use a transient or a continuous event.' });
                return;
            }
            if (!isNumber(event.at)) {
                issues.push({
                    path: `${at}.at`,
                    message: `${show(event.at)} is not a number. Use ms from 0.`,
                });
            }
            else if (event.at < 0) {
                issues.push({
                    path: `${at}.at`,
                    message: `${event.at} ms is before the start. Use 0 or more.`,
                });
            }
            const start = isNumber(event.at) && event.at >= 0 ? event.at : 0;
            if (event.type === 'transient') {
                checkUnit(event.intensity, `${at}.intensity`, issues);
                checkUnit(event.sharpness, `${at}.sharpness`, issues);
                end = Math.max(end, start);
            }
            else if (event.type === 'continuous') {
                if (!isNumber(event.duration)) {
                    issues.push({
                        path: `${at}.duration`,
                        message: `${show(event.duration)} is not a number. Use ms, at least ${MIN_CONTINUOUS_MS}.`,
                    });
                }
                else if (event.duration < MIN_CONTINUOUS_MS) {
                    issues.push({
                        path: `${at}.duration`,
                        message: `${event.duration} ms. Continuous events need at least ${MIN_CONTINUOUS_MS} ms.`,
                    });
                }
                else {
                    end = Math.max(end, start + event.duration);
                }
                checkLevel(event.intensity, `${at}.intensity`, issues);
                checkLevel(event.sharpness, `${at}.sharpness`, issues);
            }
            else {
                issues.push({
                    path: `${at}.type`,
                    message: `${show(event.type)} is not an event type. Use "transient" or "continuous".`,
                });
            }
        });
        if (opts.maxDurationMs !== undefined && end > opts.maxDurationMs) {
            issues.push({
                path: 'events',
                message: `The pattern runs for ${end} ms. The limit is ${opts.maxDurationMs} ms; shorten or move events earlier.`,
            });
        }
        return issues;
    }
    function checkPolicy(policy, issues) {
        if (policy === undefined)
            return;
        if (typeof policy === 'string') {
            if (!POLICIES.includes(policy)) {
                issues.push({
                    path: 'policy',
                    message: `${policy} is not a policy. Use ${POLICIES.join(', ')} or { coalesce: ms }.`,
                });
            }
            return;
        }
        if (isRecord(policy) &&
            isNumber(policy.coalesce) &&
            policy.coalesce > 0 &&
            policy.coalesce <= MAX_COALESCE_MS) {
            return;
        }
        issues.push({
            path: 'policy',
            message: `Not a policy. Use interrupt, queue, drop-if-busy or { coalesce: ms } with ms from 1 to ${MAX_COALESCE_MS}.`,
        });
    }
    /** Type guard: true when `input` has no validation issues. */
    function isPattern(input, opts = {}) {
        return validatePattern(input, opts).length === 0;
    }
    /** One line per issue, for error messages and logs. */
    function formatIssues(issues) {
        return issues.map((i) => (i.path ? `${i.path}: ${i.message}` : i.message)).join('\n');
    }

    // Public haptics API: capabilities, raw playback, the UI lane and the pattern registry
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    /** Invalid input rejects with one of these; hardware limits never do. */
    class HapticsError extends Error {
        code;
        constructor(code, message) {
            super(message);
            this.code = code;
            this.name = 'HapticsError';
        }
    }
    // ── state ─────────────────────────────────────────────────────────────────────────────────────
    let cached = null;
    let loaded = null;
    let masterScale = 1;
    let maxTier = null;
    let stopCount = 0; // moves on with every stop(), so a trigger waiting on capabilities can tell
    const patterns = new Map();
    const scheduler = new PatternScheduler();
    const clamp01 = (v) => Math.min(1, Math.max(0, v));
    /** A 0..1 option, with a missing or non-finite value counting as full strength. */
    const unit = (v) => (Number.isFinite(v) ? clamp01(v) : 1);
    function load() {
        const next = core.invoke('plugin:haptics|capabilities').then((caps) => {
            if (cached === next)
                loaded = caps;
            return caps;
        }, (err) => {
            if (cached === next)
                cached = null;
            throw err;
        });
        cached = next;
        return next;
    }
    // The touch-feedback setting can change while the app is away, so read it again on return.
    if (typeof document !== 'undefined') {
        document.addEventListener('visibilitychange', () => {
            if (document.visibilityState === 'visible' && cached)
                void load().catch(() => undefined);
        });
    }
    // ── capabilities ──────────────────────────────────────────────────────────────────────────────
    /** Reads the device capabilities once and caches them; `refresh` reads them again. */
    function capabilities(opts) {
        if (opts?.refresh || !cached)
            return load();
        return cached;
    }
    // ── global controls ───────────────────────────────────────────────────────────────────────────
    /** Multiplies every intensity, 0..1. Applies to patterns and raw `play()`. */
    function setMasterScale(v) {
        masterScale = clamp01(Number.isFinite(v) ? v : 1);
    }
    /** Caps the tier for testing and previews; `null` uses the device's top tier. */
    function setMaxTier(t) {
        maxTier = t;
    }
    /** Cancels the motor and clears every queue, pending merge and timer. */
    async function stop() {
        stopCount++;
        scheduler.stop();
        await core.invoke('plugin:haptics|stop');
    }
    // ── patterns ──────────────────────────────────────────────────────────────────────────────────
    function assertValid(pattern, caps) {
        const issues = validatePattern(pattern, { maxDurationMs: caps.limits.maxDurationMs });
        if (issues.length)
            throw new HapticsError('INVALID_EFFECT', formatIssues(issues));
    }
    function requireCaps() {
        if (!loaded) {
            throw new Error('Capabilities are not loaded yet. Await capabilities() or register() first.');
        }
        return loaded;
    }
    /**
     * Compiles a pattern for this device without playing it. Synchronous and pure, so the lab can
     * preview every tier. Needs the capabilities to have been loaded.
     */
    function compile(pattern, opts) {
        const caps = requireCaps();
        assertValid(pattern, caps);
        return compilePattern(pattern, caps, { tier: opts?.tier, maxTier, scale: masterScale });
    }
    /** Validates a pattern, remembers it under `id` and reports how it compiles on this device. */
    async function register(id, pattern, opts) {
        const caps = await capabilities();
        assertValid(pattern, caps);
        // Copy it, so editing the caller's object later can't change what is registered.
        const own = { ...structuredClone(pattern), id };
        patterns.set(id, { pattern: own, options: opts });
        return compilePattern(own, caps, {
            tier: opts?.tier,
            maxTier,
            scale: masterScale,
        });
    }
    async function registerAll(table) {
        const reports = {};
        for (const [id, value] of Object.entries(table)) {
            const entry = 'pattern' in value ? value : { pattern: value };
            reports[id] = await register(id, entry.pattern, entry.options);
        }
        return reports;
    }
    function unregister(id) {
        patterns.delete(id);
        scheduler.cancel(id);
    }
    const REASON_NOTES = /missing on this motor|drops to tier 2|over the|Capped|Truncated|No envelope/;
    function silent(reason, decision, tier = 0) {
        return {
            ok: true,
            tier,
            target: 'phone',
            estimatedMs: 0,
            downgraded: reason !== '',
            ...(reason ? { reason, downgradeReason: reason } : {}),
            ...(decision ? { policy: decision } : {}),
        };
    }
    async function playCompiled(report) {
        if (report.steps.length === 0) {
            return silent(report.notes[0] ?? 'Nothing to play', undefined, report.tier);
        }
        // Compiled output already carries the master scale and the tier cap, so it skips `play()`.
        if (report.request && report.steps.length === 1)
            return sendPlay(report.request);
        return sendSteps(report.steps);
    }
    /** Plays a registered pattern by id, following its policy. */
    async function trigger(id, opts = {}) {
        const entry = patterns.get(id);
        if (!entry)
            throw new HapticsError('UNKNOWN_PATTERN', `No pattern is registered as "${id}".`);
        const stoppedAt = stopCount;
        const caps = await capabilities();
        if (stopCount !== stoppedAt)
            return silent('Stopped before it played', 'dropped');
        const tier = Math.min(entry.options?.tier ?? 4, opts.tier ?? 4);
        const compileFor = (scale) => compilePattern(entry.pattern, caps, {
            tier,
            maxTier,
            scale,
            usage: opts.usage,
            respectSystemSettings: opts.respectSystemSettings,
        });
        const triggerScale = unit(opts.scale);
        if (masterScale * triggerScale === 0)
            return silent('Scale is 0, so nothing plays');
        const first = compileFor(masterScale * triggerScale);
        const outcome = await scheduler.submit({
            key: id,
            policy: entry.pattern.policy ?? 'interrupt',
            estimatedMs: first.estimatedMs,
            scale: triggerScale,
            run: (scale) => {
                // A merged group arrives with a higher scale, so compile again at that strength.
                const report = scale === triggerScale ? first : compileFor(masterScale * scale);
                return playCompiled(report).then((res) => withNotes(res, report, caps));
            },
        });
        if (outcome.result)
            return { ...outcome.result, policy: outcome.policy };
        return silent('', outcome.policy, first.tier);
    }
    function withNotes(res, report, caps) {
        const reasons = res.reason ? res.reason.split(' · ') : [];
        if (report.tier < caps.topTier) {
            reasons.push(maxTier !== null && report.tier === maxTier
                ? `Capped at tier ${maxTier} by setMaxTier`
                : `Compiled at tier ${report.tier}`);
        }
        for (const note of report.notes)
            if (REASON_NOTES.test(note))
                reasons.push(note);
        const unique = [...new Set(reasons)];
        if (!unique.length)
            return res;
        const reason = unique.join(' · ');
        return { ...res, downgraded: true, reason, downgradeReason: reason };
    }
    // ── raw ───────────────────────────────────────────────────────────────────────────────────────
    function effectTier(req, caps) {
        switch (req.effect.type) {
            case 'envelopeWaveform':
                return 4;
            case 'composition':
                return 3;
            case 'predefined':
                return Math.min(caps.topTier, 3);
            default:
                return caps.hasAmplitudeControl ? 2 : 1;
        }
    }
    /** Applies the master scale to a raw request's amplitude fields. */
    function scaled(req) {
        if (masterScale === 1)
            return req;
        const e = req.effect;
        switch (e.type) {
            case 'oneshot':
                return {
                    ...req,
                    effect: { ...e, amplitude: Math.max(1, Math.round((e.amplitude ?? 255) * masterScale)) },
                };
            case 'waveform':
                if (!e.amplitudes)
                    return req;
                return {
                    ...req,
                    effect: { ...e, amplitudes: e.amplitudes.map((a) => Math.round(a * masterScale)) },
                };
            case 'composition':
                return {
                    ...req,
                    effect: {
                        ...e,
                        steps: e.steps.map((s) => ({ ...s, scale: clamp01((s.scale ?? 1) * masterScale) })),
                    },
                };
            case 'envelopeWaveform':
                return {
                    ...req,
                    effect: {
                        ...e,
                        controlPoints: e.controlPoints.map((p) => ({
                            ...p,
                            amplitude: clamp01(p.amplitude * masterScale),
                        })),
                    },
                };
            default:
                return req;
        }
    }
    /**
     * The raw escape hatch: no compiler, no policies. It still applies the plugin limits, the master
     * scale and the `setMaxTier` cap, which resolves a request above the cap at tier 0.
     */
    async function play(req) {
        if (masterScale === 0)
            return silent('Master scale is 0, so nothing plays');
        if (maxTier !== null) {
            const caps = await capabilities();
            if (effectTier(req, caps) > maxTier) {
                return silent(`Capped at tier ${maxTier} by setMaxTier`);
            }
        }
        return sendPlay(scaled(req));
    }
    /**
     * Hardware limits never make a play call reject, so a rejection means the native side refused the
     * input. Native errors arrive as plain strings; wrap them so callers always get `{ code, message }`.
     */
    async function invalidInput(call) {
        try {
            return await call;
        }
        catch (err) {
            if (err instanceof HapticsError)
                throw err;
            throw new HapticsError('INVALID_EFFECT', err instanceof Error ? err.message : String(err));
        }
    }
    function sendPlay(req) {
        return invalidInput(core.invoke('plugin:haptics|play', { req }));
    }
    function sendSteps(steps) {
        return invalidInput(core.invoke('plugin:haptics|play_steps', { steps }));
    }
    /** Plays `{ atMs, request }` steps scheduled natively from one start time. */
    async function playSteps(steps) {
        if (masterScale === 0)
            return silent('Master scale is 0, so nothing plays');
        if (maxTier !== null) {
            const caps = await capabilities();
            if (steps.some((s) => effectTier(s.request, caps) > maxTier)) {
                return silent(`Capped at tier ${maxTier} by setMaxTier`);
            }
        }
        return sendSteps(steps.map((s) => ({ ...s, request: scaled(s.request) })));
    }
    // ── UI lane ───────────────────────────────────────────────────────────────────────────────────
    /** System-style feedback that follows the touch-feedback setting. Not affected by the controls above. */
    function ui(kind) {
        return invalidInput(core.invoke('plugin:haptics|ui', { kind }));
    }

    exports.AMPLITUDE_CEILING = AMPLITUDE_CEILING;
    exports.HapticsError = HapticsError;
    exports.MIN_CONTINUOUS_MS = MIN_CONTINUOUS_MS;
    exports.NEIGHBOURS = NEIGHBOURS;
    exports.PATTERN_FORMAT = PATTERN_FORMAT;
    exports.PRIMITIVE_IDS = PRIMITIVE_IDS;
    exports.PRIMITIVE_MS = PRIMITIVE_MS;
    exports.PatternScheduler = PatternScheduler;
    exports.capabilities = capabilities;
    exports.compile = compile;
    exports.compilePattern = compilePattern;
    exports.formatIssues = formatIssues;
    exports.isPattern = isPattern;
    exports.pickPrimitive = pickPrimitive;
    exports.play = play;
    exports.playSteps = playSteps;
    exports.register = register;
    exports.registerAll = registerAll;
    exports.resolvePrimitive = resolvePrimitive;
    exports.setMasterScale = setMasterScale;
    exports.setMaxTier = setMaxTier;
    exports.stop = stop;
    exports.trigger = trigger;
    exports.ui = ui;
    exports.unregister = unregister;
    exports.validatePattern = validatePattern;

    return exports;

})({}, __TAURI__.core);
Object.defineProperty(window.__TAURI__, 'haptics', { value: __TAURI_PLUGIN_HAPTICS__ }) }

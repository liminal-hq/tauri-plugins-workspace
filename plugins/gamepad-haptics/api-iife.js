if ('__TAURI__' in window) {
var __TAURI_PLUGIN_GAMEPAD_HAPTICS__ = (function (exports, core, event) {
    'use strict';

    // Matches a pad the game knows (a Web Gamepad, an SDL GUID, a vendor and product) to a pad the plugin can drive
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const hex = (s) => Number.parseInt(s, 16);
    /**
     * Reads the vendor and product ids out of a Web Gamepad `id`. Chromium writes
     * `Name (STANDARD GAMEPAD Vendor: 054c Product: 0268)`, Firefox `054c-0268-Name`; Safari writes
     * neither, so it returns undefined and the caller falls back to the name.
     */
    function parseWebGamepadId(id) {
        const chromium = /Vendor:\s*([0-9a-f]{4})\s+Product:\s*([0-9a-f]{4})/i.exec(id);
        if (chromium)
            return { vendorId: hex(chromium[1]), productId: hex(chromium[2]) };
        const firefox = /^([0-9a-f]{4})-([0-9a-f]{4})-/i.exec(id);
        if (firefox)
            return { vendorId: hex(firefox[1]), productId: hex(firefox[2]) };
        return undefined;
    }
    const normaliseName = (s) => s
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, ' ')
        .trim();
    /** The pads that fit `hint`, never guessing between identical ones. */
    function resolvePad(pads, hint) {
        let matches;
        if ('guid' in hint) {
            matches = pads.filter((p) => p.guid.toLowerCase() === hint.guid.toLowerCase());
        }
        else if ('vendorId' in hint) {
            matches = pads.filter((p) => p.vendorId === hint.vendorId && p.productId === hint.productId);
            if (hint.serial !== undefined) {
                const exact = matches.filter((p) => p.serial === hint.serial);
                if (exact.length > 0)
                    matches = exact;
            }
        }
        else {
            const ids = parseWebGamepadId(hint.gamepad.id);
            if (ids) {
                matches = pads.filter((p) => p.vendorId === ids.vendorId && p.productId === ids.productId);
            }
            else {
                const name = normaliseName(hint.gamepad.id);
                matches = pads.filter((p) => {
                    const n = normaliseName(p.name);
                    return n.length > 0 && (name.includes(n) || n.includes(name));
                });
            }
        }
        return { matches, ambiguous: matches.length > 1 };
    }

    // Portable haptic pattern format, shared by the validator, compiler and scheduler
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PATTERN_FORMAT = 'haptics-lab/pattern@1';
    /** Shortest continuous event, in ms. */
    const MIN_CONTINUOUS_MS = 20;
    /** A transient always plays for at least this long, so it needs that much room before the limit. */
    const MIN_TRANSIENT_MS = 1;

    // Compiles a haptic pattern into motor frames for a pad's tier
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    // Pure TypeScript, like the rest of this folder. The Rust rules in `src/validate.rs` are the source
    // of truth for what a frame list may hold; this compiler emits inside them.
    const MAX_FRAMES = 512;
    /** Time between samples. Longer patterns widen it so they stay within `MAX_FRAMES`. */
    const SAMPLE_MS = 10;
    /** A transient is stretched to this long, so a motor has time to spin up. */
    const MIN_TAP_MS = 40;
    /** Levels that differ by less than this are the same frame. */
    const SAME_LEVEL = 0.02;
    const clamp01 = (v) => Math.min(1, Math.max(0, v));
    /** The value of a constant or a `{t, v}` curve at `t`, 0..1 across the event. */
    function curveAt(curve, t) {
        if (typeof curve === 'number')
            return clamp01(curve);
        if (curve.length === 0)
            return 0;
        const points = [...curve].sort((a, b) => a.t - b.t);
        if (t <= points[0].t)
            return clamp01(points[0].v);
        for (let i = 1; i < points.length; i++) {
            const a = points[i - 1];
            const b = points[i];
            if (t <= b.t) {
                const span = b.t - a.t;
                return clamp01(span === 0 ? b.v : a.v + ((b.v - a.v) * (t - a.t)) / span);
            }
        }
        return clamp01(points[points.length - 1].v);
    }
    /**
     * How an intensity and a sharpness split across the two body motors. Sharp is crisp and goes to the
     * light motor, dull is deep and goes to the heavy motor; the 1.25 keeps a mid sharpness from
     * halving both.
     */
    function mix(intensity, sharpness) {
        const i = clamp01(intensity);
        const s = clamp01(sharpness);
        return {
            heavy: i * Math.min(1, 1.25 * (1 - s)),
            light: i * Math.min(1, 1.25 * s),
        };
    }
    function endOf(event) {
        return event.type === 'transient'
            ? event.at + MIN_TAP_MS
            : event.at + Math.max(event.duration, MIN_CONTINUOUS_MS);
    }
    /** The levels every event asks for at time `t`; overlapping events take the stronger level. */
    function levelsAt(events, t, tier) {
        const out = { heavy: 0, light: 0, left: 0, right: 0 };
        for (const event of events) {
            if (t < event.at || t >= endOf(event))
                continue;
            let m;
            let trigger;
            if (event.type === 'transient') {
                m = mix(event.intensity, event.sharpness);
                trigger = { left: 0, right: clamp01(event.intensity) };
            }
            else {
                const c = event;
                const p = (t - c.at) / Math.max(c.duration, MIN_CONTINUOUS_MS);
                const i = curveAt(c.intensity, p);
                m = mix(i, curveAt(c.sharpness, p));
                trigger = { left: i, right: i };
            }
            out.heavy = Math.max(out.heavy, m.heavy);
            out.light = Math.max(out.light, m.light);
            if (tier >= 3) {
                out.left = Math.max(out.left, trigger.left);
                out.right = Math.max(out.right, trigger.right);
            }
        }
        if (tier <= 1) {
            out.heavy = Math.max(out.heavy, out.light);
            out.light = 0;
        }
        return out;
    }
    const near = (a, b) => Math.abs(a.heavy - b.heavy) < SAME_LEVEL &&
        Math.abs(a.light - b.light) < SAME_LEVEL &&
        Math.abs((a.leftTrigger ?? 0) - (b.leftTrigger ?? 0)) < SAME_LEVEL &&
        Math.abs((a.rightTrigger ?? 0) - (b.rightTrigger ?? 0)) < SAME_LEVEL;
    const isSilent = (f) => f.heavy === 0 && f.light === 0 && !(f.leftTrigger ?? 0) && !(f.rightTrigger ?? 0);
    /**
     * The tier a frame list needs, which is what the Rust side reports back as its request tier: 3 with
     * triggers, 2 when both body motors are used and differ somewhere, else 1.
     */
    function framesTier(frames) {
        if (frames.length === 0 || frames.every(isSilent))
            return 0;
        if (frames.some((f) => (f.leftTrigger ?? 0) > 0 || (f.rightTrigger ?? 0) > 0))
            return 3;
        const heavy = frames.some((f) => f.heavy > 0);
        const light = frames.some((f) => f.light > 0);
        if (heavy && light && frames.some((f) => f.heavy !== f.light))
            return 2;
        return 1;
    }
    const round = (v) => Math.round(v * 1000) / 1000;
    function compilePad(pattern, caps, options = {}) {
        const notes = [];
        const tier = Math.min(caps.topTier, options.maxTier ?? 3);
        if (tier === 0) {
            return { frames: [], tier: 0, estimatedMs: 0, notes: ['This pad cannot play rumble'] };
        }
        if (tier < 3 && caps.topTier >= 3 && options.maxTier !== undefined) {
            notes.push(`Compiled for tier ${tier} as asked`);
        }
        const events = [...pattern.events].sort((a, b) => a.at - b.at);
        let end = Math.max(0, ...events.map(endOf));
        if (end > caps.maxDurationMs) {
            end = caps.maxDurationMs;
            notes.push(`Truncated at the limit of ${caps.maxDurationMs} ms`);
        }
        if (end === 0)
            return { frames: [], tier: 0, estimatedMs: 0, notes };
        // Widen the sample step when a long pattern would otherwise need more frames than allowed.
        const step = Math.max(SAMPLE_MS, Math.ceil(end / (MAX_FRAMES - 1)));
        if (step > SAMPLE_MS)
            notes.push(`Sampled every ${step} ms to stay within ${MAX_FRAMES} frames`);
        const frames = [];
        for (let start = 0; start < end; start += step) {
            const durationMs = Math.min(step, end - start);
            const l = levelsAt(events, start + durationMs / 2, tier);
            const frame = { durationMs, heavy: round(l.heavy), light: round(l.light) };
            if (tier >= 3) {
                frame.leftTrigger = round(l.left);
                frame.rightTrigger = round(l.right);
            }
            const last = frames[frames.length - 1];
            if (last && near(last, frame))
                last.durationMs += durationMs;
            else
                frames.push(frame);
        }
        // A trailing silence carries no rumble.
        while (frames.length > 0 && isSilent(frames[frames.length - 1]))
            frames.pop();
        const estimatedMs = frames.reduce((sum, f) => sum + f.durationMs, 0);
        const needed = framesTier(frames);
        if (needed === 0) {
            notes.push('Nothing in the pattern is strong enough to feel');
            return { frames: [], tier: 0, estimatedMs: 0, notes };
        }
        return { frames, tier: Math.min(needed, tier), estimatedMs, notes };
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
        /** A run that played nothing was never busy, so the pattern is free again. */
        release(st, job, result) {
            if (job.didPlay && !job.didPlay(result) && st.queue.length === 0)
                st.busyUntil = 0;
        }
        async play(st, job, scale, policy) {
            st.busyUntil = Date.now() + job.estimatedMs;
            try {
                const result = await job.run(Math.min(1, scale));
                this.release(st, job, result);
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
                            .then((result) => {
                            this.release(st, job, result);
                            resolve({ policy: 'queued', result });
                        }, reject);
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
                end = Math.max(end, start + MIN_TRANSIENT_MS);
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
    /** One line per issue, for error messages and logs. */
    function formatIssues(issues) {
        return issues.map((i) => (i.path ? `${i.path}: ${i.message}` : i.message)).join('\n');
    }

    // Plays frames on a pad through the webview's Gamepad API, for when no native path can address it
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    /** The pads the webview exposes. The browser lists none until a button is pressed on the page. */
    function webPads() {
        if (typeof navigator === 'undefined' || typeof navigator.getGamepads !== 'function')
            return [];
        return Array.from(navigator.getGamepads()).filter((p) => p !== null && p.connected !== false);
    }
    /** Whether a web pad can rumble: it needs a `dual-rumble` actuator. */
    function canRumble(pad) {
        return typeof pad.vibrationActuator?.playEffect === 'function';
    }
    const running = new Map();
    /** Stops one web pad, or every pad when `index` is omitted. */
    async function stopWeb(index) {
        const targets = index === undefined ? [...running.keys()] : [index];
        for (const i of targets) {
            for (const timer of running.get(i) ?? [])
                clearTimeout(timer);
            running.delete(i);
            const pad = webPads().find((p) => p.index === i);
            await pad?.vibrationActuator?.reset?.().catch(() => undefined);
        }
    }
    /**
     * Plays `frames` on a web pad: one `dual-rumble` effect per frame, each started on its own timer
     * so a pad that drops one still ends silent. Returns the time the pattern takes.
     */
    async function playWeb(pad, frames, scale = 1) {
        const actuator = pad.vibrationActuator;
        if (!actuator?.playEffect)
            throw new Error('This pad has no vibration actuator');
        await stopWeb(pad.index);
        const timers = [];
        running.set(pad.index, timers);
        let at = 0;
        for (const frame of frames) {
            const params = {
                startDelay: 0,
                duration: frame.durationMs,
                strongMagnitude: Math.min(1, frame.heavy * scale),
                weakMagnitude: Math.min(1, frame.light * scale),
            };
            if (at === 0) {
                await actuator.playEffect('dual-rumble', params);
            }
            else {
                timers.push(setTimeout(() => void actuator.playEffect?.('dual-rumble', params), at));
            }
            at += frame.durationMs;
        }
        timers.push(setTimeout(() => void stopWeb(pad.index), at + 50));
        return at;
    }

    // Typed guest-side wrappers for the gamepad-haptics plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:gamepad-haptics|';
    const PAD_CONNECTED_EVENT = 'gamepad-haptics://connected';
    const PAD_CHANGED_EVENT = 'gamepad-haptics://changed';
    const PAD_DISCONNECTED_EVENT = 'gamepad-haptics://disconnected';
    function capabilities() {
        return core.invoke(`${PREFIX}capabilities`);
    }
    function listPads() {
        return core.invoke(`${PREFIX}list_pads`);
    }
    /**
     * Plays motor frames on one native pad. Rust validates them first, so a bad request is rejected
     * even when `scale` is 0 or the pad cannot play. `scale` multiplies with the configured master scale.
     */
    function playFrames(padId, frames, scale) {
        return core.invoke(`${PREFIX}play_frames`, { args: { padId, frames, scale } });
    }
    /** Buzzes one pad in a pattern that tells it from the others, so a player can confirm which it is. */
    function identify(padId) {
        return core.invoke(`${PREFIX}identify`, { padId });
    }
    /** Stops one pad, or every pad when `padId` is omitted. */
    async function stop(padId) {
        await core.invoke(`${PREFIX}stop`, { padId });
        if (padId === undefined)
            await stopWeb();
    }
    function onPadConnected(handler) {
        return event.listen(PAD_CONNECTED_EVENT, (e) => handler(e.payload));
    }
    function onPadChanged(handler) {
        return event.listen(PAD_CHANGED_EVENT, (e) => handler(e.payload));
    }
    function onPadDisconnected(handler) {
        return event.listen(PAD_DISCONNECTED_EVENT, (e) => handler(e.payload));
    }
    const silent = (target, reason) => ({
        ok: true,
        tier: 0,
        target,
        downgraded: true,
        reason,
    });
    /**
     * Creates a backend that plays on a native pad when the plugin can address one and falls back to the
     * webview's Gamepad API when it cannot. One pad is never driven by both: a pad found natively is
     * played natively. Do not call `playEffect` on the same pad yourself while this backend plays.
     */
    function createBackend() {
        const patterns = new Map();
        const scheduler = new PatternScheduler();
        let masterScale = 1;
        let maxTier = null;
        async function limitsAndPads() {
            return capabilities();
        }
        function compileFor(pattern, topTier, maxDurationMs) {
            return compilePad(pattern, { topTier, maxDurationMs }, maxTier === null ? {} : { maxTier });
        }
        return {
            id: 'gamepad',
            capabilities: limitsAndPads,
            async register(id, pattern) {
                const caps = await limitsAndPads();
                const issues = validatePattern(pattern, { maxDurationMs: caps.limits.maxDurationMs });
                if (issues.length > 0)
                    throw new Error(formatIssues(issues));
                patterns.set(id, pattern);
                const best = caps.pads.reduce((t, p) => Math.max(t, p.topTier), 0);
                return compileFor(pattern, best === 0 ? 2 : best, caps.limits.maxDurationMs);
            },
            async trigger(id, options = {}) {
                const pattern = patterns.get(id);
                if (!pattern)
                    throw new Error(`Unknown pattern \`${id}\`. Register it first.`);
                const caps = await limitsAndPads();
                const scale = (options.scale ?? 1) * masterScale;
                const policy = pattern.policy ?? 'interrupt';
                // Choose the pad: an id, a hint, or the first native pad that can play.
                let native;
                let hintedWeb;
                if (options.padId !== undefined) {
                    native = caps.pads.find((p) => p.id === options.padId);
                    if (!native)
                        return silent(options.padId, `No pad \`${options.padId}\``);
                }
                else if (options.hint !== undefined) {
                    const found = resolvePad(caps.pads, options.hint);
                    if (found.ambiguous) {
                        return silent('gamepad', 'More than one pad fits; call identify() and pass the padId the player confirms');
                    }
                    native = found.matches[0];
                    if (!native && 'gamepad' in options.hint)
                        hintedWeb = options.hint.gamepad.index;
                }
                else {
                    native = caps.pads.find((p) => p.topTier > 0);
                }
                if (native) {
                    const report = compileFor(pattern, native.topTier, caps.limits.maxDurationMs);
                    if (report.frames.length === 0) {
                        return silent(native.id, report.notes[0] ?? 'Nothing to play');
                    }
                    const padId = native.id;
                    const outcome = await scheduler.submit({
                        key: id,
                        policy,
                        estimatedMs: report.estimatedMs,
                        scale,
                        run: (s) => playFrames(padId, report.frames, s === 1 ? undefined : s),
                        didPlay: (res) => res.tier > 0,
                    });
                    return outcome.result ?? silent(padId, `Not played: ${outcome.policy}`);
                }
                // No native pad: use the webview's Gamepad API.
                const web = webPads().filter(canRumble);
                const pad = hintedWeb === undefined ? web[0] : web.find((p) => p.index === hintedWeb);
                if (!pad)
                    return silent('gamepad', 'No gamepad found');
                const report = compileFor(pattern, 2, caps.limits.maxDurationMs);
                if (report.frames.length === 0)
                    return silent(`web:${pad.index}`, report.notes[0] ?? 'Nothing to play');
                const outcome = await scheduler.submit({
                    key: id,
                    policy,
                    estimatedMs: report.estimatedMs,
                    scale,
                    run: async (s) => {
                        await playWeb(pad, report.frames, s);
                        return { ok: true, tier: report.tier, target: `web:${pad.index}`, downgraded: false };
                    },
                    didPlay: (res) => res.tier > 0,
                });
                return outcome.result ?? silent(`web:${pad.index}`, `Not played: ${outcome.policy}`);
            },
            setMasterScale(value) {
                masterScale = Math.min(1, Math.max(0, value));
            },
            setMaxTier(tier) {
                maxTier = tier;
            },
            async stop() {
                scheduler.stop();
                await stop();
            },
        };
    }

    exports.PAD_CHANGED_EVENT = PAD_CHANGED_EVENT;
    exports.PAD_CONNECTED_EVENT = PAD_CONNECTED_EVENT;
    exports.PAD_DISCONNECTED_EVENT = PAD_DISCONNECTED_EVENT;
    exports.PATTERN_FORMAT = PATTERN_FORMAT;
    exports.capabilities = capabilities;
    exports.createBackend = createBackend;
    exports.identify = identify;
    exports.listPads = listPads;
    exports.onPadChanged = onPadChanged;
    exports.onPadConnected = onPadConnected;
    exports.onPadDisconnected = onPadDisconnected;
    exports.parseWebGamepadId = parseWebGamepadId;
    exports.playFrames = playFrames;
    exports.resolvePad = resolvePad;
    exports.stop = stop;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'gamepadHaptics', { value: __TAURI_PLUGIN_GAMEPAD_HAPTICS__ }) }

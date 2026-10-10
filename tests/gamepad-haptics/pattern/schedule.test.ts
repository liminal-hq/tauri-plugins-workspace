// Unit tests for the pattern scheduler policies, using fake timers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	MAX_QUEUE,
	PatternScheduler,
} from '../../../plugins/gamepad-haptics/guest-js/pattern/schedule';
import type { Job, Outcome } from '../../../plugins/gamepad-haptics/guest-js/pattern/schedule';
import type { Policy } from '../../../plugins/gamepad-haptics/guest-js/pattern/types';

type Played = { at: number; scale: number };

let scheduler: PatternScheduler;
let played: Played[];
let start: number;

beforeEach(() => {
	vi.useFakeTimers();
	scheduler = new PatternScheduler();
	played = [];
	start = Date.now();
});

afterEach(() => {
	scheduler.stop();
	vi.useRealTimers();
});

const job = (policy: Policy, estimatedMs = 100, scale = 0.5): Job<number> => ({
	key: 'cue',
	policy,
	estimatedMs,
	scale,
	run: async (s) => {
		played.push({ at: Date.now() - start, scale: s });
		return played.length;
	},
});

/** Fires `count` triggers `gapMs` apart and returns each outcome's promise. */
async function fire(
	policy: Policy,
	count = 5,
	gapMs = 25,
	estimatedMs = 100
): Promise<Promise<Outcome<number>>[]> {
	const out: Promise<Outcome<number>>[] = [];
	for (let i = 0; i < count; i++) {
		out.push(scheduler.submit(job(policy, estimatedMs)));
		if (i < count - 1) await vi.advanceTimersByTimeAsync(gapMs);
	}
	return out;
}

describe('interrupt', () => {
	it('plays every trigger immediately', async () => {
		const outcomes = await fire('interrupt');
		await vi.runAllTimersAsync();
		expect((await Promise.all(outcomes)).map((o) => o.policy)).toEqual(Array(5).fill('played'));
		expect(played.map((p) => p.at)).toEqual([0, 25, 50, 75, 100]);
	});
});

describe('queue', () => {
	it('plays one at a time, each waiting for the previous to end', async () => {
		const outcomes = await fire('queue', 4, 25, 100);
		await vi.runAllTimersAsync();
		const results = await Promise.all(outcomes);
		expect(results.map((o) => o.policy)).toEqual(['played', 'queued', 'queued', 'queued']);
		expect(played.map((p) => p.at)).toEqual([0, 100, 200, 300]);
	});

	it('drops triggers once the queue holds four', async () => {
		const outcomes = await fire('queue', 7, 5, 100);
		await vi.runAllTimersAsync();
		const policies = (await Promise.all(outcomes)).map((o) => o.policy);
		expect(policies.filter((p) => p === 'played')).toHaveLength(1);
		expect(policies.filter((p) => p === 'queued')).toHaveLength(MAX_QUEUE);
		expect(policies.filter((p) => p === 'dropped')).toHaveLength(2);
		expect(played).toHaveLength(1 + MAX_QUEUE);
	});

	it('plays straight away again once the queue has drained', async () => {
		const first = scheduler.submit(job('queue', 50));
		await vi.advanceTimersByTimeAsync(200);
		expect((await first).policy).toBe('played');
		const later = await scheduler.submit(job('queue', 50));
		expect(later.policy).toBe('played');
	});
});

describe('drop-if-busy', () => {
	it('ignores triggers while the pattern is still playing', async () => {
		const outcomes = await fire('drop-if-busy', 5, 25, 100);
		await vi.runAllTimersAsync();
		const policies = (await Promise.all(outcomes)).map((o) => o.policy);
		// 0 plays; 25, 50, 75 are inside 100 ms; 100 is free again.
		expect(policies).toEqual(['played', 'dropped', 'dropped', 'dropped', 'played']);
		expect(played.map((p) => p.at)).toEqual([0, 100]);
	});
});

describe('coalesce', () => {
	it('merges triggers inside the window into one stronger hit', async () => {
		const outcomes = await fire({ coalesce: 100 }, 5, 10);
		await vi.runAllTimersAsync();
		const policies = (await Promise.all(outcomes)).map((o) => o.policy);
		expect(policies).toEqual(['played', 'coalesced', 'coalesced', 'coalesced', 'coalesced']);
		expect(played).toHaveLength(1);
		// 0.5 + 0.15 × 3 merges; the fourth merge is absorbed without a further boost.
		expect(played[0].scale).toBeCloseTo(0.95);
		expect(played[0].at).toBe(100);
	});

	it('plays a group at its strongest merged scale', async () => {
		const outcomes = [
			scheduler.submit(job({ coalesce: 50 }, 100, 0.2)),
			scheduler.submit(job({ coalesce: 50 }, 100, 0.8)),
		];
		await vi.runAllTimersAsync();
		await Promise.all(outcomes);
		// 0.8 from the stronger trigger + 0.15 × 1 merge.
		expect(played[0].scale).toBeCloseTo(0.95);
	});

	it('caps the scale at 1', async () => {
		const outcomes: Promise<Outcome<number>>[] = [];
		for (let i = 0; i < 4; i++) outcomes.push(scheduler.submit(job({ coalesce: 50 }, 100, 0.9)));
		await vi.runAllTimersAsync();
		await Promise.all(outcomes);
		expect(played[0].scale).toBe(1);
	});

	it('starts a new group after the window closes', async () => {
		const first = scheduler.submit(job({ coalesce: 40 }));
		await vi.advanceTimersByTimeAsync(60);
		const second = scheduler.submit(job({ coalesce: 40 }));
		await vi.advanceTimersByTimeAsync(60);
		expect((await first).policy).toBe('played');
		expect((await second).policy).toBe('played');
		expect(played).toHaveLength(2);
	});
});

describe('stop', () => {
	it('clears every queue and timer', async () => {
		const q = [scheduler.submit(job('queue')), scheduler.submit(job('queue'))];
		const c = scheduler.submit({ ...job({ coalesce: 80 }), key: 'other' });
		await vi.advanceTimersByTimeAsync(10);
		scheduler.stop();
		await vi.runAllTimersAsync();

		const results = await Promise.all([...q, c]);
		expect(results.map((o) => o.policy)).toEqual(['played', 'dropped', 'dropped']);
		expect(played).toHaveLength(1);
		expect(scheduler.isBusy('cue')).toBe(false);
	});

	it('lets the pattern play again straight after', async () => {
		await scheduler.submit(job('drop-if-busy', 1000));
		expect(scheduler.isBusy('cue')).toBe(true);
		scheduler.stop();
		expect((await scheduler.submit(job('drop-if-busy', 1000))).policy).toBe('played');
	});
});

describe('a run that played nothing', () => {
	it('leaves the pattern free for the next trigger', async () => {
		const silent: Job<number> = { ...job('drop-if-busy', 1000), didPlay: () => false };
		expect((await scheduler.submit(silent)).policy).toBe('played');
		expect(scheduler.isBusy('cue')).toBe(false);
		expect((await scheduler.submit(job('drop-if-busy', 1000))).policy).toBe('played');
	});

	it('still holds the pattern busy when the run played', async () => {
		const loud: Job<number> = { ...job('drop-if-busy', 1000), didPlay: () => true };
		await scheduler.submit(loud);
		expect(scheduler.isBusy('cue')).toBe(true);
	});
});

describe('failures and cancel', () => {
	it('is not busy after a play that failed', async () => {
		const failing: Job<number> = {
			...job('drop-if-busy', 1000),
			run: async () => {
				throw new Error('refused');
			},
		};
		await expect(scheduler.submit(failing)).rejects.toThrow('refused');
		expect(scheduler.isBusy('cue')).toBe(false);
		expect((await scheduler.submit(job('drop-if-busy', 1000))).policy).toBe('played');
	});

	it('settles a queued trigger whose run throws synchronously', async () => {
		await scheduler.submit(job('queue', 100));
		const throwing: Job<number> = {
			...job('queue', 100),
			run: () => {
				throw new Error('boom');
			},
		};
		const queued = scheduler.submit(throwing);
		const assertion = expect(queued).rejects.toThrow('boom');
		await vi.advanceTimersByTimeAsync(200);
		await assertion;
	});

	it('cancels one pattern and leaves the others', async () => {
		const queued = [scheduler.submit(job('queue')), scheduler.submit(job('queue'))];
		const other = await scheduler.submit({ ...job('drop-if-busy', 1000), key: 'other' });
		scheduler.cancel('cue');
		await vi.runAllTimersAsync();
		expect((await Promise.all(queued)).map((o) => o.policy)).toEqual(['played', 'dropped']);
		expect(other.policy).toBe('played');
		expect(scheduler.isBusy('other')).toBe(true);
		expect(scheduler.isBusy('cue')).toBe(false);
	});
});

describe('keys', () => {
	it('keeps busy state separate per pattern', async () => {
		await scheduler.submit(job('drop-if-busy', 1000));
		const other = await scheduler.submit({ ...job('drop-if-busy', 1000), key: 'other' });
		expect(other.policy).toBe('played');
	});
});

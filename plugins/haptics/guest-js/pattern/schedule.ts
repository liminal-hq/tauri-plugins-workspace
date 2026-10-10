// Per-pattern scheduler for the interrupt, queue, drop-if-busy and coalesce policies
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Policy } from './types';

/** What the scheduler did with a trigger. */
export type Decision = 'played' | 'queued' | 'dropped' | 'coalesced';

export type Job<R> = {
	/** Triggers with the same key share one busy state and one queue; this is the pattern id. */
	key: string;
	policy: Policy;
	/** How long the pattern runs; "busy" is tracked from this, never polled from native. */
	estimatedMs: number;
	/** The trigger's own scale, 0..1; a coalesced group raises it. */
	scale: number;
	/** Plays the pattern at `scale`. */
	run: (scale: number) => Promise<R>;
};

export type Outcome<R> = {
	policy: Decision;
	/** Absent when nothing was played for this trigger (dropped, coalesced or cancelled). */
	result?: R;
};

export const MAX_QUEUE = 4;
export const MAX_MERGES = 3;
export const MERGE_BOOST = 0.15;

type Group<R> = {
	job: Job<R>;
	merges: number;
	/** The strongest scale among the merged triggers. */
	scale: number;
	timer: ReturnType<typeof setTimeout>;
	settle: (outcome: Outcome<R>) => void;
};

type Waiting<R> = {
	timer: ReturnType<typeof setTimeout>;
	settle: (outcome: Outcome<R>) => void;
};

type KeyState<R> = {
	busyUntil: number;
	queue: Waiting<R>[];
	group?: Group<R>;
};

/**
 * Decides whether and when a compiled pattern runs. It only schedules: playing is the caller's
 * `run` function, so the same scheduler serves any backend. `stop()` clears every queue and timer.
 */
export class PatternScheduler {
	private state = new Map<string, KeyState<unknown>>();

	submit<R>(job: Job<R>): Promise<Outcome<R>> {
		const st = this.get<R>(job.key);
		const policy = job.policy;
		const now = Date.now();

		if (policy === 'interrupt') {
			this.clear(st);
			return this.play(st, job, job.scale, 'played');
		}

		if (policy === 'drop-if-busy') {
			if (now < st.busyUntil) return Promise.resolve({ policy: 'dropped' });
			return this.play(st, job, job.scale, 'played');
		}

		if (policy === 'queue') {
			if (now >= st.busyUntil && st.queue.length === 0) {
				return this.play(st, job, job.scale, 'played');
			}
			if (st.queue.length >= MAX_QUEUE) return Promise.resolve({ policy: 'dropped' });
			return this.enqueue(st, job);
		}

		return this.coalesce(st, job, policy.coalesce);
	}

	/** True while the pattern with this key is still within its estimated run time. */
	isBusy(key: string): boolean {
		const st = this.state.get(key);
		return st !== undefined && Date.now() < st.busyUntil;
	}

	/** Cancels one pattern's queued, merged and pending triggers and forgets its state. */
	cancel(key: string): void {
		const st = this.state.get(key);
		if (!st) return;
		this.clear(st);
		this.state.delete(key);
	}

	/** Cancels every queued, merged and pending trigger and clears the busy state. */
	stop(): void {
		for (const st of this.state.values()) this.clear(st);
		this.state.clear();
	}

	private get<R>(key: string): KeyState<R> {
		let st = this.state.get(key);
		if (!st) {
			st = { busyUntil: 0, queue: [] };
			this.state.set(key, st);
		}
		return st as KeyState<R>;
	}

	private clear<R>(st: KeyState<R>): void {
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

	private async play<R>(
		st: KeyState<R>,
		job: Job<R>,
		scale: number,
		policy: Decision
	): Promise<Outcome<R>> {
		st.busyUntil = Date.now() + job.estimatedMs;
		try {
			const result = await job.run(Math.min(1, scale));
			return { policy, result };
		} catch (err) {
			// A play that failed never ran, so the pattern is not busy.
			st.busyUntil = 0;
			throw err;
		}
	}

	private enqueue<R>(st: KeyState<R>, job: Job<R>): Promise<Outcome<R>> {
		const start = Math.max(Date.now(), st.busyUntil);
		const wait = start - Date.now();
		// Reserve the slot now so a later trigger queues behind this one.
		st.busyUntil = start + job.estimatedMs;

		return new Promise<Outcome<R>>((resolve, reject) => {
			const entry: Waiting<R> = {
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

	private coalesce<R>(st: KeyState<R>, job: Job<R>, windowMs: number): Promise<Outcome<R>> {
		const group = st.group;
		if (group) {
			// Merge into the open group; once it holds three merges later triggers are absorbed
			// without a further boost.
			if (group.merges < MAX_MERGES) group.merges++;
			group.scale = Math.max(group.scale, job.scale);
			return Promise.resolve({ policy: 'coalesced' });
		}

		return new Promise<Outcome<R>>((resolve, reject) => {
			const created: Group<R> = {
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

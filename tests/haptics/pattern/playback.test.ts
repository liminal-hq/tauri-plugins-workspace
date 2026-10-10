// Unit tests for the playback model that compile reports are read from
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { placeSteps, playbackEnd } from '../../../plugins/haptics/guest-js/pattern/playback';
import type { CompiledStep } from '../../../plugins/haptics/guest-js/types';
import { midRange } from './__fixtures__/capabilities';

const step = (atMs: number, effect: CompiledStep['request']['effect']): CompiledStep => ({
	atMs,
	request: { effect },
});

describe('placeSteps', () => {
	it('places composition primitives after their delays', () => {
		const segments = placeSteps(
			[
				step(100, {
					type: 'composition',
					steps: [
						{ kind: 'primitive', primitive: 'click', scale: 0.5 },
						{ kind: 'primitive', primitive: 'tick', scale: 1, delayMs: 20 },
					],
				}),
			],
			midRange
		);
		const clickMs = segments[0].durationMs;
		expect(segments[0]).toMatchObject({ atMs: 100, amplitude: 0.5, tier: 3, label: 'click' });
		expect(segments[1]).toMatchObject({ atMs: 100 + clickMs + 20, label: 'tick' });
	});

	it('reads the on phases of a timings-only waveform at full strength', () => {
		const segments = placeSteps(
			[step(0, { type: 'waveform', timingsMs: [10, 20, 5, 30] })],
			midRange
		);
		expect(segments).toEqual([
			{ atMs: 10, durationMs: 20, amplitude: 1, tier: 1 },
			{ atMs: 35, durationMs: 30, amplitude: 1, tier: 1 },
		]);
	});

	it('gives each timing of an amplitude waveform its own strength and skips silent ones', () => {
		const segments = placeSteps(
			[step(0, { type: 'waveform', timingsMs: [0, 20, 10, 20], amplitudes: [0, 255, 0, 51] })],
			midRange
		);
		expect(segments).toEqual([
			{ atMs: 0, durationMs: 20, amplitude: 1, tier: 2 },
			{ atMs: 30, durationMs: 20, amplitude: 0.2, tier: 2 },
		]);
	});

	it('lists every envelope point, including silent ones', () => {
		const segments = placeSteps(
			[
				step(5, {
					type: 'envelopeWaveform',
					controlPoints: [
						{ amplitude: 0, frequencyHz: 100, durationMs: 10 },
						{ amplitude: 0.5, frequencyHz: 100, durationMs: 20 },
					],
				}),
			],
			midRange
		);
		expect(segments.map((s) => [s.atMs, s.durationMs, s.amplitude])).toEqual([
			[5, 10, 0],
			[15, 20, 0.5],
		]);
	});

	it('cuts a bar where the next step takes over', () => {
		const segments = placeSteps(
			[
				step(0, { type: 'oneshot', durationMs: 100, amplitude: 255 }),
				step(40, { type: 'oneshot', durationMs: 10, amplitude: 255 }),
			],
			midRange
		);
		expect(segments.map((s) => [s.atMs, s.durationMs])).toEqual([
			[0, 40],
			[40, 10],
		]);
		expect(playbackEnd(segments)).toBe(50);
	});

	it('ends at zero when nothing plays', () => {
		expect(playbackEnd(placeSteps([], midRange))).toBe(0);
	});
});

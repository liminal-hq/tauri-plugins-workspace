// Seed cues for the compiler tests: the Lieutenant Fizz cue table
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { PATTERN_FORMAT } from '../../../../plugins/phone-haptics/guest-js/pattern/types';
import type {
	CurvePoint,
	Pattern,
	PatternEvent,
} from '../../../../plugins/phone-haptics/guest-js/pattern/types';

const curve3 = (a: number, b: number, c: number): CurvePoint[] => [
	{ t: 0, v: a },
	{ t: 0.5, v: b },
	{ t: 1, v: c },
];
const transient = (at: number, intensity: number, sharpness: number): PatternEvent => ({
	type: 'transient',
	at,
	intensity,
	sharpness,
});
const hum = (
	at: number,
	duration: number,
	intensity: CurvePoint[],
	sharpness: number
): PatternEvent => ({ type: 'continuous', at, duration, intensity, sharpness });

const cue = (id: string, events: PatternEvent[]): Pattern => ({
	format: PATTERN_FORMAT,
	id,
	events,
});

export const seedCues: Record<string, Pattern> = {
	jump: cue('jump', [transient(0, 0.5, 0.7)]),
	fizzFired: cue('fizzFired', [transient(0, 0.35, 0.9)]),
	stomp: cue('stomp', [transient(0, 0.7, 0.2)]),
	hurt: cue('hurt', [transient(0, 1, 0.6), hum(20, 120, curve3(0.9, 0.45, 0), 0.1)]),
	snack: cue('snack', [transient(0, 0.3, 0.9)]),
	creamSoda: cue('creamSoda', [
		transient(0, 0.3, 0.6),
		transient(30, 0.5, 0.6),
		transient(60, 0.7, 0.6),
	]),
	extraLife: cue('extraLife', [hum(0, 180, curve3(0.3, 0.7, 0.3), 0.5)]),
	levelCleared: cue('levelCleared', [
		hum(0, 120, curve3(0.1, 0.5, 0.9), 0.5),
		transient(130, 0.8, 0.7),
	]),
	bossSlam: cue('bossSlam', [hum(0, 220, curve3(1, 0.65, 0.3), 0.05)]),
};

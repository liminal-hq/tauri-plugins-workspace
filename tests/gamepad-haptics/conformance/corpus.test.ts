// Checks the shared frames corpus against the test-only copy of the Rust rules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type { MotorFrame } from '../../../plugins/gamepad-haptics/guest-js/pattern/pad-compile';
import { MAX_FRAMES, checkFrames, checkScale } from './frame-rules';

type Case = {
	name: string;
	args: { padId: string; scale?: number; frames: MotorFrame[] };
	expect: { error?: string; silent?: string; ok?: unknown };
};

const corpus = JSON.parse(
	readFileSync(
		new URL('../../../plugins/gamepad-haptics/tests/conformance/frames.json', import.meta.url),
		'utf8'
	)
) as {
	constants: { maxFrames: number };
	defaults: { limits: { maxDurationMs: number; maxContinuousMs: number } };
	cases: Case[];
};

describe('frames corpus', () => {
	it('uses the same constants as the guest', () => {
		expect(corpus.constants.maxFrames).toBe(MAX_FRAMES);
	});

	for (const c of corpus.cases) {
		it(c.name, () => {
			const problem =
				checkScale(c.args.scale) ?? checkFrames(c.args.frames, corpus.defaults.limits);
			if (c.expect.error !== undefined) {
				expect(problem, 'the rules should reject this').toBeDefined();
				expect(problem).toContain(c.expect.error.replace(/^frames\[0\]: /, '').split(' (')[0]);
			} else {
				expect(problem, 'the rules should accept this').toBeUndefined();
			}
		});
	}
});

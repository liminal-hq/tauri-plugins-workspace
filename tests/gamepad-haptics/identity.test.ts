// Unit tests for matching a game's pad to one the plugin can drive
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { parseWebGamepadId, resolvePad } from '../../plugins/gamepad-haptics/guest-js/identity';
import type { PadInfo } from '../../plugins/gamepad-haptics/guest-js/types';

const pad = (over: Partial<PadInfo>): PadInfo => ({
	id: 'gamepad:0',
	slot: 0,
	name: 'Sony PLAYSTATION(R)3 Controller',
	vendorId: 0x054c,
	productId: 0x0268,
	serial: '60:38:0e:dc:52:da',
	transport: 'usb',
	guid: '030000004c0500006802000011810000',
	motors: 2,
	triggers: false,
	lightBinary: false,
	topTier: 2,
	backend: 'evdev',
	...over,
});

describe('parseWebGamepadId', () => {
	it('reads Chromium and Firefox ids and gives up on Safari ids', () => {
		expect(
			parseWebGamepadId('Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 09cc)')
		).toEqual({ vendorId: 0x054c, productId: 0x09cc });
		expect(parseWebGamepadId('054c-0268-Sony PLAYSTATION(R)3 Controller')).toEqual({
			vendorId: 0x054c,
			productId: 0x0268,
		});
		expect(parseWebGamepadId('Wireless Controller Extended Gamepad')).toBeUndefined();
	});
});

describe('resolvePad', () => {
	const one = [pad({})];
	const two = [pad({}), pad({ id: 'gamepad:1', slot: 1, serial: 'aa:bb' })];

	it('matches a Web gamepad by vendor and product', () => {
		const r = resolvePad(one, {
			gamepad: { id: 'PS3 (STANDARD GAMEPAD Vendor: 054c Product: 0268)' },
		});
		expect(r.matches.map((p) => p.id)).toEqual(['gamepad:0']);
		expect(r.ambiguous).toBe(false);
	});

	it('falls back to the name when the id has no vendor or product', () => {
		const r = resolvePad(one, {
			gamepad: { id: 'Sony PLAYSTATION(R)3 Controller Extended Gamepad' },
		});
		expect(r.matches).toHaveLength(1);
	});

	it('matches a GUID in any case', () => {
		expect(resolvePad(one, { guid: one[0].guid.toUpperCase() }).matches).toHaveLength(1);
		expect(resolvePad(one, { guid: 'nope' }).matches).toHaveLength(0);
	});

	it('reports identical pads as ambiguous rather than choosing', () => {
		const r = resolvePad(two, { vendorId: 0x054c, productId: 0x0268 });
		expect(r.ambiguous).toBe(true);
		expect(r.matches).toHaveLength(2);
	});

	it('uses the serial to tell identical pads apart', () => {
		const r = resolvePad(two, { vendorId: 0x054c, productId: 0x0268, serial: 'aa:bb' });
		expect(r.matches.map((p) => p.id)).toEqual(['gamepad:1']);
		expect(r.ambiguous).toBe(false);
	});
});

// Matches a pad the game knows (a Web Gamepad, an SDL GUID, a vendor and product) to a pad the plugin can drive
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PadInfo } from './types';

export type PadHint =
	| { gamepad: { id: string; index?: number } }
	| { guid: string }
	| { vendorId: number; productId: number; serial?: string };

export type Resolved = {
	/** Pads that fit the hint, best first. */
	matches: PadInfo[];
	/** True when more than one pad fits equally well, so the caller should ask the player. */
	ambiguous: boolean;
};

const hex = (s: string) => Number.parseInt(s, 16);

/**
 * Reads the vendor and product ids out of a Web Gamepad `id`. Chromium writes
 * `Name (STANDARD GAMEPAD Vendor: 054c Product: 0268)`, Firefox `054c-0268-Name`; Safari writes
 * neither, so it returns undefined and the caller falls back to the name.
 */
export function parseWebGamepadId(id: string): { vendorId: number; productId: number } | undefined {
	const chromium = /Vendor:\s*([0-9a-f]{4})\s+Product:\s*([0-9a-f]{4})/i.exec(id);
	if (chromium) return { vendorId: hex(chromium[1]), productId: hex(chromium[2]) };
	const firefox = /^([0-9a-f]{4})-([0-9a-f]{4})-/i.exec(id);
	if (firefox) return { vendorId: hex(firefox[1]), productId: hex(firefox[2]) };
	return undefined;
}

const normaliseName = (s: string) =>
	s
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, ' ')
		.trim();

/** The pads that fit `hint`, never guessing between identical ones. */
export function resolvePad(pads: PadInfo[], hint: PadHint): Resolved {
	let matches: PadInfo[];
	if ('guid' in hint) {
		matches = pads.filter((p) => p.guid.toLowerCase() === hint.guid.toLowerCase());
	} else if ('vendorId' in hint) {
		matches = pads.filter((p) => p.vendorId === hint.vendorId && p.productId === hint.productId);
		if (hint.serial !== undefined) {
			const exact = matches.filter((p) => p.serial === hint.serial);
			if (exact.length > 0) matches = exact;
		}
	} else {
		const ids = parseWebGamepadId(hint.gamepad.id);
		if (ids) {
			matches = pads.filter((p) => p.vendorId === ids.vendorId && p.productId === ids.productId);
		} else {
			const name = normaliseName(hint.gamepad.id);
			matches = pads.filter((p) => {
				const n = normaliseName(p.name);
				return n.length > 0 && (name.includes(n) || n.includes(name));
			});
		}
	}
	return { matches, ambiguous: matches.length > 1 };
}

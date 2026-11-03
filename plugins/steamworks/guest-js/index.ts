// Typed guest-side wrappers for the steamworks plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
	Achievement,
	AchievementStoredPayload,
	CloudFile,
	CloudQuota,
	CloudStatus,
	OverlayPage,
	OverlayToggledPayload,
	Player,
	StatKind,
	StatsReceivedPayload,
	StatsStoredPayload,
	StatValue,
	SteamStatus,
} from './types';

export type {
	Achievement,
	AchievementStoredPayload,
	BackendKind,
	OverlayPage,
	OverlayToggledPayload,
	Player,
	StatKind,
	StatsReceivedPayload,
	StatsStoredPayload,
	StatValue,
	SteamStatus,
	UnavailableReason,
} from './types';

const PREFIX = 'plugin:steamworks|';
const CLOUD_NAME_HEADER = 'x-steamworks-cloud-name';

export const STATS_RECEIVED_EVENT = 'steamworks://stats-received';
export const STATS_STORED_EVENT = 'steamworks://stats-stored';
export const ACHIEVEMENT_STORED_EVENT = 'steamworks://achievement-stored';
export const OVERLAY_TOGGLED_EVENT = 'steamworks://overlay-toggled';

/** True when Steam initialised, or the in-memory mock is active. Never rejects. */
export function isAvailable(): Promise<boolean> {
	return invoke<boolean>(`${PREFIX}is_available`);
}

/** Says whether Steam is usable and, if not, why. Never rejects. */
export function getStatus(): Promise<SteamStatus> {
	return invoke<SteamStatus>(`${PREFIX}get_status`);
}

export function getPlayer(): Promise<Player> {
	return invoke<Player>(`${PREFIX}get_player`);
}

export function getAchievement(id: string): Promise<Achievement> {
	return invoke<Achievement>(`${PREFIX}get_achievement`, { id });
}

/** Unlocks an achievement and stores it so Steam shows its notification. */
export function unlockAchievement(id: string): Promise<void> {
	return invoke<void>(`${PREFIX}unlock_achievement`, { id });
}

/** Re-locks an achievement. Meant for development builds; it needs its own permission. */
export function clearAchievement(id: string): Promise<void> {
	return invoke<void>(`${PREFIX}clear_achievement`, { id });
}

/** Shows Steam's progress toast for a counted achievement. `current` must not exceed `max`. */
export function indicateProgress(id: string, current: number, max: number): Promise<void> {
	return invoke<void>(`${PREFIX}indicate_achievement_progress`, { id, current, max });
}

export async function getStat(name: string, kind: StatKind = 'int'): Promise<number> {
	const stat = await invoke<StatValue>(`${PREFIX}get_stat`, { name, kind });
	return stat.value;
}

/**
 * Sets a stat without storing it, so several writes can be stored together with `storeStats`.
 * Without `kind`, a whole number is sent as an int and anything else as a float. Pass `kind` for a
 * float stat that currently holds a whole number.
 */
export function setStat(name: string, value: number, kind?: StatKind): Promise<void> {
	const resolved: StatKind = kind ?? (Number.isInteger(value) ? 'int' : 'float');
	return invoke<void>(`${PREFIX}set_stat`, { name, value: { kind: resolved, value } });
}

export function storeStats(): Promise<void> {
	return invoke<void>(`${PREFIX}store_stats`);
}

/**
 * Sets the rich presence keys friends see. A `null` value removes that key. Steam allows 20 keys of
 * up to 64 bytes, with values of up to 256 bytes. Needs the `allow-presence` permission.
 */
export function setRichPresence(entries: Record<string, string | null>): Promise<void> {
	return invoke<void>(`${PREFIX}set_rich_presence`, { entries });
}

export function clearRichPresence(): Promise<void> {
	return invoke<void>(`${PREFIX}clear_rich_presence`);
}

/**
 * Opens a page of the Steam overlay. Needs the `allow-overlay` permission, and Steam's overlay
 * must be able to hook the window; see the README for Windows.
 */
export function activateOverlay(page: OverlayPage): Promise<void> {
	return invoke<void>(`${PREFIX}activate_overlay`, { page });
}

/** Opens an `http` or `https` address in the overlay's browser. Needs `allow-overlay`. */
export function activateOverlayToWebPage(url: string): Promise<void> {
	return invoke<void>(`${PREFIX}activate_overlay_to_web_page`, { url });
}

/**
 * Writes a whole file to Steam Cloud, replacing any file of that name. Names use ASCII letters,
 * digits and ` ._-`, with `/` between folders, and files are at most 100 MiB. Strings are written as
 * UTF-8. Needs `allow-cloud`. Prefer Auto-Cloud for save games; this is for apps that cannot use it.
 */
export function cloudWrite(name: string, data: Uint8Array | string): Promise<void> {
	const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
	return invoke<void>(`${PREFIX}cloud_write`, bytes, { headers: { [CLOUD_NAME_HEADER]: name } });
}

/** Reads a whole Cloud file. Rejects when the file does not exist. */
export async function cloudRead(name: string): Promise<Uint8Array> {
	const data = await invoke<ArrayBuffer | number[]>(`${PREFIX}cloud_read`, { name });
	return new Uint8Array(data);
}

/** Reads a whole Cloud file as UTF-8 text. */
export async function cloudReadText(name: string): Promise<string> {
	return new TextDecoder().decode(await cloudRead(name));
}

/** Deletes a Cloud file. Rejects when the file does not exist. */
export function cloudDelete(name: string): Promise<void> {
	return invoke<void>(`${PREFIX}cloud_delete`, { name });
}

export function cloudList(): Promise<CloudFile[]> {
	return invoke<CloudFile[]>(`${PREFIX}cloud_list`);
}

export function cloudQuota(): Promise<CloudQuota> {
	return invoke<CloudQuota>(`${PREFIX}cloud_quota`);
}

/** Says whether the player's account and this app allow Steam Cloud. */
export function cloudStatus(): Promise<CloudStatus> {
	return invoke<CloudStatus>(`${PREFIX}cloud_status`);
}

/** Needs `core:event:default` as well as the plugin's permissions. */
export function onStatsReceived(
	handler: (_payload: StatsReceivedPayload) => void
): Promise<UnlistenFn> {
	return listen<StatsReceivedPayload>(STATS_RECEIVED_EVENT, (e) => handler(e.payload));
}

export function onStatsStored(
	handler: (_payload: StatsStoredPayload) => void
): Promise<UnlistenFn> {
	return listen<StatsStoredPayload>(STATS_STORED_EVENT, (e) => handler(e.payload));
}

export function onAchievementStored(
	handler: (_payload: AchievementStoredPayload) => void
): Promise<UnlistenFn> {
	return listen<AchievementStoredPayload>(ACHIEVEMENT_STORED_EVENT, (e) => handler(e.payload));
}

/** Fires when the Steam overlay opens or closes. */
export function onOverlayToggled(
	handler: (_payload: OverlayToggledPayload) => void
): Promise<UnlistenFn> {
	return listen<OverlayToggledPayload>(OVERLAY_TOGGLED_EVENT, (e) => handler(e.payload));
}

/** True when a rejection means Steam is not usable, as opposed to a bad id or a failed call. */
export function isUnavailableError(error: unknown): boolean {
	const message = error instanceof Error ? error.message : String(error);
	return message.startsWith('unavailable:');
}

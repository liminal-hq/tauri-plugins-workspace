// Types shared by the steamworks guest API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export type BackendKind = 'steam' | 'mock';

export type UnavailableReason =
	| 'notCompiled'
	| 'unsupportedPlatform'
	| 'steamNotRunning'
	| 'initFailed'
	| 'restartRequired'
	| 'shutdown';

export type SteamStatus =
	| { state: 'available'; backend: BackendKind; appId: number }
	| { state: 'unavailable'; reason: UnavailableReason; message: string | null };

export interface Player {
	/** The 64-bit Steam ID as a decimal string, because JavaScript numbers lose precision. */
	steamId: string;
	personaName: string;
	/** The language the Steam client uses for this app, such as `english`. */
	appLanguage: string;
}

export interface Achievement {
	id: string;
	achieved: boolean;
	/** Unix time of the unlock, when Steam reports one. */
	unlockTime: number | null;
}

export type StatKind = 'int' | 'float';

export type StatValue = { kind: 'int'; value: number } | { kind: 'float'; value: number };

export interface StatsReceivedPayload {
	success: boolean;
	steamId: string;
}

export interface StatsStoredPayload {
	success: boolean;
}

export interface AchievementStoredPayload {
	id: string;
	/** Progress when the event came from `indicateProgress`; 0 once an achievement unlocks. */
	current: number;
	max: number;
}

export type OverlayPage =
	| 'friends'
	| 'community'
	| 'players'
	| 'settings'
	| 'officialgamegroup'
	| 'stats'
	| 'achievements';

export interface OverlayToggledPayload {
	/** True while the overlay is open. */
	active: boolean;
}

export interface CloudFile {
	name: string;
	size: number;
	/** Unix time the file was last written. */
	timestamp: number;
}

export interface CloudQuota {
	totalBytes: number;
	availableBytes: number;
}

export interface CloudStatus {
	/** Whether the player's account allows Steam Cloud. */
	accountEnabled: boolean;
	/** Whether the player has Steam Cloud turned on for this app. */
	appEnabled: boolean;
}

export interface DlcInfo {
	appId: number;
	name: string;
	/** Whether the player can get it, as Steam reports availability. */
	available: boolean;
	/** Whether the player owns it and has it installed. */
	installed: boolean;
}

export interface DeviceInfo {
	/** Whether the app runs on a Steam Deck. */
	steamDeck: boolean;
	/** Whether Steam is in Big Picture mode. */
	bigPicture: boolean;
}

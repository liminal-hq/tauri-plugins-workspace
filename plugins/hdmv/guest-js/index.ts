import { invoke } from '@tauri-apps/api/core';

// -- Response types --

export interface DiscSummary {
	titleCount: number;
	version: string;
	firstPlayObjectId: number;
	topMenuObjectId: number;
}

export interface TitleInfo {
	index: number;
	objectType: string;
	playbackType: string;
	objectIdRef: number;
}

export interface PlaylistInfo {
	index: number;
	playItemCount: number;
	chapterCount: number;
	durationSeconds: number;
}

export interface PlayItemInfo {
	clipId: string;
	codecId: string;
	inTimeSeconds: number;
	outTimeSeconds: number;
}

export interface ChapterInfo {
	index: number;
	playItemRef: number;
	timeSeconds: number;
}

export interface PlaylistDetail {
	index: number;
	playItems: PlayItemInfo[];
	chapters: ChapterInfo[];
}

export interface MenuStateSnapshot {
	hasMenu: boolean;
	currentPageId: number | null;
	selectedButtonId: number | null;
	popupVisible: boolean;
}

export type NavEvent =
	| { type: 'PlayTitle'; titleId: number }
	| { type: 'PlayPlaylist'; playlistId: number }
	| { type: 'PlayPlaylistItem'; playlistId: number; playItemId: number }
	| { type: 'SeekPlayMark'; playlistId: number; playMarkId: number }
	| { type: 'LinkPlayItem'; playItemId: number }
	| { type: 'LinkPlayMark'; playMarkId: number }
	| { type: 'PlayStop' }
	| { type: 'StillOn' }
	| { type: 'StillOff' }
	| { type: 'SetButtonPage'; pageId: number }
	| { type: 'EnableButton'; buttonId: number }
	| { type: 'DisableButton'; buttonId: number }
	| { type: 'PopupOff' }
	| { type: 'SetOutputMode'; mode: number }
	| { type: 'SetStream'; streamType: number; streamId: number }
	| { type: 'SetNvTimer'; timerId: number; value: number };

export interface TitleSpecDto {
	clipId: string;
	codecId: string;
	durationSeconds: number;
	chapters: number[];
}

export interface DiscBuildConfig {
	outputPath: string;
	titles: TitleSpecDto[];
}

export interface SceneData {
	/** Composition width in pixels. */
	width: number;
	/** Composition height in pixels. */
	height: number;
	/** Base64-encoded raw palette segment bytes. */
	paletteSegments: string[];
	/** Base64-encoded raw object segment bytes. */
	objectSegments: string[];
}

// -- Session lifecycle --

/**
 * Open a BDMV folder and return a session ID for subsequent commands.
 */
export async function openDisc(path: string): Promise<string> {
	return await invoke<string>('plugin:hdmv|hdmv_open_disc', { path });
}

/**
 * Close a disc session and free its resources.
 */
export async function closeDisc(sessionId: string): Promise<void> {
	await invoke<void>('plugin:hdmv|hdmv_close_disc', { sessionId });
}

// -- Disc inspection --

/**
 * Get a summary of the disc's top-level structure.
 */
export async function getDiscInfo(sessionId: string): Promise<DiscSummary> {
	return await invoke<DiscSummary>('plugin:hdmv|hdmv_get_disc_info', { sessionId });
}

/**
 * List all title entries from the disc index.
 */
export async function listTitles(sessionId: string): Promise<TitleInfo[]> {
	return await invoke<TitleInfo[]>('plugin:hdmv|hdmv_list_titles', { sessionId });
}

/**
 * List playlist summaries.
 */
export async function listPlaylists(sessionId: string): Promise<PlaylistInfo[]> {
	return await invoke<PlaylistInfo[]>('plugin:hdmv|hdmv_list_playlists', { sessionId });
}

/**
 * Get full playlist detail including play items and chapters.
 */
export async function getPlaylist(
	sessionId: string,
	playlistIndex: number
): Promise<PlaylistDetail> {
	return await invoke<PlaylistDetail>('plugin:hdmv|hdmv_get_playlist', {
		sessionId,
		playlistIndex,
	});
}

// -- Navigation --

/**
 * Execute the First Play object and return initial navigation events.
 */
export async function startNavigation(sessionId: string): Promise<NavEvent[]> {
	return await invoke<NavEvent[]>('plugin:hdmv|hdmv_start_navigation', { sessionId });
}

/**
 * Load an interactive menu scene from externally-parsed IGS data.
 *
 * Must be called before scene-dependent commands (sendKey, mouseMove,
 * mouseClick, renderPreview) will work.
 */
export async function loadScene(sessionId: string, sceneData: SceneData): Promise<void> {
	await invoke<void>('plugin:hdmv|hdmv_load_scene', { sessionId, sceneData });
}

/**
 * Send a remote key input and return resulting navigation events.
 */
export async function sendKey(sessionId: string, key: string): Promise<NavEvent[]> {
	return await invoke<NavEvent[]>('plugin:hdmv|hdmv_send_key', { sessionId, key });
}

/**
 * Hit-test mouse position, return button ID if hovering over one.
 */
export async function mouseMove(sessionId: string, x: number, y: number): Promise<number | null> {
	return await invoke<number | null>('plugin:hdmv|hdmv_mouse_move', { sessionId, x, y });
}

/**
 * Click at a position and return resulting navigation events.
 */
export async function mouseClick(sessionId: string, x: number, y: number): Promise<NavEvent[]> {
	return await invoke<NavEvent[]>('plugin:hdmv|hdmv_mouse_click', { sessionId, x, y });
}

// -- Rendering --

/**
 * Render the current menu page as a base64-encoded PNG.
 */
export async function renderPreview(sessionId: string, maxWidth: number): Promise<string> {
	return await invoke<string>('plugin:hdmv|hdmv_render_preview', { sessionId, maxWidth });
}

/**
 * Get a snapshot of the current menu state.
 */
export async function getMenuState(sessionId: string): Promise<MenuStateSnapshot> {
	return await invoke<MenuStateSnapshot>('plugin:hdmv|hdmv_get_menu_state', { sessionId });
}

// -- Authoring --

/**
 * Build a new BDMV disc structure from configuration.
 */
export async function buildDisc(config: DiscBuildConfig): Promise<void> {
	await invoke<void>('plugin:hdmv|hdmv_build_disc', { config });
}

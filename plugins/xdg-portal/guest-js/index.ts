// Exposes typed guest-side wrappers for plugin command invocation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { AccentColour } from './bindings/AccentColour';
import type { AvailabilityInfo } from './bindings/AvailabilityInfo';
import type { ColourScheme } from './bindings/ColourScheme';
import type { DesktopEnvironment } from './bindings/DesktopEnvironment';
import type { ThemeInfo } from './bindings/ThemeInfo';

const PREFIX = 'plugin:xdg-portal|';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

export const portal = {
	checkAvailability: () => cmd<AvailabilityInfo>('check_availability'),
	getThemeInfo: () => cmd<ThemeInfo>('get_theme_info'),
};

export type { ThemeInfo, ColourScheme, DesktopEnvironment, AccentColour, AvailabilityInfo };

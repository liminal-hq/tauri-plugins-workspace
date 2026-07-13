import { describe, expect, it } from 'vitest';
import { syncPathDependencyVersions } from '../scripts/sync-internal-dependency-versions.js';

describe('syncPathDependencyVersions', () => {
	it('rewrites a stale version requirement to match the resolved version', () => {
		const content = 'tauri-plugin-xdg-portal = { path = "../xdg-portal", version = "0.0.0" }\n';

		const { content: updated, changes } = syncPathDependencyVersions(content, () => '0.1.0');

		expect(updated).toBe(
			'tauri-plugin-xdg-portal = { path = "../xdg-portal", version = "0.1.0" }\n'
		);
		expect(changes).toEqual([{ path: '../xdg-portal', from: '0.0.0', to: '0.1.0' }]);
	});

	it('leaves the line untouched when the version already matches', () => {
		const content = 'tauri-plugin-xdg-portal = { path = "../xdg-portal", version = "0.1.0" }\n';

		const { content: updated, changes } = syncPathDependencyVersions(content, () => '0.1.0');

		expect(updated).toBe(content);
		expect(changes).toEqual([]);
	});

	it('ignores path dependencies with no version requirement', () => {
		const content = 'tauri-plugin-material-you = { path = "../../../plugins/material-you" }\n';

		const { content: updated, changes } = syncPathDependencyVersions(content, () => '9.9.9');

		expect(updated).toBe(content);
		expect(changes).toEqual([]);
	});

	it('ignores dependencies with no path (registry deps)', () => {
		const content = 'serde = { version = "1", features = ["derive"] }\n';

		const { content: updated, changes } = syncPathDependencyVersions(content, () => '9.9.9');

		expect(updated).toBe(content);
		expect(changes).toEqual([]);
	});

	it('skips a dependency when the resolver returns null (unresolvable path)', () => {
		const content =
			'some-external-crate = { path = "../../not-in-workspace", version = "0.1.0" }\n';

		const { content: updated, changes } = syncPathDependencyVersions(content, () => null);

		expect(updated).toBe(content);
		expect(changes).toEqual([]);
	});

	it('preserves field order and only rewrites the version value, whatever order path/version appear in', () => {
		const content = 'foo = { version = "0.0.0", path = "../foo", features = ["x"] }\n';

		const { content: updated, changes } = syncPathDependencyVersions(content, () => '0.2.0');

		expect(updated).toBe('foo = { version = "0.2.0", path = "../foo", features = ["x"] }\n');
		expect(changes).toEqual([{ path: '../foo', from: '0.0.0', to: '0.2.0' }]);
	});

	it('updates multiple dependency lines in the same file independently', () => {
		const content = [
			'foo = { path = "../foo", version = "0.0.0" }',
			'bar = { path = "../bar", version = "1.0.0" }',
			'',
		].join('\n');

		const { content: updated, changes } = syncPathDependencyVersions(content, (depPath) =>
			depPath === '../foo' ? '0.1.0' : null
		);

		expect(updated).toBe(
			[
				'foo = { path = "../foo", version = "0.1.0" }',
				'bar = { path = "../bar", version = "1.0.0" }',
				'',
			].join('\n')
		);
		expect(changes).toEqual([{ path: '../foo', from: '0.0.0', to: '0.1.0' }]);
	});
});

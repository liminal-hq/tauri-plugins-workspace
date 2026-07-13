#!/usr/bin/env node
// Syncs `version = "..."` requirements on internal path dependencies (e.g.
// `desktop-integration`'s dependency on `xdg-portal`) to match the actual current
// version of the crate they point at.
//
// Covector bumps each package's own manifest version on release, but has no
// awareness of other packages in the workspace that pin a version requirement on
// it via a path dependency — so those requirements go stale the moment the
// dependency's version changes, and the next `cargo build` fails with a version
// resolution error. This script closes that gap; see `covector-version-or-publish.yml`,
// which runs it right after `covector version`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const SKIP_DIRS = new Set(['node_modules', 'target', '.git']);

// Matches a single-line inline-table dependency, e.g.:
//   tauri-plugin-xdg-portal = { path = "../xdg-portal", version = "0.0.0" }
// Captures the `{ ... }` body so `path` and `version` can be pulled out
// independently of their order within the table.
const INLINE_TABLE_DEP = /^([ \t]*[A-Za-z0-9_-]+\s*=\s*\{)([^{}\n]*)(\})/gm;

/**
 * Rewrites any `{ path = "...", version = "..." }` dependency in `content` whose
 * `version` doesn't match what `resolveVersion(path)` reports for the crate at
 * that path. Leaves everything else — formatting, key order, unrelated deps —
 * untouched, so the diff is minimal.
 *
 * @param {string} content - Cargo.toml file contents.
 * @param {(relativePath: string) => string | null} resolveVersion - Returns the
 *   current version of the crate at the given path (relative to the Cargo.toml
 *   being processed), or null if it can't be resolved (e.g. an external path).
 * @returns {{ content: string, changes: { path: string, from: string, to: string }[] }}
 */
export function syncPathDependencyVersions(content, resolveVersion) {
	const changes = [];

	const updated = content.replace(INLINE_TABLE_DEP, (full, prefix, inner, suffix) => {
		const pathMatch = inner.match(/\bpath\s*=\s*"([^"]+)"/);
		const versionMatch = inner.match(/\bversion\s*=\s*"([^"]+)"/);
		if (!pathMatch || !versionMatch) return full;

		const depPath = pathMatch[1];
		const currentVersion = versionMatch[1];
		const targetVersion = resolveVersion(depPath);
		if (!targetVersion || targetVersion === currentVersion) return full;

		changes.push({ path: depPath, from: currentVersion, to: targetVersion });
		const newInner = inner.replace(/\bversion\s*=\s*"[^"]+"/, `version = "${targetVersion}"`);
		return `${prefix}${newInner}${suffix}`;
	});

	return { content: updated, changes };
}

function findCargoTomlFiles(root) {
	const results = [];
	(function walk(dir) {
		for (const entry of readdirSync(dir, { withFileTypes: true })) {
			if (entry.isDirectory()) {
				if (!SKIP_DIRS.has(entry.name)) walk(join(dir, entry.name));
			} else if (entry.name === 'Cargo.toml') {
				results.push(join(dir, entry.name));
			}
		}
	})(root);
	return results;
}

function readPackageVersion(cargoTomlPath) {
	let content;
	try {
		content = readFileSync(cargoTomlPath, 'utf8');
	} catch {
		return null;
	}
	// A line starting with `version = "..."` only ever occurs in `[package]` —
	// dependency versions always appear mid-line inside an inline table
	// (`key = { ..., version = "...", ... }`), never at the start of a line.
	const match = content.match(/^version\s*=\s*"([^"]+)"/m);
	return match ? match[1] : null;
}

function main() {
	const root = process.cwd();
	let totalChanges = 0;

	for (const cargoTomlPath of findCargoTomlFiles(root)) {
		const content = readFileSync(cargoTomlPath, 'utf8');
		const { content: updated, changes } = syncPathDependencyVersions(content, (depPath) =>
			readPackageVersion(join(dirname(cargoTomlPath), depPath, 'Cargo.toml'))
		);

		if (changes.length === 0) continue;

		writeFileSync(cargoTomlPath, updated);
		totalChanges += changes.length;
		for (const change of changes) {
			console.log(
				`${relative(root, cargoTomlPath)}: ${change.path} version requirement ${change.from} -> ${change.to}`
			);
		}
	}

	console.log(
		totalChanges === 0
			? 'No internal dependency versions needed syncing.'
			: `Synced ${totalChanges} internal dependency version requirement(s).`
	);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
	main();
}

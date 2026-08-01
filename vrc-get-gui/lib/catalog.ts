import {
	commands,
	type TauriBasePackageInfo,
	type TauriRemoteRepositoryInfo,
} from "@/lib/bindings";

export const VRC_GET_CATALOG_URL =
	"https://raw.githubusercontent.com/vrc-get/vrc-get/master/repositories.txt";

export const VPM_CATALOG_URL =
	"https://raw.githubusercontent.com/kurotu/vpm-catalog/main/repositories.txt";

export const VCC_REPO_CATALOG_URL =
	"https://raw.githubusercontent.com/vcc-repo/vcc-repo.github.io/main/repos.json";

export type CatalogRepositoryEntry = {
	url: string;
	name: string;
	id?: string;
	nsfw?: boolean;
};

function formatUrlToName(url: string): string {
	try {
		const parsed = new URL(url);
		const path = parsed.pathname.replace(
			/\/index\.json|\/vpm\.json|\/repos\.json|\/main\.json|\/$/,
			"",
		);
		if (path.length > 1) {
			const parts = path.split("/").filter(Boolean);
			if (parts.length > 0) {
				const last = parts[parts.length - 1];
				return last.charAt(0).toUpperCase() + last.slice(1);
			}
		}
		return parsed.hostname;
	} catch (_e) {
		return url;
	}
}

export async function fetchCatalogEntries(): Promise<CatalogRepositoryEntry[]> {
	const map = new Map<string, CatalogRepositoryEntry>();

	// 1. Fetch vcc-repo catalog feed (includes name, id, nsfw flag, and url)
	try {
		const res = await fetch(VCC_REPO_CATALOG_URL);
		if (res.ok) {
			const data = (await res.json()) as Array<{
				url?: string;
				name?: string;
				id?: string;
				nsfw?: boolean;
			}>;
			if (Array.isArray(data)) {
				data.forEach((item) => {
					if (item?.url) {
						const normUrl = item.url.trim().toLowerCase();
						map.set(normUrl, {
							url: item.url.trim(),
							name: item.name || formatUrlToName(item.url),
							id: item.id,
							nsfw: Boolean(item.nsfw),
						});
					}
				});
			}
		}
	} catch (e) {
		console.warn("Failed to fetch vcc-repo catalog feed:", e);
	}

	// 2. Fetch vrc-get repositories.txt feed
	try {
		const res = await fetch(VRC_GET_CATALOG_URL);
		if (res.ok) {
			const text = await res.text();
			text
				.split("\n")
				.map((line) => line.trim())
				.filter((line) => line.length > 0 && !line.startsWith("#"))
				.forEach((u) => {
					const normUrl = u.toLowerCase();
					if (!map.has(normUrl)) {
						map.set(normUrl, {
							url: u,
							name: formatUrlToName(u),
						});
					}
				});
		}
	} catch (e) {
		console.warn("Failed to fetch vrc-get catalog feed:", e);
	}

	// 3. Fetch vpm-catalog repositories.txt feed
	try {
		const res = await fetch(VPM_CATALOG_URL);
		if (res.ok) {
			const text = await res.text();
			text
				.split("\n")
				.map((line) => line.trim())
				.filter((line) => line.length > 0 && !line.startsWith("#"))
				.forEach((u) => {
					const normUrl = u.toLowerCase();
					if (!map.has(normUrl)) {
						map.set(normUrl, {
							url: u,
							name: formatUrlToName(u),
						});
					}
				});
		}
	} catch (e) {
		console.warn("Failed to fetch vpm-catalog feed:", e);
	}

	return Array.from(map.values());
}

export async function downloadCatalogRepoInfo(
	url: string,
): Promise<TauriRemoteRepositoryInfo | null> {
	// First try environmentFetchRepositoryInfo via Rust (bypasses CORS and supports all repos)
	try {
		const info = await commands.environmentFetchRepositoryInfo(url, {});
		if (info) {
			return info;
		}
	} catch (_e) {
		// Fall through to HTTP fetch
	}

	// Secondary fallback: direct HTTP fetch
	try {
		const res = await fetch(url);
		if (res.ok) {
			const json = (await res.json()) as Record<string, unknown>;
			if (json && typeof json === "object") {
				const displayName = (json.name as string) || (json.id as string) || url;
				const id = (json.id as string) || url;
				const packages: TauriBasePackageInfo[] = [];

				if (json.packages && typeof json.packages === "object") {
					for (const [pkgId, pkgData] of Object.entries(
						json.packages as Record<string, unknown>,
					)) {
						if (
							pkgData &&
							typeof pkgData === "object" &&
							"versions" in pkgData &&
							pkgData.versions &&
							typeof pkgData.versions === "object"
						) {
							const versionMap = pkgData.versions as Record<
								string,
								Record<string, unknown>
							>;
							const versionKeys = Object.keys(versionMap);
							if (versionKeys.length > 0) {
								const verData = versionMap[versionKeys[0]];
								if (verData) {
									packages.push({
										name: (verData.name as string) || pkgId,
										display_name:
											(verData.displayName as string) ||
											(verData.name as string) ||
											pkgId,
										description: (verData.description as string) || null,
										keywords: Array.isArray(verData.keywords)
											? (verData.keywords as string[])
											: [],
										version: {
											major: 0,
											minor: 0,
											patch: 0,
											pre: "",
											build: "",
										},
										unity: null,
										changelog_url: null,
										documentation_url: null,
										vpm_dependencies: [],
										legacy_packages: [],
										is_yanked: false,
									});
								}
							}
						}
					}
				}

				return {
					display_name: displayName,
					id: id,
					url: url,
					packages: packages,
				};
			}
		}
	} catch (e) {
		console.error(`Failed to download repository info for ${url}:`, e);
	}
	return null;
}

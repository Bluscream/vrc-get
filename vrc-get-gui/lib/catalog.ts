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

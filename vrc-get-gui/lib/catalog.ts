export const DEFAULT_CATALOG_URL =
	"https://raw.githubusercontent.com/vrc-get/vrc-get/master/repositories.txt";

export const VCC_REPO_CATALOG_URL =
	"https://raw.githubusercontent.com/vcc-repo/vcc-repo.github.io/main/repos.json";

export type CatalogRepositoryEntry = {
	url: string;
	name: string;
	id?: string;
};

// Fallback list of popular curated community VPM repositories with names
export const EMBEDDED_REPOSITORIES: CatalogRepositoryEntry[] = [
	{
		name: "anatawa12",
		id: "com.anatawa12.main",
		url: "https://vpm.anatawa12.com/vpm.json",
	},
	{
		name: "bd_",
		id: "dev.nadena.vpm",
		url: "https://vpm.nadena.dev/vpm.json",
	},
	{
		name: "VRCFury Repo",
		id: "com.vrcfury.vcc",
		url: "https://vcc.vrcfury.com",
	},
	{
		name: "Poiyomi Shaders",
		id: "com.poiyomi.vpm",
		url: "https://poiyomi.github.io/vpm/index.json",
	},
	{
		name: "lilLab",
		id: "jp.lilxyzw.vpm",
		url: "https://lilxyzw.github.io/vpm-repos/vpm.json",
	},
	{
		name: "Hai-VR",
		id: "dev.hai-vr.vpm",
		url: "https://hai-vr.github.io/vpm-listing/index.json",
	},
	{
		name: "CyanTrigger VCC Listing",
		id: "com.cyan.cyantrigger.vcc-listing",
		url: "https://cyanlaser.github.io/CyanTrigger/index.json",
	},
	{
		name: "d4rkpl4y3r",
		id: "io.github.d4rkc0d3r",
		url: "https://d4rkc0d3r.github.io/vpm-repos/main.json",
	},
	{
		name: "Curated VPM Listing",
		id: "com.vrchat.repos.curated",
		url: "https://vrchat-community.github.io/vpm-listing-curated/index.json",
	},
	{
		name: "Azukimochi",
		id: "io.github.azukimochi.main",
		url: "https://azukimochi.github.io/vpm-repos/index.json",
	},
	{
		name: "gatosyocora",
		id: "net.gatosyocora.vpm",
		url: "https://vpm.gatosyocora.net/index.json",
	},
	{
		name: "Iwashi Packages",
		id: "si.iwa.packages",
		url: "https://vpm.iwa.si/vpm.json",
	},
	{
		name: "BluWizard LABS Repository",
		id: "net.bluwizard.vpmlist",
		url: "https://vpm.bluwizard.net/index.json",
	},
	{
		name: "chocopoi Listing",
		id: "com.chocopoi.vpm-listing",
		url: "https://vpm.chocopoi.com/index.json",
	},
	{
		name: "Thry",
		id: "vpm.thry.dev",
		url: "https://vpm.thry.dev/index.json",
	},
];

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

	// Seed with default embedded list
	EMBEDDED_REPOSITORIES.forEach((entry) => {
		map.set(entry.url.toLowerCase(), entry);
	});

	// Fetch vcc-repo feed (which includes pre-indexed names, ids, and urls)
	try {
		const res = await fetch(VCC_REPO_CATALOG_URL);
		if (res.ok) {
			const data = (await res.json()) as Array<{
				url?: string;
				name?: string;
				id?: string;
			}>;
			if (Array.isArray(data)) {
				data.forEach((item) => {
					if (item?.url) {
						const normUrl = item.url.trim().toLowerCase();
						const existing = map.get(normUrl);
						map.set(normUrl, {
							url: item.url.trim(),
							name: item.name || existing?.name || formatUrlToName(item.url),
							id: item.id || existing?.id,
						});
					}
				});
			}
		}
	} catch (e) {
		console.warn("Failed to fetch vcc-repo catalog feed:", e);
	}

	// Fetch primary repositories.txt feed
	try {
		const res = await fetch(DEFAULT_CATALOG_URL);
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
		console.warn("Failed to fetch primary repositories.txt feed:", e);
	}

	return Array.from(map.values());
}

import {
	commands,
	type TauriBasePackageInfo,
	type TauriRemoteRepositoryInfo,
} from "@/lib/bindings";

export const DEFAULT_CATALOG_URL =
	"https://raw.githubusercontent.com/vrc-get/vrc-get/master/repositories.txt";

export const VCC_REPO_CATALOG_URL =
	"https://raw.githubusercontent.com/vcc-repo/vcc-repo.github.io/main/repos.json";

export type CatalogRepositoryItem = {
	url: string;
	info?: TauriRemoteRepositoryInfo;
	error?: string;
	loading: boolean;
};

// Fallback list of popular curated community VPM repositories
export const EMBEDDED_REPOSITORY_URLS: string[] = [
	"https://vpm.anatawa12.com/vpm.json",
	"https://vpm.nadena.dev/vpm.json",
	"https://vcc.vrcfury.com",
	"https://poiyomi.github.io/vpm/index.json",
	"https://lilxyzw.github.io/vpm-repos/vpm.json",
	"https://hai-vr.github.io/vpm-listing/index.json",
	"https://cyanlaser.github.io/CyanTrigger/index.json",
	"https://d4rkc0d3r.github.io/vpm-repos/main.json",
	"https://vrchat-community.github.io/vpm-listing-curated/index.json",
	"https://azukimochi.github.io/vpm-repos/index.json",
	"https://vpm.gatosyocora.net/index.json",
	"https://vpm.iwa.si/vpm.json",
	"https://vpm.bluwizard.net/index.json",
	"https://vpm.chocopoi.com/index.json",
	"https://vpm.thry.dev/index.json",
	"https://azukitiger.github.io/vrc-prefabs/index.json",
	"https://bluscream.github.io/unity-editor-scripts/index.json",
	"https://rerigferl.github.io/vpm/vpm.json",
	"https://Adjerry91.github.io/VRCFaceTracking-Templates/index.json",
	"https://lastationvrchat.github.io/Lastation-Package-Listing/index.json",
	"https://vpm.vrclinking.com/index.json",
];

export async function fetchCatalogUrls(): Promise<string[]> {
	const urlSet = new Set<string>();

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
					urlSet.add(u);
				});
		}
	} catch (e) {
		console.warn("Failed to fetch primary repositories.txt feed:", e);
	}

	// Fetch vcc-repo feed
	try {
		const res = await fetch(VCC_REPO_CATALOG_URL);
		if (res.ok) {
			const data = (await res.json()) as Array<{ url: string }>;
			if (Array.isArray(data)) {
				data.forEach((item) => {
					if (item?.url) {
						urlSet.add(item.url.trim());
					}
				});
			}
		}
	} catch (e) {
		console.warn("Failed to fetch vcc-repo catalog feed:", e);
	}

	if (urlSet.size === 0) {
		return EMBEDDED_REPOSITORY_URLS;
	}

	return Array.from(urlSet);
}

export async function downloadCatalogRepoInfo(
	url: string,
): Promise<TauriRemoteRepositoryInfo | null> {
	// Attempt direct HTTP fetch first (works for all repos, including already added ones)
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
	} catch (_e) {
		// Ignore CORS or fetch errors, fall through to Tauri command
	}

	// Fallback to Tauri command
	try {
		const res = await commands.environmentDownloadRepository(url, {});
		if (res.type === "Success") {
			return res.value;
		}
		if (res.type === "Duplicated") {
			return {
				display_name: res.duplicated_name || url,
				id: url,
				url: url,
				packages: [],
			};
		}
	} catch (e) {
		console.error(`Failed to download repository info for ${url}:`, e);
	}
	return null;
}

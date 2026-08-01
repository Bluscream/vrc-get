import { commands, type TauriRemoteRepositoryInfo } from "@/lib/bindings";

export const DEFAULT_CATALOG_URL =
	"https://raw.githubusercontent.com/vrc-get/vrc-get/master/repositories.txt";

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
];

export async function fetchCatalogUrls(): Promise<string[]> {
	try {
		const res = await fetch(DEFAULT_CATALOG_URL);
		if (res.ok) {
			const text = await res.text();
			const urls = text
				.split("\n")
				.map((line) => line.trim())
				.filter((line) => line.length > 0 && !line.startsWith("#"));
			if (urls.length > 0) {
				return urls;
			}
		}
	} catch (e) {
		console.warn(
			"Failed to fetch online catalog repository list, using fallback list:",
			e,
		);
	}
	return EMBEDDED_REPOSITORY_URLS;
}

export async function downloadCatalogRepoInfo(
	url: string,
): Promise<TauriRemoteRepositoryInfo | null> {
	try {
		const res = await commands.environmentDownloadRepository(url, {});
		if (res.type === "Success") {
			return res.value;
		}
	} catch (e) {
		console.error(`Failed to download repository info for ${url}:`, e);
	}
	return null;
}

import crypto from "node:crypto";
import fs from "node:fs";

const VRC_GET_CATALOG_URL =
	"https://raw.githubusercontent.com/vrc-get/vrc-get/master/repositories.txt";
const VPM_CATALOG_URL =
	"https://raw.githubusercontent.com/kurotu/vpm-catalog/master/repositories.txt";
const VCC_REPO_URL =
	"https://raw.githubusercontent.com/vcc-repo/vcc-repo.github.io/main/repos.json";

function formatUrlToName(url) {
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

function computeSha256(text) {
	return crypto.createHash("sha256").update(text).digest("hex");
}

async function main() {
	console.log("Fetching existing vcc-repo catalog...");
	const repoMap = new Map();

	try {
		const res = await fetch(VCC_REPO_URL);
		if (res.ok) {
			const data = await res.json();
			for (const item of data) {
				if (item.url) {
					repoMap.set(item.url.trim().toLowerCase(), item);
				}
			}
		}
	} catch (e) {
		console.warn("Could not fetch existing vcc-repo repos.json:", e);
	}

	console.log("Fetching vpm-catalog repositories.txt...");
	const urlSet = new Set();
	try {
		const res = await fetch(VPM_CATALOG_URL);
		if (res.ok) {
			const text = await res.text();
			text
				.split("\n")
				.map((l) => l.trim())
				.filter((l) => l.length > 0 && !l.startsWith("#"))
				.forEach((u) => urlSet.add(u));
		}
	} catch (e) {
		console.warn("Failed to fetch vpm-catalog:", e);
	}

	console.log("Fetching vrc-get repositories.txt...");
	try {
		const res = await fetch(VRC_GET_CATALOG_URL);
		if (res.ok) {
			const text = await res.text();
			text
				.split("\n")
				.map((l) => l.trim())
				.filter((l) => l.length > 0 && !l.startsWith("#"))
				.forEach((u) => urlSet.add(u));
		}
	} catch (e) {
		console.warn("Failed to fetch vrc-get catalog:", e);
	}

	console.log(`Processing ${urlSet.size} total repositories...`);
	const results = [];
	const urls = Array.from(urlSet);

	// Process in batches of 10 for performance
	const batchSize = 10;
	for (let i = 0; i < urls.length; i += batchSize) {
		const chunk = urls.slice(i, i + batchSize);
		console.log(`Processing batch ${Math.floor(i / batchSize) + 1}/${Math.ceil(urls.length / batchSize)}...`);

		const batchResults = await Promise.all(
			chunk.map(async (url) => {
				const normUrl = url.toLowerCase();
				const existing = repoMap.get(normUrl) || {};
				let name = existing.name;
				let id = existing.id;
				let author = existing.author || null;
				let nsfw = existing.nsfw || false;
				let hash = null;
				let lastUpdated = existing.last_updated || new Date().toISOString();
				let packagesData = {};

				try {
					const controller = new AbortController();
					const timeoutId = setTimeout(() => controller.abort(), 8000);
					const res = await fetch(url, { signal: controller.signal });
					clearTimeout(timeoutId);

					if (res.ok) {
						const lastMod = res.headers.get("last-modified");
						if (lastMod) {
							lastUpdated = new Date(lastMod).toISOString();
						}

						const text = await res.text();
						hash = computeSha256(text);

						const json = JSON.parse(text);
						if (json && typeof json === "object") {
							name = json.name || name || json.id;
							id = json.id || id;
							if (json.author) author = json.author;

							if (json.packages && typeof json.packages === "object") {
								for (const [pkgId, pkgObj] of Object.entries(json.packages)) {
									if (pkgObj && typeof pkgObj === "object" && pkgObj.versions) {
										const versions = pkgObj.versions || {};
										const versionKeys = Object.keys(versions);
										const latestVersion = versionKeys.length > 0 ? versions[versionKeys[0]] : null;

										packagesData[pkgId] = {
											id: pkgId,
											name: latestVersion?.name || pkgId,
											displayName: latestVersion?.displayName || latestVersion?.name || pkgId,
											description: latestVersion?.description || null,
											latestVersion: versionKeys[0] || null,
											versionCount: versionKeys.length,
											keywords: latestVersion?.keywords || [],
											unity: latestVersion?.unity || null,
											author: latestVersion?.author || author || null,
											versions: Object.fromEntries(
												Object.entries(versions).map(([vKey, vVal]) => [
													vKey,
													{
														version: vKey,
														name: vVal.name,
														displayName: vVal.displayName,
														description: vVal.description,
														url: vVal.url,
														zipSha256: vVal.zipSha256 || vVal.hash || null,
														unity: vVal.unity,
														vpmDependencies: vVal.vpmDependencies || vVal.dependencies || {},
													},
												]),
											),
										};
									}
								}
							}
						}
					}
				} catch (e) {
					// Fallback gracefully on fetch or parse errors
				}

				return {
					url: url,
					name: name || formatUrlToName(url),
					id: id || formatUrlToName(url).toLowerCase(),
					...(author ? { author } : {}),
					...(nsfw ? { nsfw: true } : {}),
					hash: hash || null,
					last_updated: lastUpdated,
					packageCount: Object.keys(packagesData).length,
					packages: packagesData,
				};
			}),
		);

		results.push(...batchResults);
	}

	console.log(`Generated ${results.length} enriched repository entries.`);
	fs.writeFileSync("vcc-repo-generated.json", JSON.stringify(results, null, 2));
	console.log("Saved enriched catalog to vcc-repo-generated.json");
}

main().catch(console.error);

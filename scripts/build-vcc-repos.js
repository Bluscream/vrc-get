import crypto from "node:crypto";
import fs from "node:fs";

const VRC_GET_CATALOG_URL =
	"https://raw.githubusercontent.com/vrc-get/vrc-get/master/repositories.txt";
const VPM_CATALOG_URL =
	"https://raw.githubusercontent.com/kurotu/vpm-catalog/master/repositories.txt";
const VCC_REPO_URL =
	"https://raw.githubusercontent.com/vcc-repo/vcc-repo.github.io/main/repos.json";

function formatUrlToName(url, json) {
	// If json has a valid display name that isn't generic or a filename
	if (json && json.name && typeof json.name === "string") {
		const trimmed = json.name.trim();
		if (
			trimmed &&
			!/^vpm$/i.test(trimmed) &&
			!/\.json$/i.test(trimmed) &&
			!/^listings?$/i.test(trimmed) &&
			!/^registry/i.test(trimmed)
		) {
			return trimmed;
		}
	}

	try {
		const parsed = new URL(url);
		const hostParts = parsed.hostname.split(".");

		// Check GitHub Pages format (username.github.io/repo-name)
		if (parsed.hostname.endsWith(".github.io")) {
			const user = hostParts[0];
			const pathSegments = parsed.pathname.split("/").filter(Boolean);
			if (pathSegments.length > 0) {
				const repo = pathSegments[0].replace(/\.json$/i, "");
				if (repo && !/^index|vpm|repos|main|registry|listings$/i.test(repo)) {
					return `${user} - ${repo.replace(/[-_]/g, " ")}`;
				}
			}
			return user;
		}

		// Check domain & subdomains (e.g. vpm.dreadscripts.com)
		if (hostParts.length >= 2) {
			const mainDomain = hostParts[hostParts.length - 2];
			if (mainDomain && !/^github|gitlab|gitee$/i.test(mainDomain)) {
				const formattedDomain =
					mainDomain.charAt(0).toUpperCase() + mainDomain.slice(1);
				const pathSegments = parsed.pathname
					.split("/")
					.filter(Boolean)
					.map((s) => s.replace(/\.json$/i, ""))
					.filter(
						(s) =>
							!/^index|vpm|repos|main|registry|listings|category$/i.test(s),
					);
				if (pathSegments.length > 0) {
					return `${formattedDomain} - ${pathSegments.map(s => s.charAt(0).toUpperCase() + s.slice(1)).join(" ")}`;
				}
				return `${formattedDomain} VPM`;
			}
		}

		return parsed.hostname;
	} catch (_e) {
		return url;
	}
}

function formatUrlToId(url, json) {
	if (json && json.id && typeof json.id === "string") {
		const trimmed = json.id.trim();
		if (
			trimmed &&
			!/^vpm$/i.test(trimmed) &&
			!/\.json$/i.test(trimmed) &&
			!/^listings?$/i.test(trimmed) &&
			!/^registry/i.test(trimmed)
		) {
			return trimmed;
		}
	}

	try {
		const parsed = new URL(url);
		const hostParts = parsed.hostname.split(".");

		if (parsed.hostname.endsWith(".github.io")) {
			const user = hostParts[0];
			const pathSegments = parsed.pathname.split("/").filter(Boolean);
			if (pathSegments.length > 0) {
				const repo = pathSegments[0].replace(/\.json$/i, "");
				if (repo && !/^index|vpm|repos|main|registry|listings$/i.test(repo)) {
					return `${user}.${repo.toLowerCase()}`;
				}
			}
			return user.toLowerCase();
		}

		return parsed.hostname.toLowerCase();
	} catch (_e) {
		return url.toLowerCase();
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
				let parsedJson = null;

				try {
					const controller = new AbortController();
					const timeoutId = setTimeout(() => controller.abort(), 10000);
					const res = await fetch(url, {
						headers: {
							"User-Agent": "vrc-get/1.1.9 ALCOM (https://github.com/vrc-get/vrc-get)",
							"Accept": "application/json, text/plain, */*",
						},
						signal: controller.signal,
					});
					clearTimeout(timeoutId);

					if (res.ok) {
						const lastMod = res.headers.get("last-modified");
						if (lastMod) {
							lastUpdated = new Date(lastMod).toISOString();
						}

						const text = await res.text();
						hash = computeSha256(text);

						parsedJson = JSON.parse(text);
						if (parsedJson && typeof parsedJson === "object") {
							if (parsedJson.author) author = parsedJson.author;

							if (parsedJson.packages && typeof parsedJson.packages === "object") {
								for (const [pkgId, pkgObj] of Object.entries(parsedJson.packages)) {
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
					// Fallback gracefully
				}

				name = formatUrlToName(url, parsedJson);
				id = formatUrlToId(url, parsedJson);

				return {
					url: url,
					name: name,
					id: id,
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

	console.log(`Generated ${results.length} cleaned repository entries.`);
	fs.writeFileSync("vcc-repo-generated.json", JSON.stringify(results, null, 2));
	console.log("Saved cleaned catalog to vcc-repo-generated.json");
}

main().catch(console.error);

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

	for (const url of urlSet) {
		const normUrl = url.toLowerCase();
		const existing = repoMap.get(normUrl) || {};
		let name = existing.name;
		let id = existing.id;
		let nsfw = existing.nsfw || false;

		// Fetch repository JSON if name/id are missing
		if (!name || !id) {
			try {
				const res = await fetch(url);
				if (res.ok) {
					const json = await res.json();
					if (json && typeof json === "object") {
						name = name || json.name || json.id;
						id = id || json.id;
					}
				}
			} catch (_e) {
				// Ignore fetch errors
			}
		}

		results.push({
			url: url,
			name: name || formatUrlToName(url),
			id: id || formatUrlToName(url).toLowerCase(),
			...(nsfw ? { nsfw: true } : {}),
		});
	}

	console.log(`Generated ${results.length} repository entries.`);
	fs.writeFileSync("vcc-repo-generated.json", JSON.stringify(results, null, 2));
	console.log("Saved to vcc-repo-generated.json");
}

main().catch(console.error);

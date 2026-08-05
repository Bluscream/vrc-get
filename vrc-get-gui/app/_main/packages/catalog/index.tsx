"use client";

import { queryOptions, useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Check, Plus } from "lucide-react";
import { Suspense, useEffect, useMemo, useState } from "react";
import { HNavBar, VStack } from "@/components/layout";
import { ScrollableCardTable } from "@/components/ScrollableCardTable";
import { SearchBox } from "@/components/SearchBox";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { commands, type TauriRemoteRepositoryInfo } from "@/lib/bindings";
import {
	type CatalogRepositoryEntry,
	downloadCatalogRepoInfo,
	fetchCatalogEntries,
} from "@/lib/catalog";
import { tc } from "@/lib/i18n";
import { usePrevPathName } from "@/lib/prev-page";
import { HeadingPageName } from "../-tab-selector";
import { addRepository } from "../repositories/-use-add-repository";

export const Route = createFileRoute("/_main/packages/catalog/")({
	component: Page,
});

const environmentRepositoriesInfo = queryOptions({
	queryKey: ["environmentRepositoriesInfo"],
	queryFn: commands.environmentRepositoriesInfo,
});

function Page() {
	return (
		<Suspense>
			<PageBody />
		</Suspense>
	);
}

type CatalogRepoItemState = CatalogRepositoryEntry & {
	info?: TauriRemoteRepositoryInfo | null;
	loading: boolean;
};

function PageBody() {
	const userReposQuery = useQuery(environmentRepositoriesInfo);
	const userRepoUrls = useMemo(() => {
		const set = new Set<string>();
		if (userReposQuery.data?.user_repositories) {
			for (const repo of userReposQuery.data.user_repositories) {
				if (repo.url) set.add(repo.url.toLowerCase());
			}
		}
		return set;
	}, [userReposQuery.data]);

	const [repos, setRepos] = useState<CatalogRepoItemState[]>([]);
	const [searchQuery, setSearchQuery] = useState("");
	const [isLoading, setIsLoading] = useState(true);

	useEffect(() => {
		let isMounted = true;
		setIsLoading(true);

		fetchCatalogEntries().then((entries) => {
			if (!isMounted) return;
			const initialItems: CatalogRepoItemState[] = entries.map((entry) => ({
				...entry,
				loading: true,
			}));
			setRepos(initialItems);
			setIsLoading(false);

			// Download package contents in batches of 5
			const batchSize = 5;
			async function loadBatches() {
				for (let i = 0; i < entries.length; i += batchSize) {
					if (!isMounted) break;
					const chunk = entries.slice(i, i + batchSize);
					const results = await Promise.all(
						chunk.map(async (entry) => {
							const info = await downloadCatalogRepoInfo(entry.url);
							return { url: entry.url, info };
						}),
					);
					if (!isMounted) break;
					setRepos((prev) =>
						prev.map((item) => {
							const found = results.find((r) => r.url === item.url);
							if (found) {
								return { ...item, info: found.info, loading: false };
							}
							return item;
						}),
					);
				}
			}
			loadBatches();
		});

		return () => {
			isMounted = false;
		};
	}, []);

	const filteredRepos = useMemo(() => {
		if (!searchQuery.trim()) return repos;
		const q = searchQuery.toLowerCase().trim();
		return repos.filter((item) => {
			if (item.name?.toLowerCase().includes(q)) return true;
			if (item.url?.toLowerCase().includes(q)) return true;
			if (item.id?.toLowerCase().includes(q)) return true;
			if (item.info?.display_name?.toLowerCase().includes(q)) return true;
			if (item.info?.id?.toLowerCase().includes(q)) return true;
			if (
				item.info?.packages?.some(
					(pkg) =>
						pkg?.name?.toLowerCase().includes(q) ||
						pkg?.display_name?.toLowerCase().includes(q),
				)
			) {
				return true;
			}
			return false;
		});
	}, [repos, searchQuery]);

	const bodyAnimation = usePrevPathName().startsWith("/packages")
		? "slide-left"
		: "";

	return (
		<VStack>
			<HNavBar
				className="shrink-0"
				leading={<HeadingPageName pageType={"/packages/catalog"} />}
				trailing={
					<SearchBox
						className="w-64"
						value={searchQuery}
						onChange={(e) => setSearchQuery(e.target.value)}
					/>
				}
			/>
			<main
				className={`shrink overflow-hidden flex flex-col w-full h-full p-4 gap-4 ${bodyAnimation}`}
			>
				{isLoading && (
					<div className="text-center py-8 text-muted-foreground">
						{tc("vpm catalog:loading catalog...")}
					</div>
				)}

				{!isLoading && filteredRepos.length === 0 && (
					<div className="text-center py-8 text-muted-foreground">
						{tc("vpm catalog:no repositories found")}
					</div>
				)}

				{!isLoading && filteredRepos.length > 0 && (
					<ScrollableCardTable className="h-full w-full">
						<div className="flex flex-col gap-3 pb-4">
							{filteredRepos.map((item) => {
								const isAdded = userRepoUrls.has(item.url.toLowerCase());
								const displayName = item.info?.display_name || item.name;

								return (
									<Card key={item.url} className="w-full">
										<CardHeader className="py-3 px-4 flex flex-row items-center justify-between space-y-0">
											<div className="flex flex-col gap-1 min-w-0 pr-4">
												<div className="flex items-center gap-2">
													<CardTitle className="text-base font-semibold truncate">
														{displayName}
													</CardTitle>
													{item.nsfw && (
														<span className="text-[10px] uppercase font-bold px-1.5 py-0.5 rounded bg-destructive/10 text-destructive border border-destructive/20">
															NSFW
														</span>
													)}
												</div>
												<span className="text-xs text-muted-foreground truncate">
													{item.url}
												</span>
											</div>
											<div className="flex items-center gap-2 shrink-0">
												{isAdded ? (
													<span className="inline-flex items-center gap-1 px-3 py-1 text-xs font-semibold rounded-md bg-secondary text-secondary-foreground">
														<Check className="h-3.5 w-3.5" />
														{tc("vpm catalog:status:added")}
													</span>
												) : (
													<Button
														size="sm"
														className="gap-1"
														onClick={() => addRepository(item.url, {})}
													>
														<Plus className="h-3.5 w-3.5" />
														{tc("vpm catalog:button:add")}
													</Button>
												)}
											</div>
										</CardHeader>
										{item.info && item.info.packages.length > 0 && (
											<CardContent className="py-2 px-4 border-t bg-muted/20">
												<div className="text-xs font-medium text-muted-foreground mb-1.5">
													{tc("vpm catalog:packages count", {
														count: item.info.packages.length,
													})}
												</div>
												<div className="flex flex-wrap gap-1.5 max-h-24 overflow-y-auto">
													{item.info.packages.map((pkg) => (
														<span
															key={pkg.name}
															className="inline-flex items-center text-xs py-0.5 px-2 rounded border border-input bg-background font-normal"
														>
															{pkg.display_name || pkg.name}
														</span>
													))}
												</div>
											</CardContent>
										)}
									</Card>
								);
							})}
						</div>
					</ScrollableCardTable>
				)}
			</main>
		</VStack>
	);
}

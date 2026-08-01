"use client";

import { queryOptions, useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Check, Plus, Search } from "lucide-react";
import { Suspense, useEffect, useMemo, useState } from "react";
import { HNavBar, VStack } from "@/components/layout";
import { ScrollableCardTable } from "@/components/ScrollableCardTable";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { commands, type TauriRemoteRepositoryInfo } from "@/lib/bindings";
import { downloadCatalogRepoInfo, fetchCatalogUrls } from "@/lib/catalog";
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

type CatalogRepoState = {
	url: string;
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

	const [repos, setRepos] = useState<CatalogRepoState[]>([]);
	const [searchQuery, setSearchQuery] = useState("");
	const [isLoadingUrls, setIsLoadingUrls] = useState(true);

	useEffect(() => {
		let isMounted = true;
		setIsLoadingUrls(true);
		fetchCatalogUrls().then((urls) => {
			if (!isMounted) return;
			setRepos(urls.map((url) => ({ url, loading: true })));
			setIsLoadingUrls(false);

			// Download repository metadata in batches of 5
			const batchSize = 5;
			async function loadBatches() {
				for (let i = 0; i < urls.length; i += batchSize) {
					if (!isMounted) break;
					const chunk = urls.slice(i, i + batchSize);
					const results = await Promise.all(
						chunk.map(async (url) => {
							const info = await downloadCatalogRepoInfo(url);
							return { url, info };
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
			if (item.url.toLowerCase().includes(q)) return true;
			if (!item.info) return false;
			if (item.info.display_name.toLowerCase().includes(q)) return true;
			if (item.info.id.toLowerCase().includes(q)) return true;
			return item.info.packages.some(
				(pkg) =>
					pkg.name.toLowerCase().includes(q) ||
					pkg.id.toLowerCase().includes(q),
			);
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
					<div className="relative w-64 compact:h-10 flex items-center">
						<Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
						<Input
							type="search"
							placeholder={tc("vpm catalog:search:placeholder")}
							className="pl-8 text-sm"
							value={searchQuery}
							onChange={(e) => setSearchQuery(e.target.value)}
						/>
					</div>
				}
			/>
			<main
				className={`shrink overflow-hidden flex flex-col w-full h-full p-4 gap-4 ${bodyAnimation}`}
			>
				{isLoadingUrls && (
					<div className="text-center py-8 text-muted-foreground">
						{tc("vpm catalog:loading catalog...")}
					</div>
				)}

				{!isLoadingUrls && filteredRepos.length === 0 && (
					<div className="text-center py-8 text-muted-foreground">
						{tc("vpm catalog:no repositories found")}
					</div>
				)}

				{!isLoadingUrls && filteredRepos.length > 0 && (
					<ScrollableCardTable className="h-full w-full">
						<div className="flex flex-col gap-3 pb-4">
							{filteredRepos.map((item) => {
								const isAdded = userRepoUrls.has(item.url.toLowerCase());
								return (
									<Card key={item.url} className="w-full">
										<CardHeader className="py-3 px-4 flex flex-row items-center justify-between space-y-0">
											<div className="flex flex-col gap-1 min-w-0 pr-4">
												<CardTitle className="text-base font-semibold truncate">
													{item.info ? item.info.display_name : item.url}
												</CardTitle>
												<span className="text-xs text-muted-foreground truncate">
													{item.url}
												</span>
											</div>
											<div className="flex items-center gap-2 shrink-0">
												{isAdded ? (
													<Badge
														variant="secondary"
														className="gap-1 px-3 py-1"
													>
														<Check className="h-3.5 w-3.5" />
														{tc("vpm catalog:status:added")}
													</Badge>
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
														<Badge
															key={pkg.id}
															variant="outline"
															className="text-xs py-0.5 px-2"
														>
															{pkg.name || pkg.id}
														</Badge>
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

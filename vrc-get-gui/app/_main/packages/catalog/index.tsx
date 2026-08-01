"use client";

import { queryOptions, useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Check, Plus, Search } from "lucide-react";
import { Suspense, useEffect, useMemo, useState } from "react";
import { HNavBar, VStack } from "@/components/layout";
import { ScrollableCardTable } from "@/components/ScrollableCardTable";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { commands } from "@/lib/bindings";
import {
	type CatalogRepositoryEntry,
	fetchCatalogEntries,
} from "@/lib/catalog";
import { tc, tt } from "@/lib/i18n";
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

	const [entries, setEntries] = useState<CatalogRepositoryEntry[]>([]);
	const [searchQuery, setSearchQuery] = useState("");
	const [isLoading, setIsLoading] = useState(true);

	useEffect(() => {
		let isMounted = true;
		setIsLoading(true);
		fetchCatalogEntries().then((data) => {
			if (!isMounted) return;
			setEntries(data);
			setIsLoading(false);
		});

		return () => {
			isMounted = false;
		};
	}, []);

	const filteredEntries = useMemo(() => {
		if (!searchQuery.trim()) return entries;
		const q = searchQuery.toLowerCase().trim();
		return entries.filter((item) => {
			if (item.name?.toLowerCase().includes(q)) return true;
			if (item.url?.toLowerCase().includes(q)) return true;
			if (item.id?.toLowerCase().includes(q)) return true;
			return false;
		});
	}, [entries, searchQuery]);

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
							placeholder={tt("vpm catalog:search:placeholder")}
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
				{isLoading && (
					<div className="text-center py-8 text-muted-foreground">
						{tc("vpm catalog:loading catalog...")}
					</div>
				)}

				{!isLoading && filteredEntries.length === 0 && (
					<div className="text-center py-8 text-muted-foreground">
						{tc("vpm catalog:no repositories found")}
					</div>
				)}

				{!isLoading && filteredEntries.length > 0 && (
					<ScrollableCardTable className="h-full w-full">
						<div className="flex flex-col gap-3 pb-4">
							{filteredEntries.map((item) => {
								const isAdded = userRepoUrls.has(item.url.toLowerCase());
								return (
									<Card key={item.url} className="w-full">
										<CardHeader className="py-3 px-4 flex flex-row items-center justify-between space-y-0">
											<div className="flex flex-col gap-1 min-w-0 pr-4">
												<CardTitle className="text-base font-semibold truncate">
													{item.name}
												</CardTitle>
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

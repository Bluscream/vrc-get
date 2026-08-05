"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
	AlertCircle,
	ArchiveRestore,
	CheckCircle2,
	ChevronDown,
	Copy,
	Folder,
	HardDrive,
	PackageCheck,
	RefreshCw,
	Trash2,
} from "lucide-react";
import { useMemo, useState } from "react";
import { SearchBox } from "@/components/SearchBox";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
	Dialog,
	DialogClose,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import {
	commands,
	type TauriBackupInfo,
	type TauriRestoreBackupProgress,
	type TauriRestoreResult,
} from "@/lib/bindings";
import { callAsyncCommand } from "@/lib/call-async-command";
import { tc } from "@/lib/i18n";
import { toastError, toastSuccess } from "@/lib/toast";

export const Route = createFileRoute("/_main/backups/")({
	component: BackupsPage,
});

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

function formatDate(seconds: number): string {
	if (!seconds) return "-";
	return new Date(seconds * 1000).toLocaleString();
}

function BackupsPage() {
	const queryClient = useQueryClient();
	const [searchQuery, setSearchQuery] = useState("");
	const [selectedBackup, setSelectedBackup] = useState<TauriBackupInfo | null>(
		null,
	);
	const [backupToDelete, setBackupToDelete] = useState<TauriBackupInfo | null>(
		null,
	);
	const [isDeleting, setIsDeleting] = useState(false);
	const [customFolderName, setCustomFolderName] = useState("");
	const [restoreResult, setRestoreResult] =
		useState<TauriRestoreResult | null>(null);
	const [isApplyingChanges, setIsApplyingChanges] = useState(false);
	const [isRestoring, setIsRestoring] = useState(false);
	const [restoreProgress, setRestoreProgress] =
		useState<TauriRestoreBackupProgress | null>(null);

	const settingsQuery = useQuery({
		queryKey: ["environmentGetSettings"],
		queryFn: commands.environmentGetSettings,
	});

	const backupsQuery = useQuery({
		queryKey: ["environmentListBackups"],
		queryFn: commands.environmentListBackups,
	});

	const filteredBackups = useMemo(() => {
		if (!backupsQuery.data) return [];
		if (!searchQuery.trim()) return backupsQuery.data;
		const q = searchQuery.toLowerCase();
		return backupsQuery.data.filter(
			(b) =>
				b.file_name.toLowerCase().includes(q) ||
				b.path.toLowerCase().includes(q),
		);
	}, [backupsQuery.data, searchQuery]);

	const handleOpenRestoreModal = (backup: TauriBackupInfo) => {
		setSelectedBackup(backup);
		const defaultStem = backup.file_name.replace(/\.(zip|tar|gz)$/i, "");
		setCustomFolderName(defaultStem);
		setRestoreProgress(null);
		setIsRestoring(false);
	};

	const handleOpenBackupDir = async (backup: TauriBackupInfo) => {
		try {
			await commands.utilOpen(backup.path, "open-folder");
		} catch (err) {
			toastError(`Failed to open backup folder: ${String(err)}`);
		}
	};

	const handleConfirmDelete = async () => {
		if (!backupToDelete) return;
		try {
			setIsDeleting(true);
			await commands.environmentDeleteBackup(backupToDelete.path);
			toastSuccess(`Deleted backup: ${backupToDelete.file_name}`);
			setBackupToDelete(null);
			void backupsQuery.refetch();
		} catch (err) {
			toastError(`Failed to delete backup: ${String(err)}`);
		} finally {
			setIsDeleting(false);
		}
	};

	const handleConfirmRestore = () => {
		if (!selectedBackup) return;
		setIsRestoring(true);
		setRestoreProgress({
			proceed: 0,
			total: 1,
			last_proceed: "Opening backup archive...",
			read_bytes: 0,
			total_bytes: selectedBackup.size_bytes || 0,
			bytes_per_sec: 0,
		});

		const [_, promise] = callAsyncCommand(
			commands.environmentRestoreBackup,
			[selectedBackup.path, customFolderName.trim() || null],
			(prog) => {
				setRestoreProgress(prog);
			},
		);

		promise
			.then((res) => {
				setSelectedBackup(null);
				setIsRestoring(false);
				setRestoreProgress(null);
				void queryClient.invalidateQueries({
					queryKey: ["environmentProjects"],
				});
				if (res.should_resolve) {
					setRestoreResult(res);
				} else {
					toastSuccess(`Project restored successfully to: ${res.dest_path}`);
				}
			})
			.catch((err) => {
				setIsRestoring(false);
				setRestoreProgress(null);
				toastError(`Failed to restore backup: ${String(err)}`);
			});
	};

	const handleApplyPendingChanges = async () => {
		if (!restoreResult || !restoreResult.pending_changes) return;
		try {
			setIsApplyingChanges(true);
			await commands.projectApplyPendingChanges(
				restoreResult.dest_path,
				restoreResult.pending_changes.changes_version,
			);
			toastSuccess(
				`VPM Packages installed successfully for ${restoreResult.dest_path}!`,
			);
			void queryClient.invalidateQueries({
				queryKey: ["environmentProjects"],
			});
			setRestoreResult(null);
		} catch (err) {
			toastError(`Failed to install packages: ${String(err)}`);
		} finally {
			setIsApplyingChanges(false);
		}
	};

	const handleCopyMissingDependencies = () => {
		if (!restoreResult?.missing_dependencies?.length) return;
		const text = restoreResult.missing_dependencies.join("\n");
		void navigator.clipboard.writeText(text);
		toastSuccess("Missing package dependencies copied to clipboard!");
	};

	const backupPath =
		settingsQuery.data?.project_backup_path || "Default Backup Location";
	const defaultProjectPath =
		settingsQuery.data?.default_project_path || "Default Project Location";

	return (
		<div className="flex flex-col h-full w-full p-4 overflow-hidden gap-4">
			{/* Page Header */}
			<div className="flex items-center justify-between shrink-0 gap-4">
				<div>
					<h1 className="text-2xl font-bold flex items-center gap-2">
						<ArchiveRestore className="h-6 w-6 text-primary" />
						Project Backups
					</h1>
					<p className="text-sm text-muted-foreground mt-1">
						List of backup archives saved in your configured Backup Path. You
						can restore any backup archive to your Default Project Path.
					</p>
				</div>
				<div className="flex items-center gap-2 shrink-0">
					<SearchBox
						className="w-64"
						value={searchQuery}
						onChange={(e) => setSearchQuery(e.target.value)}
					/>
					<Tooltip>
						<TooltipTrigger asChild>
							<Button
								variant="outline"
								size="icon"
								onClick={() => void backupsQuery.refetch()}
								disabled={backupsQuery.isFetching}
							>
								<RefreshCw
									className={`h-4 w-4 ${backupsQuery.isFetching ? "animate-spin" : ""}`}
								/>
							</Button>
						</TooltipTrigger>
						<TooltipContent>Refresh Backup List</TooltipContent>
					</Tooltip>
				</div>
			</div>

			{/* Table Content */}
			<Card className="flex-1 overflow-auto p-0 border shadow-sm">
				{backupsQuery.isLoading ? (
					<div className="flex items-center justify-center h-48 text-muted-foreground">
						Loading backups...
					</div>
				) : backupsQuery.isError ? (
					<div className="flex items-center justify-center h-48 text-destructive font-semibold">
						Failed to load backups: {String(backupsQuery.error)}
					</div>
				) : filteredBackups.length === 0 ? (
					<div className="flex flex-col items-center justify-center h-48 text-muted-foreground gap-2">
						<ArchiveRestore className="h-10 w-10 opacity-40" />
						<span>
							{searchQuery
								? "No backup archives match your search query."
								: "No backup archives found in your Backup Path."}
						</span>
					</div>
				) : (
					<table className="w-full text-left text-sm border-collapse">
						<thead className="sticky top-0 bg-muted/80 backdrop-blur-sm border-b z-10">
							<tr>
								<th className="p-3 font-semibold">File Name</th>
								<th className="p-3 font-semibold">Date Created</th>
								<th className="p-3 font-semibold">Size</th>
								<th className="p-3 font-semibold text-right">Actions</th>
							</tr>
						</thead>
						<tbody className="divide-y">
							{filteredBackups.map((backup) => (
								<tr
									key={backup.path}
									className="hover:bg-muted/40 transition-colors"
								>
									<td className="p-3">
										<div className="font-medium text-foreground">
											{backup.file_name}
										</div>
										<div className="text-xs text-muted-foreground font-mono truncate max-w-md">
											{backup.path}
										</div>
									</td>
									<td className="p-3 whitespace-nowrap text-muted-foreground">
										{formatDate(backup.last_modified)}
									</td>
									<td className="p-3 whitespace-nowrap font-mono text-xs">
										{formatBytes(backup.size_bytes)}
									</td>
									<td className="p-3 text-right whitespace-nowrap">
										<DropdownMenu>
											<div className="inline-flex divide-x rounded-md shadow-xs">
												<Button
													size="sm"
													variant="default"
													className="rounded-r-none"
													onClick={() => handleOpenRestoreModal(backup)}
												>
													<ArchiveRestore className="h-4 w-4 mr-1.5" />
													Restore
												</Button>
												<DropdownMenuTrigger asChild>
													<Button
														size="sm"
														variant="default"
														className="rounded-l-none px-1.5"
													>
														<ChevronDown className="h-4 w-4" />
													</Button>
												</DropdownMenuTrigger>
											</div>
											<DropdownMenuContent align="end">
												<DropdownMenuItem
													onClick={() => void handleOpenBackupDir(backup)}
												>
													<Folder className="h-4 w-4 mr-2" />
													Open Backup Directory
												</DropdownMenuItem>
												<DropdownMenuItem
													className="text-destructive focus:text-destructive"
													onClick={() => setBackupToDelete(backup)}
												>
													<Trash2 className="h-4 w-4 mr-2" />
													Delete Backup
												</DropdownMenuItem>
											</DropdownMenuContent>
										</DropdownMenu>
									</td>
								</tr>
							))}
						</tbody>
					</table>
				)}
			</Card>

			{/* Delete Backup Confirmation Dialog */}
			<Dialog
				open={backupToDelete !== null}
				onOpenChange={(open) => !open && setBackupToDelete(null)}
			>
				<DialogContent className="max-w-md">
					<DialogHeader>
						<DialogTitle className="flex items-center gap-2 text-destructive">
							<Trash2 className="h-5 w-5" />
							Delete Backup Archive
						</DialogTitle>
					</DialogHeader>

					<div className="py-2 text-sm">
						<p>
							Are you sure you want to delete{" "}
							<strong className="font-mono">{backupToDelete?.file_name}</strong>
							?
						</p>
						<p className="text-xs text-muted-foreground mt-1.5 font-mono truncate">
							{backupToDelete?.path}
						</p>
						<p className="text-xs text-destructive mt-2 font-semibold">
							This action cannot be undone.
						</p>
					</div>

					<DialogFooter className="gap-2">
						<DialogClose asChild>
							<Button variant="outline" disabled={isDeleting}>
								{tc("general:button:cancel")}
							</Button>
						</DialogClose>
						<Button
							variant="destructive"
							onClick={handleConfirmDelete}
							disabled={isDeleting}
						>
							{isDeleting ? "Deleting..." : "Delete Backup"}
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>

			{/* Restore Confirmation Dialog */}
			<Dialog
				open={selectedBackup !== null}
				onOpenChange={(open) => !open && setSelectedBackup(null)}
			>
				<DialogContent className="max-w-md">
					<DialogHeader>
						<DialogTitle className="flex items-center gap-2">
							<ArchiveRestore className="h-5 w-5 text-primary" />
							Restore Backup Archive
						</DialogTitle>
					</DialogHeader>

					<div className="flex flex-col gap-3 py-2 text-sm">
						<p>
							You are about to restore{" "}
							<strong className="font-mono">{selectedBackup?.file_name}</strong>{" "}
							into your Default Project Path.
						</p>

						<div className="flex flex-col gap-1.5">
							<label
								htmlFor="target-folder-name"
								className="text-xs font-semibold text-muted-foreground"
							>
								Target Folder Name
							</label>
							<Input
								id="target-folder-name"
								value={customFolderName}
								disabled={isRestoring}
								onChange={(e) => setCustomFolderName(e.target.value)}
								placeholder="Restored Project Folder Name"
							/>
							<span className="text-xs text-muted-foreground">
								Will extract into:{" "}
								<code className="text-xs font-mono">
									{defaultProjectPath}/{customFolderName || "ProjectName"}
								</code>
							</span>
						</div>

						{isRestoring && restoreProgress ? (
							<div className="flex flex-col gap-2 p-3 rounded-md bg-muted/40 border text-xs">
								<div className="flex items-center justify-between font-semibold">
									<span>Extracting Backup</span>
									<span>
										{restoreProgress.proceed} / {restoreProgress.total} files
									</span>
								</div>
								<p className="overflow-hidden w-full whitespace-pre font-mono text-[11px] text-muted-foreground">
									{restoreProgress.last_proceed || "Reading backup archive..."}
								</p>
								<Progress
									value={restoreProgress.read_bytes || restoreProgress.proceed}
									max={restoreProgress.total_bytes || restoreProgress.total}
								/>
							</div>
						) : null}
					</div>

					<DialogFooter className="gap-2">
						<DialogClose asChild>
							<Button variant="outline" disabled={isRestoring}>
								{tc("general:button:cancel")}
							</Button>
						</DialogClose>
						<Button
							variant="default"
							onClick={handleConfirmRestore}
							disabled={isRestoring || !customFolderName.trim()}
						>
							{isRestoring ? "Restoring..." : "Confirm Restore"}
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>

			{/* Re-install VPM Packages Modal */}
			<Dialog
				open={restoreResult !== null}
				onOpenChange={(open) => !open && setRestoreResult(null)}
			>
				<DialogContent className="max-w-lg">
					<DialogHeader>
						<DialogTitle className="flex items-center gap-2">
							<PackageCheck className="h-5 w-5 text-primary" />
							VPM Package Dependencies
						</DialogTitle>
					</DialogHeader>

					<div className="flex flex-col gap-3 py-2 text-sm">
						<p>
							Backup restoration complete! ALCOM detected VPM package dependencies
							for this project:
						</p>

						{restoreResult?.pending_changes && (
							<div className="flex flex-col gap-2 p-3 rounded-md bg-muted/40 border">
								<span className="font-semibold text-xs text-foreground flex items-center gap-1.5">
									<CheckCircle2 className="h-4 w-4 text-emerald-500" />
									Available Packages to Install:
								</span>
								<ul className="text-xs font-mono space-y-1 pl-5 list-disc max-h-36 overflow-y-auto">
									{restoreResult.pending_changes.package_changes.map(
										([name, change]) => (
											<li key={name}>
												<span className="font-semibold">{name}</span>
											</li>
										),
									)}
								</ul>
							</div>
						)}

						{restoreResult?.missing_dependencies &&
							restoreResult.missing_dependencies.length > 0 && (
								<div className="flex flex-col gap-2 p-3 rounded-md bg-destructive/10 border border-destructive/20">
									<div className="flex items-center justify-between">
										<span className="font-semibold text-xs text-destructive flex items-center gap-1.5">
											<AlertCircle className="h-4 w-4" />
											Unresolvable / Missing Dependencies:
										</span>
										<Button
											size="xs"
											variant="outline"
											className="h-6 text-[11px] gap-1"
											onClick={handleCopyMissingDependencies}
										>
											<Copy className="h-3 w-3" />
											Copy Missing
										</Button>
									</div>
									<ul className="text-xs font-mono space-y-1 pl-5 list-disc max-h-36 overflow-y-auto text-destructive">
										{restoreResult.missing_dependencies.map((dep) => (
											<li key={dep}>{dep}</li>
										))}
									</ul>
								</div>
							)}
					</div>

					<DialogFooter className="gap-2">
						<DialogClose asChild>
							<Button variant="outline" disabled={isApplyingChanges}>
								Skip / Close
							</Button>
						</DialogClose>
						{restoreResult?.pending_changes && (
							<Button
								variant="default"
								onClick={handleApplyPendingChanges}
								disabled={isApplyingChanges}
							>
								{isApplyingChanges ? "Installing..." : "Install Packages"}
							</Button>
						)}
					</DialogFooter>
				</DialogContent>
			</Dialog>
		</div>
	);
}

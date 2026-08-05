"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
	AlertCircle,
	ArchiveRestore,
	CheckCircle2,
	Copy,
	Folder,
	HardDrive,
	PackageCheck,
	RefreshCw,
} from "lucide-react";
import { useState } from "react";
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
	const [selectedBackup, setSelectedBackup] = useState<TauriBackupInfo | null>(
		null,
	);
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

	const handleOpenRestoreModal = (backup: TauriBackupInfo) => {
		setSelectedBackup(backup);
		const defaultStem = backup.file_name.replace(/\.(zip|tar|gz)$/i, "");
		setCustomFolderName(defaultStem);
		setRestoreProgress(null);
		setIsRestoring(false);
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
			<div className="flex items-center justify-between shrink-0">
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

			{/* Info Cards */}
			<div className="grid grid-cols-1 md:grid-cols-2 gap-3 shrink-0">
				<Card className="p-3 flex items-center gap-3 bg-card/60">
					<Folder className="h-5 w-5 text-primary shrink-0" />
					<div className="overflow-hidden">
						<div className="text-xs text-muted-foreground font-semibold">
							Backup Path
						</div>
						<div className="text-sm font-mono truncate" title={backupPath}>
							{backupPath}
						</div>
					</div>
				</Card>
				<Card className="p-3 flex items-center gap-3 bg-card/60">
					<HardDrive className="h-5 w-5 text-primary shrink-0" />
					<div className="overflow-hidden">
						<div className="text-xs text-muted-foreground font-semibold">
							Default Project Path
						</div>
						<div
							className="text-sm font-mono truncate"
							title={defaultProjectPath}
						>
							{defaultProjectPath}
						</div>
					</div>
				</Card>
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
				) : !backupsQuery.data || backupsQuery.data.length === 0 ? (
					<div className="flex flex-col items-center justify-center h-48 text-muted-foreground gap-2">
						<ArchiveRestore className="h-10 w-10 opacity-40" />
						<span>No backup archives found in your Backup Path.</span>
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
							{backupsQuery.data.map((backup) => (
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
										<Button
											size="sm"
											variant="default"
											onClick={() => handleOpenRestoreModal(backup)}
										>
											<ArchiveRestore className="h-4 w-4 mr-1.5" />
											Restore
										</Button>
									</td>
								</tr>
							))}
						</tbody>
					</table>
				)}
			</Card>

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
							Restored Project - VPM Packages
						</DialogTitle>
					</DialogHeader>

					<div className="flex flex-col gap-4 py-2 text-sm max-h-[60vh] overflow-y-auto">
						<p className="text-muted-foreground">
							Project extracted to{" "}
							<code className="text-xs font-mono">
								{restoreResult?.dest_path}
							</code>
							.
						</p>

						{/* Packages Ready to Install */}
						{restoreResult?.pending_changes &&
						restoreResult.pending_changes.package_changes.length > 0 ? (
							<div className="flex flex-col gap-2">
								<h3 className="font-semibold text-foreground flex items-center gap-1.5">
									<CheckCircle2 className="h-4 w-4 text-green-500" />
									Packages Excluded from Backup (Ready to Re-install)
								</h3>
								<p className="text-xs text-muted-foreground">
									The following packages were detected in project dependencies
									and will be downloaded from your repositories:
								</p>
								<div className="border rounded-md divide-y max-h-48 overflow-y-auto bg-card">
									{restoreResult.pending_changes.package_changes.map(
										([pkgId, change]) => (
											<div
												key={pkgId}
												className="p-2.5 flex items-center justify-between text-xs font-mono"
											>
												<span className="font-semibold text-foreground">
													{pkgId}
												</span>
												<span className="text-muted-foreground">
													{change.type === "InstallNew"
														? `v${change.install.version}`
														: change.type}
												</span>
											</div>
										),
									)}
								</div>
							</div>
						) : null}

						{/* Missing / Unresolvable Dependencies Warning */}
						{restoreResult?.missing_dependencies &&
						restoreResult.missing_dependencies.length > 0 ? (
							<div className="flex flex-col gap-2 p-3 rounded-md border border-amber-500/40 bg-amber-500/10 text-amber-900 dark:text-amber-200">
								<div className="flex items-center justify-between gap-2">
									<h3 className="font-semibold flex items-center gap-1.5">
										<AlertCircle className="h-4 w-4 text-amber-500 shrink-0" />
										Unresolvable Package Dependencies
									</h3>
									<Button
										size="sm"
										variant="outline"
										className="h-7 text-xs gap-1 border-amber-500/40 bg-amber-500/10 hover:bg-amber-500/20"
										onClick={handleCopyMissingDependencies}
									>
										<Copy className="h-3.5 w-3.5" />
										Copy Missing
									</Button>
								</div>
								<p className="text-xs">
									The following required packages could not be found in your
									currently installed ALCOM repositories. You may need to add
									their corresponding repository feeds to restore them:
								</p>
								<ul className="list-disc list-inside font-mono text-xs space-y-0.5 pl-1">
									{restoreResult.missing_dependencies.map((dep) => (
										<li key={dep}>{dep}</li>
									))}
								</ul>
							</div>
						) : null}
					</div>

					<DialogFooter className="gap-2">
						<Button variant="outline" onClick={() => setRestoreResult(null)}>
							{restoreResult?.pending_changes &&
							restoreResult.pending_changes.package_changes.length > 0
								? "Skip for Now"
								: "Close"}
						</Button>
						{restoreResult?.pending_changes &&
						restoreResult.pending_changes.package_changes.length > 0 ? (
							<Button
								variant="default"
								onClick={handleApplyPendingChanges}
								disabled={isApplyingChanges}
							>
								{isApplyingChanges
									? "Installing Packages..."
									: "Install Packages"}
							</Button>
						) : null}
					</DialogFooter>
				</DialogContent>
			</Dialog>
		</div>
	);
}


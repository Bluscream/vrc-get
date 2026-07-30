import { queryOptions } from "@tanstack/react-query";
import { useState } from "react";
import { BackupProjectDialog } from "@/components/BackupProjectDialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { DialogFooter, DialogTitle } from "@/components/ui/dialog";
import { commands } from "@/lib/bindings";
import { type DialogContext, openSingleDialog } from "@/lib/dialog";
import { tc } from "@/lib/i18n";
import { nameFromPath } from "@/lib/os";
import { toastNormal, toastSuccess, toastThrownError } from "@/lib/toast";

/// How long we wait for Unity to close on its own before offering to force kill it.
const GRACEFUL_CLOSE_TIMEOUT_MS = 5000;
const POLL_INTERVAL_MS = 500;

/// Whether Unity is currently open for the project.
///
/// This is polled so the UI follows the project state while the page stays open,
/// including when Unity is started or closed outside of ALCOM.
export function projectIsUnityLaunching(projectPath: string) {
	return queryOptions({
		queryKey: ["projectIsUnityLaunching", projectPath],
		queryFn: () => commands.projectIsUnityLaunching(projectPath),
		refetchInterval: 2000,
		refetchIntervalInBackground: false,
	});
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/// Asks Unity to close the project, and offers to force kill it if it does not.
export async function closeUnity(projectPath: string) {
	try {
		const requested = await commands.projectCloseUnity(projectPath);
		if (!requested) {
			toastNormal(tc("projects:toast:unity not running"));
			return;
		}

		toastNormal(tc("projects:toast:closing unity..."));

		const deadline = Date.now() + GRACEFUL_CLOSE_TIMEOUT_MS;
		while (Date.now() < deadline) {
			await sleep(POLL_INTERVAL_MS);
			if (!(await commands.projectIsUnityLaunching(projectPath))) {
				toastSuccess(tc("projects:toast:unity closed"));
				return;
			}
		}

		const result = await openSingleDialog(ForceKillUnityDialog, {
			projectName: nameFromPath(projectPath),
			seconds: GRACEFUL_CLOSE_TIMEOUT_MS / 1000,
		});
		if (result == null) return;

		if (result.backup) {
			const backup = await openSingleDialog(BackupProjectDialog, {
				projectPath,
				header: tc("projects:dialog:backup before force kill header"),
			});
			// Don't kill Unity, and lose the progress, if the backup did not complete.
			if (backup === "cancelled") {
				toastNormal(tc("projects:toast:backup canceled"));
				return;
			}
			toastSuccess(tc("projects:toast:backup succeeded"));
		}

		const killed = await commands.projectKillUnity(projectPath);
		if (killed) toastSuccess(tc("projects:toast:unity killed"));
		else toastNormal(tc("projects:toast:unity not running"));
	} catch (e) {
		console.error(e);
		toastThrownError(e);
	}
}

function ForceKillUnityDialog({
	projectName,
	seconds,
	dialog,
}: {
	projectName: string;
	seconds: number;
	dialog: DialogContext<{ backup: boolean } | null>;
}) {
	const [backup, setBackup] = useState(false);

	return (
		<>
			<DialogTitle>{tc("projects:dialog:unity did not close")}</DialogTitle>
			<div>
				<p className={"whitespace-normal"}>
					{tc("projects:dialog:unity did not close description", {
						name: projectName,
						seconds,
					})}
				</p>
			</div>
			<div>
				<label className={"flex items-center gap-2 whitespace-normal"}>
					<Checkbox
						checked={backup}
						onCheckedChange={(e) => setBackup(e === true)}
					/>
					{tc("projects:dialog:backup before force kill")}
				</label>
			</div>
			<DialogFooter className={"gap-2"}>
				<Button onClick={() => dialog.close(null)} className="mr-1">
					{tc("general:button:cancel")}
				</Button>
				<Button
					onClick={() => dialog.close({ backup })}
					variant={"destructive"}
				>
					{tc("projects:dialog:button:force kill unity")}
				</Button>
			</DialogFooter>
		</>
	);
}
